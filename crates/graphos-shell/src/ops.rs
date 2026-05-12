use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use graphos_core::{
    Edge, EdgeKind, GraphError, GraphPool, NodeId, NodeType,
    edge::{EdgeFlags, EdgeWeight},
    fnv1a_64, CapabilityToken,
};

use crate::parser::{Command, Path};
use crate::render;
use crate::session::ShellSession;

/// Execute a parsed command, return output text
pub fn execute(
    cmd: &Command,
    session: &mut ShellSession,
    pool: &mut GraphPool,
) -> Result<String, GraphError> {
    match cmd {
        Command::Look { deep } => op_look(session, pool, *deep),
        Command::Go { path } => op_go(session, pool, path),
        Command::Spawn { type_name, name } => op_spawn(session, pool, type_name, name.as_deref()),
        Command::Who => op_who(session, pool),
        Command::Touch { edge_name } => op_touch(session, pool, edge_name),
        Command::Help => Ok(render::help_text()),
        Command::Empty => Ok(String::new()),
        Command::NaturalLanguage { text } => {
            Ok(format!("[AI] Intent not yet connected: \"{}\"", text))
        }
        Command::Find {
            type_filter,
            max_depth,
        } => op_find(session, pool, type_filter.as_deref(), *max_depth),
        Command::Bind { name, target } => op_bind(session, pool, name, target),
        Command::Cut { src, target } => op_cut(session, pool, src, target),
        Command::Link { src, target, kind } => op_link(session, pool, src, target, kind),
        Command::History => Ok(String::from("[history not yet implemented]")),
        Command::Ask { prompt } => Ok(format!("[AI] {}", prompt)),
        Command::RawTraversal { node_id_hex } => Ok(format!("[raw] #{}", node_id_hex)),
        Command::Snapshot => Ok(String::from("[SNAPSHOT] Would create snapshot (not yet wired)")),
        Command::Rollback { epoch } => {
            Ok(format!("[SNAPSHOT] Would rollback to epoch {} (not yet wired)", epoch))
        }
        Command::Store { data } => {
            Ok(format!("[CAS] Would store: {} ({} bytes)", data, data.len()))
        }
        Command::Similar { k } => {
            Ok(format!("[VECTOR] Would find {} similar nodes (not yet wired)", k))
        }
        Command::Physics { steps } => {
            Ok(format!("[VECTOR] Would run {} physics steps (not yet wired)", steps))
        }
        Command::Clusters => Ok(String::from("[VECTOR] Would show clusters (not yet wired)")),
    }
}

fn op_look(session: &ShellSession, pool: &GraphPool, deep: bool) -> Result<String, GraphError> {
    let mut out = String::new();

    out.push_str(&format!(
        "Node: {:?} (type={:#06x})\n",
        session.cwd,
        session.cwd.node_type()
    ));
    out.push_str("Edges:\n");

    for edge in pool.edges_from(session.cwd) {
        let kind_name = edge_kind_name(edge.kind);
        let target_type = edge.target.node_type();
        out.push_str(&format!(
            "  --[{}]--> (type={:#06x}, ctr={})",
            kind_name,
            target_type,
            edge.target.counter()
        ));
        if edge.payload != 0 {
            out.push_str(&format!(" name_hash={:#018x}", edge.payload));
        }
        out.push('\n');
    }

    if deep {
        out.push_str("\n  [deep traversal: 2nd hop]\n");
        for edge in pool.edges_from(session.cwd) {
            for inner in pool.edges_from(edge.target) {
                out.push_str(&format!(
                    "    --[{}]--> (type={:#06x})\n",
                    edge_kind_name(inner.kind),
                    inner.target.node_type()
                ));
            }
        }
    }

    Ok(out)
}

fn op_go(
    session: &mut ShellSession,
    pool: &mut GraphPool,
    path: &Path,
) -> Result<String, GraphError> {
    let target = resolve_path(session, pool, path)?;
    session.go(pool, target)?;
    Ok(format!(
        "Moved to node (type={:#06x}, ctr={})",
        target.node_type(),
        target.counter()
    ))
}

fn op_spawn(
    session: &mut ShellSession,
    pool: &mut GraphPool,
    type_name: &str,
    name: Option<&str>,
) -> Result<String, GraphError> {
    let node_type = parse_node_type(type_name).ok_or(GraphError::InvalidEdge)?;
    let new_id = pool.alloc_node(node_type, &session.cap, 1)?;

    // Connect from cwd to new node
    let edge = if let Some(n) = name {
        Edge::new_named(session.cwd, new_id, n)
    } else {
        Edge::new_reference(session.cwd, new_id)
    };
    pool.connect(edge, &session.cap)?;

    Ok(format!(
        "Spawned {:#06x} node (ctr={}){}",
        node_type as u16,
        new_id.counter(),
        name.map(|n| format!(" as \"{}\"", n))
            .unwrap_or_default()
    ))
}

fn op_who(session: &ShellSession, pool: &GraphPool) -> Result<String, GraphError> {
    let mut out = String::new();
    out.push_str("Shell Session:\n");
    out.push_str(&format!(
        "  node_id: type={:#06x}, ctr={}\n",
        session.node_id.node_type(),
        session.node_id.counter()
    ));
    out.push_str(&format!(
        "  cwd:     type={:#06x}, ctr={}\n",
        session.cwd.node_type(),
        session.cwd.counter()
    ));
    out.push_str(&format!(
        "  cap:     rights={:#06x}, scope={:#06x}, badge={}\n",
        session.cap.rights.bits(),
        session.cap.scope,
        session.cap.badge
    ));

    let edge_count = pool.edges_from(session.cwd).count();
    out.push_str(&format!("  cwd edges: {}\n", edge_count));
    Ok(out)
}

fn op_touch(
    session: &ShellSession,
    pool: &GraphPool,
    edge_name: &str,
) -> Result<String, GraphError> {
    let name_hash = fnv1a_64(edge_name.as_bytes());
    for edge in pool.edges_from(session.cwd) {
        if edge.payload == name_hash {
            let result = pool.traverse(session.cwd, edge.target, &session.cap)?;
            return Ok(format!("Traversed '{}' -> {:?}", edge_name, result));
        }
    }
    Err(GraphError::EdgeNotFound)
}

fn op_find(
    session: &ShellSession,
    pool: &GraphPool,
    type_filter: Option<&str>,
    max_depth: u8,
) -> Result<String, GraphError> {
    let mut out = String::new();
    let target_type: Option<u16> = type_filter.and_then(|s| parse_node_type(s)).map(|t| t as u16);

    // Simple BFS
    let mut visited: Vec<NodeId> = Vec::new();
    let mut queue: Vec<(NodeId, u8)> = Vec::new();
    queue.push((session.cwd, 0));

    while let Some((node, depth)) = queue.pop() {
        if depth > max_depth {
            continue;
        }
        if visited.contains(&node) {
            continue;
        }
        visited.push(node);

        let nt = node.node_type();
        if target_type.is_none() || target_type == Some(nt) {
            if node != session.cwd {
                out.push_str(&format!(
                    "  [depth={}] type={:#06x}, ctr={}\n",
                    depth,
                    nt,
                    node.counter()
                ));
            }
        }

        if depth < max_depth {
            for edge in pool.edges_from(node) {
                queue.push((edge.target, depth + 1));
            }
        }
    }

    if out.is_empty() {
        out.push_str("  (no nodes found)\n");
    }
    Ok(out)
}

fn op_bind(
    session: &mut ShellSession,
    pool: &mut GraphPool,
    name: &str,
    target_path: &Path,
) -> Result<String, GraphError> {
    let target = resolve_path(session, pool, target_path)?;
    let edge = Edge {
        source: session.binding_node,
        target,
        kind: EdgeKind::Alias,
        weight: EdgeWeight::DEFAULT,
        required_cap: CapabilityToken::OPEN,
        payload: fnv1a_64(name.as_bytes()),
        flags: EdgeFlags::empty(),
        _pad: [0; 7],
    };
    pool.connect(edge, &session.cap)?;
    Ok(format!(
        "Bound '{}' -> (type={:#06x}, ctr={})",
        name,
        target.node_type(),
        target.counter()
    ))
}

fn op_cut(
    session: &mut ShellSession,
    pool: &mut GraphPool,
    src: &Path,
    target: &Path,
) -> Result<String, GraphError> {
    let src_id = resolve_path(session, pool, src)?;
    let tgt_id = resolve_path(session, pool, target)?;
    pool.disconnect(src_id, tgt_id, &session.cap)?;
    Ok(String::from("Edge removed."))
}

fn op_link(
    session: &mut ShellSession,
    pool: &mut GraphPool,
    src: &Path,
    target: &Path,
    kind: &str,
) -> Result<String, GraphError> {
    let src_id = resolve_path(session, pool, src)?;
    let tgt_id = resolve_path(session, pool, target)?;
    let edge_kind = parse_edge_kind(kind);
    let edge = Edge {
        source: src_id,
        target: tgt_id,
        kind: edge_kind,
        weight: EdgeWeight::DEFAULT,
        required_cap: CapabilityToken::OPEN,
        payload: 0,
        flags: EdgeFlags::empty(),
        _pad: [0; 7],
    };
    pool.connect(edge, &session.cap)?;
    Ok(format!(
        "Linked ({:#06x}) --[{}]--> ({:#06x})",
        src_id.node_type(),
        kind,
        tgt_id.node_type()
    ))
}

// --- Helpers ---

fn resolve_path(
    session: &ShellSession,
    pool: &GraphPool,
    path: &Path,
) -> Result<NodeId, GraphError> {
    match path {
        Path::Absolute(segments) => pool.resolve_path(session.cwd, segments, &session.cap),
        Path::Relative(segments) => pool.resolve_path(session.cwd, segments, &session.cap),
        Path::Parent => {
            // Parent traversal not yet implemented (requires reverse-edge index)
            Err(GraphError::PathNotFound)
        }
        Path::Alias(name) => {
            let name_hash = fnv1a_64(name.as_bytes());
            for edge in pool.edges_from(session.binding_node) {
                if edge.kind == EdgeKind::Alias && edge.payload == name_hash {
                    return Ok(edge.target);
                }
            }
            Err(GraphError::PathNotFound)
        }
        Path::Direct(_hex) => {
            // TODO: Parse hex string to NodeId
            Err(GraphError::PathNotFound)
        }
    }
}

fn parse_node_type(s: &str) -> Option<NodeType> {
    match s {
        "root" | "Root" => Some(NodeType::Root),
        "process" | "Process" => Some(NodeType::Process),
        "memory" | "Memory" => Some(NodeType::Memory),
        "file" | "File" => Some(NodeType::File),
        "device" | "Device" => Some(NodeType::Device),
        "model" | "Model" => Some(NodeType::Model),
        "wasm" | "Wasm" | "WasmModule" => Some(NodeType::WasmModule),
        "capability" | "Capability" => Some(NodeType::Capability),
        "channel" | "Channel" => Some(NodeType::Channel),
        "socket" | "Socket" => Some(NodeType::Socket),
        "directory" | "Directory" | "dir" => Some(NodeType::Directory),
        "scheduler" | "Scheduler" => Some(NodeType::Scheduler),
        "jit" | "JitCode" => Some(NodeType::JitCode),
        "serial" | "Serial" => Some(NodeType::Serial),
        "disk" | "Disk" => Some(NodeType::Disk),
        "network" | "Network" | "net" => Some(NodeType::Network),
        "filesystem" | "Filesystem" | "fs" => Some(NodeType::Filesystem),
        "ai" | "AiEngine" => Some(NodeType::AiEngine),
        "shell" | "Shell" => Some(NodeType::Shell),
        "command" | "Command" => Some(NodeType::Command),
        "binding" | "Binding" => Some(NodeType::Binding),
        "history" | "History" => Some(NodeType::History),
        _ => None,
    }
}

fn parse_edge_kind(s: &str) -> EdgeKind {
    match s {
        "reference" | "ref" => EdgeKind::Reference,
        "staticfn" | "fn" => EdgeKind::StaticFn,
        "jit" => EdgeKind::JitCompiled,
        "wasm" => EdgeKind::WasmCall,
        "inference" | "infer" => EdgeKind::InferenceCall,
        "message" | "msg" => EdgeKind::Message,
        "capdelegate" | "cap" => EdgeKind::CapDelegate,
        "alias" => EdgeKind::Alias,
        _ => EdgeKind::Reference,
    }
}

fn edge_kind_name(kind: EdgeKind) -> &'static str {
    match kind {
        EdgeKind::Reference => "ref",
        EdgeKind::StaticFn => "fn",
        EdgeKind::JitCompiled => "jit",
        EdgeKind::WasmCall => "wasm",
        EdgeKind::InferenceCall => "infer",
        EdgeKind::Message => "msg",
        EdgeKind::CapDelegate => "cap",
        EdgeKind::Alias => "alias",
        EdgeKind::Stdin => "stdin",
        EdgeKind::Stdout => "stdout",
        EdgeKind::CwdEdge => "cwd",
        EdgeKind::PipeSegment => "pipe",
        EdgeKind::HistoryLink => "history",
    }
}
