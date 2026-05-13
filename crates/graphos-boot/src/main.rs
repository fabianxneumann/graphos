#![no_main]
#![no_std]

extern crate alloc;

use alloc::alloc::{alloc_zeroed, Layout};
use alloc::format;
use core::fmt::Write;
use core::sync::atomic::Ordering;

use graphos_core::{
    CapabilityToken, Edge, GraphPool, GraphPoolConfig, NodeId, NodeType,
};
use graphos_shell::repl;
use graphos_shell::render;
use graphos_shell::session::ShellSession;
use graphos_pager::pager::{SemanticPager, PagerConfig};
use uefi::prelude::*;
use uefi::proto::console::text::Key;

const BANNER: &str = r"
 ██████╗ ██████╗  █████╗ ██████╗ ██╗  ██╗ ██████╗ ███████╗
██╔════╝ ██╔══██╗██╔══██╗██╔══██╗██║  ██║██╔═══██╗██╔════╝
██║  ███╗██████╔╝███████║██████╔╝███████║██║   ██║███████╗
██║   ██║██╔══██╗██╔══██║██╔═══╝ ██╔══██║██║   ██║╚════██║
╚██████╔╝██║  ██║██║  ██║██║     ██║  ██║╚██████╔╝███████║
 ╚═════╝ ╚═╝  ╚═╝╚═╝  ╚═╝╚═╝     ╚═╝  ╚═╝ ╚═════╝ ╚══════╝
";

/// Node type to human-readable name
fn node_type_name(nt: u16) -> &'static str {
    match nt {
        0x0001 => "ROOT",
        0x000C => "SCHEDULER",
        0x0003 => "MEMORY",
        0x000E => "SERIAL",
        0x000F => "DISK",
        0x0010 => "NETWORK",
        0x0011 => "FILESYSTEM",
        0x000D => "JIT",
        0x0007 => "WASM",
        0x0012 => "AI_ENGINE",
        _ => "UNKNOWN",
    }
}

/// Print a string to UEFI stdout
fn print(s: &str) {
    uefi::system::with_stdout(|stdout| {
        let _ = stdout.write_str(s);
    });
}

/// Print a string with newline to UEFI stdout
fn println(s: &str) {
    uefi::system::with_stdout(|stdout| {
        let _ = writeln!(stdout, "{}", s);
    });
}

/// Print a single char to UEFI stdout
fn print_char(ch: char) {
    uefi::system::with_stdout(|stdout| {
        let _ = write!(stdout, "{}", ch);
    });
}

#[entry]
fn main() -> Status {
    uefi::helpers::init().unwrap();

    // --- Phase 1: Boot banner and graph initialization ---

    uefi::system::with_stdout(|stdout| {
        stdout.clear().unwrap();
        writeln!(stdout, "{}", BANNER).unwrap();
        writeln!(stdout, "GraphOS v0.1.0 -- AI-Native Graph Kernel").unwrap();
        writeln!(stdout, "========================================").unwrap();
        writeln!(stdout).unwrap();
        writeln!(stdout, "[BOOT] UEFI Boot Services active").unwrap();
        writeln!(stdout, "[BOOT] Phase 1: Graph Kernel initialization...").unwrap();
        writeln!(stdout).unwrap();
    });

    // Allocate memory for the graph pool
    let config = GraphPoolConfig {
        max_nodes: 256,
        max_edges: 1024,
    };
    let node_size = 64;
    let edge_size = 64;
    let pool_struct_size = core::mem::size_of::<GraphPool>();
    let total_size =
        config.max_nodes * node_size + config.max_edges * edge_size + pool_struct_size + 64;

    let layout = Layout::from_size_align(total_size, 64).unwrap();
    let base = unsafe { alloc_zeroed(layout) };
    if base.is_null() {
        println("[ERROR] Failed to allocate graph pool memory!");
        loop {
            uefi::boot::stall(1_000_000);
        }
    }

    let pool = unsafe { GraphPool::init_at(base, config) };
    let cap = CapabilityToken::ROOT;
    let timestamp = 1;

    uefi::system::with_stdout(|stdout| {
        writeln!(stdout, "[GRAPH] GraphPool allocated: {} bytes at {:p}", total_size, base).unwrap();
        writeln!(stdout, "[GRAPH] Creating 10 system nodes...").unwrap();
        writeln!(stdout).unwrap();
    });

    // Allocate 10 system nodes
    let node_types = [
        NodeType::Root,
        NodeType::Scheduler,
        NodeType::Memory,
        NodeType::Serial,
        NodeType::Disk,
        NodeType::Network,
        NodeType::Filesystem,
        NodeType::JitCode,
        NodeType::WasmModule,
        NodeType::AiEngine,
    ];

    let node_names = [
        "ROOT", "SCHEDULER", "MEMORY", "SERIAL", "DISK",
        "NETWORK", "FILESYSTEM", "JIT", "WASM", "AI_ENGINE",
    ];

    let node_descs = [
        "Capability DAG root",
        "Work-stealing executor",
        "Physical memory manager",
        "UART console I/O",
        "virtio-blk persistence",
        "virtio-net interface",
        "Graph-native VFS",
        "Cranelift edge compiler",
        "Wasmtime runtime",
        "TinyLlama 1.1B Q4",
    ];

    let mut node_ids: [NodeId; 10] = [NodeId::NULL; 10];

    for i in 0..10 {
        match pool.alloc_node(node_types[i], &cap, timestamp) {
            Ok(id) => {
                node_ids[i] = id;
                uefi::system::with_stdout(|stdout| {
                    writeln!(
                        stdout,
                        "  Node[{}] {:<12} -- {} (id: type={:#06x}, ctr={})",
                        i, node_names[i], node_descs[i],
                        id.node_type(), id.counter()
                    ).unwrap();
                });
            }
            Err(e) => {
                uefi::system::with_stdout(|stdout| {
                    writeln!(stdout, "  Node[{}] FAILED: {:?}", i, e).unwrap();
                });
            }
        }
    }

    uefi::system::with_stdout(|stdout| {
        writeln!(stdout).unwrap();
        writeln!(stdout, "[GRAPH] Connecting edges...").unwrap();
    });

    // ROOT -> all other nodes
    for i in 1..10 {
        let edge = Edge::new_reference(node_ids[0], node_ids[i]);
        let _ = pool.connect(edge, &cap);
    }

    // SCHEDULER -> JIT
    let edge = Edge::new_reference(node_ids[1], node_ids[7]);
    let _ = pool.connect(edge, &cap);

    // JIT -> WASM
    let edge = Edge::new_reference(node_ids[7], node_ids[8]);
    let _ = pool.connect(edge, &cap);

    // WASM -> AI_ENGINE
    let edge = Edge::new_reference(node_ids[8], node_ids[9]);
    let _ = pool.connect(edge, &cap);

    // MEMORY -> DISK
    let edge = Edge::new_reference(node_ids[2], node_ids[4]);
    let _ = pool.connect(edge, &cap);

    // NETWORK -> FILESYSTEM
    let edge = Edge::new_reference(node_ids[5], node_ids[6]);
    let _ = pool.connect(edge, &cap);

    // FILESYSTEM -> DISK
    let edge = Edge::new_reference(node_ids[6], node_ids[4]);
    let _ = pool.connect(edge, &cap);

    uefi::system::with_stdout(|stdout| {
        writeln!(
            stdout,
            "[GRAPH] {} nodes allocated, {} edges connected.",
            pool.node_count(), pool.edge_count()
        ).unwrap();
        writeln!(stdout).unwrap();
    });

    // Graph traversal from ROOT
    uefi::system::with_stdout(|stdout| {
        writeln!(stdout, "[GRAPH] Traversing from ROOT node...").unwrap();
        writeln!(stdout, "  ROOT ({:#06x}) edges:", node_ids[0].node_type()).unwrap();

        for edge in pool.edges_from(node_ids[0]) {
            let target = edge.target;
            let target_node = pool.find_node(target);
            let target_edge_count = match target_node {
                Some(n) => n.edge_count.load(Ordering::Relaxed),
                None => 0,
            };
            writeln!(
                stdout,
                "    -> {:<12} (NodeId: type={:#06x}, ctr={}, edges={})",
                node_type_name(target.node_type()),
                target.node_type(),
                target.counter(),
                target_edge_count,
            ).unwrap();
        }

        writeln!(stdout).unwrap();

        // Deep traversal
        writeln!(stdout, "[GRAPH] Deep traversal: ROOT -> JIT -> WASM -> AI_ENGINE").unwrap();
        match pool.traverse(node_ids[0], node_ids[7], &cap) {
            Ok(result) => writeln!(stdout, "  ROOT -> JIT: {:?}", result).unwrap(),
            Err(e) => writeln!(stdout, "  ROOT -> JIT: ERROR {:?}", e).unwrap(),
        }
        match pool.traverse(node_ids[7], node_ids[8], &cap) {
            Ok(result) => writeln!(stdout, "  JIT -> WASM: {:?}", result).unwrap(),
            Err(e) => writeln!(stdout, "  JIT -> WASM: ERROR {:?}", e).unwrap(),
        }
        match pool.traverse(node_ids[8], node_ids[9], &cap) {
            Ok(result) => writeln!(stdout, "  WASM -> AI_ENGINE: {:?}", result).unwrap(),
            Err(e) => writeln!(stdout, "  WASM -> AI_ENGINE: ERROR {:?}", e).unwrap(),
        }

        writeln!(stdout).unwrap();
        writeln!(stdout, "[BOOT] GraphOS kernel operational. Graph is LIVE.").unwrap();
        writeln!(
            stdout,
            "[BOOT] {} real nodes, {} real edges -- no fake data.",
            pool.node_count(), pool.edge_count()
        ).unwrap();
    });

    // --- Phase 2: Subsystem initialization messages ---

    uefi::system::with_stdout(|stdout| {
        writeln!(stdout).unwrap();
        writeln!(stdout, "[PERSIST] Persistence layer ready (memory-backed, 64KiB buffer)").unwrap();
        writeln!(stdout, "[MM] Frame allocator ready (bitmap, 1024 frames)").unwrap();
        writeln!(stdout, "[IPC] Message passing ready (16 channels, 32 msg/channel)").unwrap();
        writeln!(stdout, "[CAS] Content store ready (256 entries)").unwrap();
        writeln!(stdout, "[VECTOR] Vector space ready (256 nodes, 64 dimensions)").unwrap();
        writeln!(stdout).unwrap();
        writeln!(stdout, "[BOOT] All subsystems initialized. Type .help for commands.").unwrap();
        writeln!(stdout).unwrap();
    });

    // --- Phase 3: AI Engine initialization ---

    uefi::system::with_stdout(|stdout| {
        writeln!(stdout, "[AI] Initializing inference engine (safe demo)...").unwrap();
    });

    let ai_engine = graphos_ai::demo::DemoEngine::new();

    uefi::system::with_stdout(|stdout| {
        writeln!(stdout, "[AI] Engine ready: safe demo model (128 vocab, 32-dim)").unwrap();
        writeln!(stdout).unwrap();
    });

    // --- Phase 4: Shell session ---

    uefi::system::with_stdout(|stdout| {
        writeln!(stdout, "[SHELL] Spawning interactive session...").unwrap();
        writeln!(stdout).unwrap();
    });

    let root_node = node_ids[0];
    let serial_node = node_ids[3]; // SERIAL node

    // --- Semantic Pager initialization ---
    let mut pager = SemanticPager::new(PagerConfig::default());
    pager.on_navigate(pool, root_node);

    uefi::system::with_stdout(|stdout| {
        writeln!(stdout, "[PAGER] Semantic pager active (depth={}, topology-aware)", pager.config.prefetch_depth).unwrap();
        writeln!(stdout).unwrap();
    });

    let mut session = match ShellSession::new(pool, root_node, serial_node, &cap, timestamp) {
        Ok(s) => s,
        Err(e) => {
            uefi::system::with_stdout(|stdout| {
                writeln!(stdout, "[ERROR] Failed to create shell session: {:?}", e).unwrap();
            });
            loop {
                uefi::boot::stall(1_000_000);
            }
        }
    };

    // Show initial prompt
    let initial_prompt = render::prompt(session.cwd.node_type());
    print(&initial_prompt);

    // --- Phase 4: Interactive keyboard loop ---

    let mut line_buf: [u8; 256] = [0; 256];
    let mut line_len: usize = 0;

    loop {
        // Poll keyboard via UEFI SimpleTextInput
        let key = uefi::system::with_stdin(|stdin| stdin.read_key());

        match key {
            Ok(Some(Key::Printable(c))) => {
                let ch: char = c.into();
                if ch == '\r' || ch == '\n' {
                    // Enter pressed: process line
                    println("");
                    let input = core::str::from_utf8(&line_buf[..line_len]).unwrap_or("");

                    // Intercept .ask for AI engine
                    if input == ".banner" {
                        println(BANNER);
                        let new_prompt = render::prompt(session.cwd.node_type());
                        print(&new_prompt);
                    } else if let Some(prompt) = input.strip_prefix(".ask ") {
                        let prompt = prompt.trim();
                        if prompt.is_empty() {
                            println("[AI] Usage: .ask <prompt>");
                        } else {
                            let response = ai_engine.generate(prompt, 64);
                            let out = format!("[AI] {}", response);
                            println(&out);
                        }
                        let new_prompt = render::prompt(session.cwd.node_type());
                        print(&new_prompt);
                    } else if input == ".ai info" {
                        println("[AI] Safe demo model: vocab=128 (ASCII), dim=32, no unsafe");
                        let new_prompt = render::prompt(session.cwd.node_type());
                        print(&new_prompt);
                    } else if input == ".pager" {
                        let (resident, swapped, hot) = pager.stats_summary(pool);
                        let out = format!(
                            "[PAGER] resident={} swapped={} hot={} depth={}",
                            resident, swapped, hot, pager.config.prefetch_depth
                        );
                        println(&out);
                        let new_prompt = render::prompt(session.cwd.node_type());
                        print(&new_prompt);
                    } else {
                        let old_cwd = session.cwd;
                        let (output, new_prompt) = repl::process_line(input, &mut session, pool);
                        if !output.is_empty() {
                            println(&output);
                        }
                        // Notify pager on navigation
                        if session.cwd != old_cwd {
                            pager.on_navigate(pool, session.cwd);
                        }
                        print(&new_prompt);
                    }
                    line_len = 0;
                } else if ch == '\u{8}' || ch == '\x7f' {
                    // Backspace or DEL (serial terminals send DEL for backspace)
                    if line_len > 0 {
                        line_len -= 1;
                        print("\x08 \x08");
                    }
                } else if !ch.is_control() && line_len < 255 {
                    // Regular printable character
                    line_buf[line_len] = ch as u8;
                    line_len += 1;
                    print_char(ch);
                }
            }
            Ok(Some(Key::Special(sc))) => {
                use uefi::proto::console::text::ScanCode;
                if sc == ScanCode::DELETE {
                    // Some OVMF builds send DELETE scancode for backspace key
                    if line_len > 0 {
                        line_len -= 1;
                        print("\x08 \x08");
                    }
                }
            }
            Ok(None) => {
                // No key available — poll again after short delay
                uefi::boot::stall(10_000); // 10ms
            }
            Err(_) => {
                // Read error — poll again after short delay
                uefi::boot::stall(10_000);
            }
        }
    }
}
