use crate::node_id::{NodeId, NodeIdGenerator, NodeType};
use crate::node::NodeHeader;
use crate::edge::{Edge, EdgeKind};
use crate::capability::{CapabilityToken, CapRights};
use crate::hash::fnv1a_64;

const EMPTY_SLOT: u32 = 0xFFFF_FFFF;
const HASH_TABLE_SIZE: usize = 65536; // Must be power of 2 for mask
const EDGE_HEAD_SIZE: usize = 65536;

/// FNV-1a hash on the lower 64 bits (counter) of a NodeId for hash table probing.
#[inline]
fn hash_node_id(id: NodeId) -> usize {
    let key = id.counter();
    let bytes = key.to_le_bytes();
    (fnv1a_64(&bytes) as usize) & (HASH_TABLE_SIZE - 1)
}

/// Errors from graph operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphError {
    OutOfMemory,
    NodeNotFound,
    EdgeNotFound,
    InsufficientRights,
    ScopeViolation,
    PoolFull,
    InvalidEdge,
    PathNotFound,
    AmbiguousPath,
    PipelineAborted,
}

/// Result of a graph traversal
#[derive(Debug)]
pub enum TraversalResult {
    NodeReached(NodeId),
    FnExecuted { target: NodeId, result_ptr: u64 },
    MessageSent { target: NodeId },
}

/// Configuration for GraphPool
pub struct GraphPoolConfig {
    pub max_nodes: usize,
    pub max_edges: usize,
}

impl Default for GraphPoolConfig {
    fn default() -> Self {
        Self {
            max_nodes: 65536,
            max_edges: 262144,
        }
    }
}

/// The central graph data structure.
/// Uses contiguous arrays for cache-friendly iteration.
pub struct GraphPool {
    nodes: &'static mut [NodeHeader],
    edges: &'static mut [Edge],
    node_count: usize,
    edge_count: usize,
    id_gen: NodeIdGenerator,
    config: GraphPoolConfig,
    // Index structures for O(1) node lookup and O(degree) edge iteration
    node_map: &'static mut [u32],      // Open-addressing hash: hash(NodeId) -> slab_index
    edge_heads: &'static mut [u32],    // First edge index per node slab_index
}

impl GraphPool {
    /// Initialize a GraphPool at a given memory region.
    /// Safety: `base` must point to a properly aligned, zeroed region of at least
    /// `config.max_nodes * 64 + config.max_edges * 64 + HASH_TABLE_SIZE * 4 + EDGE_HEAD_SIZE * 4`
    /// bytes plus `size_of::<GraphPool>()`.
    pub unsafe fn init_at(base: *mut u8, config: GraphPoolConfig) -> &'static mut Self {
        let nodes_size = config.max_nodes * core::mem::size_of::<NodeHeader>();
        let edges_size = config.max_edges * core::mem::size_of::<Edge>();
        let node_map_size = HASH_TABLE_SIZE * core::mem::size_of::<u32>();
        let edge_heads_size = EDGE_HEAD_SIZE * core::mem::size_of::<u32>();

        let nodes_ptr = base as *mut NodeHeader;
        let edges_ptr = base.add(nodes_size) as *mut Edge;
        let node_map_ptr = base.add(nodes_size + edges_size) as *mut u32;
        let edge_heads_ptr = base.add(nodes_size + edges_size + node_map_size) as *mut u32;

        let nodes = core::slice::from_raw_parts_mut(nodes_ptr, config.max_nodes);
        let edges = core::slice::from_raw_parts_mut(edges_ptr, config.max_edges);
        let node_map = core::slice::from_raw_parts_mut(node_map_ptr, HASH_TABLE_SIZE);
        let edge_heads = core::slice::from_raw_parts_mut(edge_heads_ptr, EDGE_HEAD_SIZE);

        // Initialize hash table and edge heads to EMPTY
        for slot in node_map.iter_mut() {
            *slot = EMPTY_SLOT;
        }
        for head in edge_heads.iter_mut() {
            *head = EMPTY_SLOT;
        }

        // Place the GraphPool struct itself after all data arrays
        let total_data = nodes_size + edges_size + node_map_size + edge_heads_size;
        let pool_ptr = base.add(total_data) as *mut GraphPool;
        pool_ptr.write(GraphPool {
            nodes,
            edges,
            node_count: 0,
            edge_count: 0,
            id_gen: NodeIdGenerator::new(),
            config,
            node_map,
            edge_heads,
        });
        &mut *pool_ptr
    }

    /// Allocate a new node
    pub fn alloc_node(
        &mut self,
        node_type: NodeType,
        cap: &CapabilityToken,
        timestamp_ms: u64,
    ) -> Result<NodeId, GraphError> {
        if !cap.rights.contains(CapRights::CREATE) {
            return Err(GraphError::InsufficientRights);
        }
        if self.node_count >= self.config.max_nodes {
            return Err(GraphError::PoolFull);
        }

        let id = self.id_gen.next(node_type, timestamp_ms);
        let idx = self.node_count;
        self.nodes[idx] = NodeHeader::new(id, node_type as u16, CapabilityToken::OPEN);
        self.nodes[idx].slab_index = idx as u32;
        self.node_count += 1;
        self.hash_insert(id, idx as u32);
        Ok(id)
    }

    /// Connect two nodes with an edge
    pub fn connect(
        &mut self,
        edge: Edge,
        cap: &CapabilityToken,
    ) -> Result<usize, GraphError> {
        if !cap.rights.contains(CapRights::WRITE) {
            return Err(GraphError::InsufficientRights);
        }
        if self.edge_count >= self.config.max_edges {
            return Err(GraphError::PoolFull);
        }
        if edge.source.is_null() || edge.target.is_null() {
            return Err(GraphError::InvalidEdge);
        }

        let idx = self.edge_count;
        let source_id = edge.source;
        self.edges[idx] = edge;
        self.edge_count += 1;

        // Increment source node's edge count
        if let Some(node) = self.find_node_mut(source_id) {
            node.edge_count.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
        }

        Ok(idx)
    }

    /// Find edges originating from a node
    pub fn edges_from(&self, source: NodeId) -> impl Iterator<Item = &Edge> {
        self.edges[..self.edge_count]
            .iter()
            .filter(move |e| e.source == source)
    }

    /// Traverse an edge: execute the transformation
    pub fn traverse(
        &self,
        source: NodeId,
        target: NodeId,
        cap: &CapabilityToken,
    ) -> Result<TraversalResult, GraphError> {
        if !cap.rights.contains(CapRights::TRAVERSE) {
            return Err(GraphError::InsufficientRights);
        }

        let edge = self.edges[..self.edge_count]
            .iter()
            .find(|e| e.source == source && e.target == target)
            .ok_or(GraphError::EdgeNotFound)?;

        if !cap.satisfies(&edge.required_cap) {
            return Err(GraphError::InsufficientRights);
        }

        match edge.kind {
            crate::edge::EdgeKind::Reference => {
                Ok(TraversalResult::NodeReached(target))
            }
            crate::edge::EdgeKind::StaticFn => {
                // In bare-metal: call the function pointer
                // For now: just report traversal
                Ok(TraversalResult::FnExecuted {
                    target,
                    result_ptr: edge.payload,
                })
            }
            _ => Ok(TraversalResult::NodeReached(target)),
        }
    }

    /// Get node count
    pub fn node_count(&self) -> usize {
        self.node_count
    }

    /// Get edge count
    pub fn edge_count(&self) -> usize {
        self.edge_count
    }

    /// Find a node by ID — O(1) via hash table
    pub fn find_node(&self, id: NodeId) -> Option<&NodeHeader> {
        self.find_node_indexed(id).map(|(_, node)| node)
    }

    fn find_node_mut(&mut self, id: NodeId) -> Option<&mut NodeHeader> {
        self.nodes[..self.node_count]
            .iter_mut()
            .find(|n| n.id == id)
    }

    /// Get all edges of a specific kind from a node
    pub fn edges_from_by_kind(&self, source: NodeId, kind: EdgeKind) -> impl Iterator<Item = &Edge> {
        self.edges_from(source).filter(move |e| e.kind == kind)
    }

    /// Get the first (and presumably only) edge of a kind from a node.
    /// For unique relationships like CwdEdge.
    pub fn edge_singular(&self, source: NodeId, kind: EdgeKind) -> Option<&Edge> {
        self.edges_from_by_kind(source, kind).next()
    }

    /// Remove an edge between two nodes.
    /// Sets matched edge to tombstone so iteration skips it.
    pub fn disconnect(
        &mut self,
        source: NodeId,
        target: NodeId,
        cap: &CapabilityToken,
    ) -> Result<(), GraphError> {
        if !cap.rights.contains(CapRights::WRITE) {
            return Err(GraphError::InsufficientRights);
        }
        for i in 0..self.edge_count {
            let edge = &mut self.edges[i];
            if edge.source == source && edge.target == target {
                edge.source = NodeId::NULL;
                edge.target = NodeId::NULL;
                // Decrement source node's edge count
                if let Some(node) = self.find_node_mut(source) {
                    node.edge_count.fetch_sub(1, core::sync::atomic::Ordering::Relaxed);
                }
                return Ok(());
            }
        }
        Err(GraphError::EdgeNotFound)
    }

    /// Atomically change the target of a singular edge (e.g., move CwdEdge to new node).
    /// Finds the first edge of `kind` from `source` and changes its target.
    pub fn rehang_edge(
        &mut self,
        source: NodeId,
        kind: EdgeKind,
        new_target: NodeId,
        cap: &CapabilityToken,
    ) -> Result<(), GraphError> {
        if !cap.rights.contains(CapRights::WRITE) {
            return Err(GraphError::InsufficientRights);
        }
        for i in 0..self.edge_count {
            let edge = &mut self.edges[i];
            if edge.source == source && edge.kind == kind {
                edge.target = new_target;
                return Ok(());
            }
        }
        Err(GraphError::EdgeNotFound)
    }

    // ── Raw access for persistence layer ──────────────────────────────

    /// Return a slice over the active nodes (for serialization).
    pub fn nodes_slice(&self) -> &[NodeHeader] {
        &self.nodes[..self.node_count]
    }

    /// Return a slice over the active edges (for serialization).
    pub fn edges_slice(&self) -> &[Edge] {
        &self.edges[..self.edge_count]
    }

    /// Raw mutable access to the full node backing array.
    pub fn nodes_mut_raw(&mut self) -> &mut [NodeHeader] {
        self.nodes
    }

    /// Raw mutable access to the full edge backing array.
    pub fn edges_mut_raw(&mut self) -> &mut [Edge] {
        self.edges
    }

    /// Restore node/edge counts after a raw deserialization.
    ///
    /// # Safety
    /// Caller must guarantee that `node_count` nodes and `edge_count` edges
    /// have been written into the backing arrays with valid data.
    pub unsafe fn restore_raw(&mut self, node_count: usize, edge_count: usize) {
        self.node_count = node_count;
        self.edge_count = edge_count;
    }

    /// Resolve a path (sequence of edge name hashes) from a starting node.
    /// Each segment follows a named edge (payload == hash of name).
    pub fn resolve_path(
        &self,
        from: NodeId,
        segments: &[u64],
        cap: &CapabilityToken,
    ) -> Result<NodeId, GraphError> {
        if !cap.rights.contains(CapRights::TRAVERSE) {
            return Err(GraphError::InsufficientRights);
        }
        let mut current = from;
        for &name_hash in segments {
            let mut found = false;
            for edge in self.edges_from(current) {
                if edge.payload == name_hash {
                    current = edge.target;
                    found = true;
                    break;
                }
            }
            if !found {
                return Err(GraphError::PathNotFound);
            }
        }
        Ok(current)
    }

    // ── Index operations ─────────────────────────────────────────────────

    /// Insert a node ID into the hash table
    fn hash_insert(&mut self, id: NodeId, slab_index: u32) {
        let mut slot = hash_node_id(id);
        loop {
            if self.node_map[slot] == EMPTY_SLOT {
                self.node_map[slot] = slab_index;
                return;
            }
            slot = (slot + 1) & (HASH_TABLE_SIZE - 1);
        }
    }

    /// O(1) node lookup via hash table
    pub fn find_node_indexed(&self, id: NodeId) -> Option<(u32, &NodeHeader)> {
        let mut slot = hash_node_id(id);
        for _ in 0..HASH_TABLE_SIZE {
            let idx = self.node_map[slot];
            if idx == EMPTY_SLOT {
                return None;
            }
            if self.nodes[idx as usize].id == id {
                return Some((idx, &self.nodes[idx as usize]));
            }
            slot = (slot + 1) & (HASH_TABLE_SIZE - 1);
        }
        None
    }

    /// Get slab index for a node ID
    pub fn node_index(&self, id: NodeId) -> Option<u32> {
        self.find_node_indexed(id).map(|(idx, _)| idx)
    }

    /// Iterate edges from a node using slab_index (O(degree) filter)
    pub fn edges_from_indexed(&self, slab_index: u32) -> impl Iterator<Item = &Edge> {
        let source_id = if (slab_index as usize) < self.node_count {
            self.nodes[slab_index as usize].id
        } else {
            NodeId::NULL
        };
        self.edges[..self.edge_count]
            .iter()
            .filter(move |e| e.source == source_id)
    }
}
