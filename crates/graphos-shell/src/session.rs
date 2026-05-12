use graphos_core::{
    GraphPool, NodeId, NodeType, EdgeKind, Edge, CapabilityToken, CapRights, GraphError,
    edge::{EdgeWeight, EdgeFlags},
};

/// A shell session — itself a node in the graph.
/// On creation, edges to stdin/stdout/cwd are established.
pub struct ShellSession {
    pub node_id: NodeId,
    pub cwd: NodeId,
    pub cap: CapabilityToken,
    pub history_head: NodeId,
    pub binding_node: NodeId,
}

impl ShellSession {
    /// Create a new shell session in the graph.
    /// Allocates Shell-Node, Binding-Node, History-Node and wires them up.
    pub fn new(
        pool: &mut GraphPool,
        root_node: NodeId,
        serial_node: NodeId,
        cap: &CapabilityToken,
        timestamp: u64,
    ) -> Result<Self, GraphError> {
        // 1. Shell node
        let shell_id = pool.alloc_node(NodeType::Shell, cap, timestamp)?;

        // 2. Binding node (aliases)
        let binding_id = pool.alloc_node(NodeType::Binding, cap, timestamp)?;

        // 3. History node
        let history_id = pool.alloc_node(NodeType::History, cap, timestamp)?;

        // 4. CwdEdge: Shell -> Root
        let cwd_edge = Edge {
            source: shell_id,
            target: root_node,
            kind: EdgeKind::CwdEdge,
            weight: EdgeWeight::DEFAULT,
            required_cap: CapabilityToken::OPEN,
            payload: 0,
            flags: EdgeFlags::empty(),
            _pad: [0; 7],
        };
        pool.connect(cwd_edge, cap)?;

        // 5. Stdout: Shell -> Serial
        let stdout_edge = Edge {
            source: shell_id,
            target: serial_node,
            kind: EdgeKind::Stdout,
            weight: EdgeWeight::DEFAULT,
            required_cap: CapabilityToken::OPEN,
            payload: 0,
            flags: EdgeFlags::empty(),
            _pad: [0; 7],
        };
        pool.connect(stdout_edge, cap)?;

        // 6. Stdin: Serial -> Shell (reverse direction)
        let stdin_edge = Edge {
            source: serial_node,
            target: shell_id,
            kind: EdgeKind::Stdin,
            weight: EdgeWeight::DEFAULT,
            required_cap: CapabilityToken::OPEN,
            payload: 0,
            flags: EdgeFlags::empty(),
            _pad: [0; 7],
        };
        pool.connect(stdin_edge, cap)?;

        // 7. Shell -> Binding (named reference)
        let binding_edge = Edge::new_named(shell_id, binding_id, "bindings");
        pool.connect(binding_edge, cap)?;

        // 8. Shell -> History (named reference)
        let history_edge = Edge::new_named(shell_id, history_id, "history");
        pool.connect(history_edge, cap)?;

        // Derive session capability (restricted: no KERNEL, no REVOKE)
        let session_cap = cap
            .derive(
                CapRights::READ
                    | CapRights::WRITE
                    | CapRights::EXECUTE
                    | CapRights::TRAVERSE
                    | CapRights::CREATE
                    | CapRights::DELETE
                    | CapRights::DELEGATE,
                0xFFFF,
            )
            .unwrap_or(*cap);

        Ok(Self {
            node_id: shell_id,
            cwd: root_node,
            cap: session_cap,
            history_head: history_id,
            binding_node: binding_id,
        })
    }

    /// Move cursor to a new node (rehang CwdEdge)
    pub fn go(&mut self, pool: &mut GraphPool, target: NodeId) -> Result<(), GraphError> {
        pool.rehang_edge(self.node_id, EdgeKind::CwdEdge, target, &self.cap)?;
        self.cwd = target;
        Ok(())
    }
}
