//! Provenance graph engine: maintains the in-memory graph and supports traversal.

use std::sync::Arc;

use nexus_cog_core::provenance::{ProvenanceEdge, ProvenanceEdgeType, ProvenanceGraph, ProvenanceRecord, ProvenanceSource};
use indexmap::IndexMap;
use parking_lot::RwLock;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;

/// Inner mutable state of [`ProvenanceGraphEngine`].
#[derive(Debug)]
struct Inner {
    graph: DiGraph<ProvenanceRecord, ProvenanceEdgeType>,
    index: IndexMap<String, NodeIndex>,
}

/// Manages a [`ProvenanceGraph`] backed by [`petgraph`] for efficient traversal.
///
/// Cloning is cheap (Arc clone). Mutations use interior mutability via
/// [`parking_lot::RwLock`], so query engines and other readers can hold their
/// own snapshot without lifetime ties.
#[derive(Debug, Clone)]
pub struct ProvenanceGraphEngine {
    inner: Arc<RwLock<Inner>>,
}

impl Default for ProvenanceGraphEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ProvenanceGraphEngine {
    /// Construct an empty engine.
        pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(Inner {
                graph: DiGraph::new(),
                index: IndexMap::new(),
            })),
        }
    }

    /// Construct from an existing graph snapshot.
        pub fn from_graph(graph: ProvenanceGraph) -> Self {
        let engine = Self::new();
        {
            let mut inner = engine.inner.write();
            for record in graph.records {
                let id = record.id.clone();
                let idx = inner.graph.add_node(record);
                inner.index.insert(id, idx);
            }
            for edge in graph.edges {
                let Some(from_idx) = inner.index.get(&edge.from).copied() else { continue };
                let Some(to_idx) = inner.index.get(&edge.to).copied() else { continue };
                inner.graph.add_edge(from_idx, to_idx, edge.edge_type);
            }
        }
        engine
    }

    /// Number of records.
        pub fn len(&self) -> usize {
        self.inner.read().graph.node_count()
    }

    /// Returns `true` if there are no records.
        pub fn is_empty(&self) -> bool {
        self.inner.read().graph.node_count() == 0
    }

    /// Add a record to the graph.
    pub fn add_record(&mut self, record: ProvenanceRecord) -> NodeIndex {
        let id = record.id.clone();
        let mut inner = self.inner.write();
        let idx = inner.graph.add_node(record);
        inner.index.insert(id, idx);
        idx
    }

    /// Add an edge between two records by ID.
    pub fn add_edge_by_id(&mut self, from: &str, to: &str, edge_type: ProvenanceEdgeType) -> bool {
        let mut inner = self.inner.write();
        let Some(from_idx) = inner.index.get(from).copied() else { return false };
        let Some(to_idx) = inner.index.get(to).copied() else { return false };
        inner.graph.add_edge(from_idx, to_idx, edge_type);
        true
    }

    /// Get a record by ID.
        pub fn get(&self, id: &str) -> Option<ProvenanceRecord> {
        let inner = self.inner.read();
        inner.index.get(id).and_then(|idx| inner.graph.node_weight(*idx).cloned())
    }

    /// Records for a specific artifact.
        pub fn records_for_artifact(&self, artifact: &str) -> Vec<ProvenanceRecord> {
        let inner = self.inner.read();
        inner
            .graph
            .node_indices()
            .filter_map(|idx| inner.graph.node_weight(idx).cloned())
            .filter(|r| r.artifact == artifact)
            .collect()
    }

    /// Records produced by a specific source type.
        pub fn records_with_source(&self, source: ProvenanceSource) -> Vec<ProvenanceRecord> {
        let inner = self.inner.read();
        inner
            .graph
            .node_indices()
            .filter_map(|idx| inner.graph.node_weight(idx).cloned())
            .filter(|r| r.source == source)
            .collect()
    }

    /// All records (cloned).
        pub fn records(&self) -> Vec<ProvenanceRecord> {
        let inner = self.inner.read();
        inner
            .graph
            .node_indices()
            .filter_map(|idx| inner.graph.node_weight(idx).cloned())
            .collect()
    }

    /// Direct children of a record (records that were produced from this one).
        pub fn children_of(&self, id: &str) -> Vec<ProvenanceRecord> {
        let inner = self.inner.read();
        let Some(&idx) = inner.index.get(id) else { return Vec::new() };
        inner
            .graph
            .edges(idx)
            .filter_map(|e| inner.graph.node_weight(e.target()).cloned())
            .collect()
    }

    /// Direct parents of a record (records that produced this one).
        pub fn parents_of(&self, id: &str) -> Vec<ProvenanceRecord> {
        let inner = self.inner.read();
        let Some(&idx) = inner.index.get(id) else { return Vec::new() };
        inner
            .graph
            .edges_directed(idx, petgraph::Direction::Incoming)
            .filter_map(|e| inner.graph.node_weight(e.source()).cloned())
            .collect()
    }

    /// All ancestors (transitive parents) of a record, excluding the record itself.
        pub fn ancestors_of(&self, id: &str) -> Vec<ProvenanceRecord> {
        let inner = self.inner.read();
        let Some(&start) = inner.index.get(id) else { return Vec::new() };
        let mut visited = std::collections::HashSet::new();
        visited.insert(start);
        let mut order = Vec::new();
        let mut stack = vec![start];
        while let Some(idx) = stack.pop() {
            for edge in inner.graph.edges_directed(idx, petgraph::Direction::Incoming) {
                if visited.insert(edge.source()) {
                    order.push(edge.source());
                    stack.push(edge.source());
                }
            }
        }
        order.into_iter().filter_map(|idx| inner.graph.node_weight(idx).cloned()).collect()
    }

    /// All descendants (transitive children) of a record, excluding the record itself.
        pub fn descendants_of(&self, id: &str) -> Vec<ProvenanceRecord> {
        let inner = self.inner.read();
        let Some(&start) = inner.index.get(id) else { return Vec::new() };
        let mut visited = std::collections::HashSet::new();
        visited.insert(start);
        let mut order = Vec::new();
        let mut stack = vec![start];
        while let Some(idx) = stack.pop() {
            for edge in inner.graph.edges_directed(idx, petgraph::Direction::Outgoing) {
                if visited.insert(edge.target()) {
                    order.push(edge.target());
                    stack.push(edge.target());
                }
            }
        }
        order.into_iter().filter_map(|idx| inner.graph.node_weight(idx).cloned()).collect()
    }

    /// Snapshot the engine into a serializable graph.
        pub fn snapshot(&self) -> ProvenanceGraph {
        let inner = self.inner.read();
        let records: Vec<ProvenanceRecord> = inner
            .graph
            .node_indices()
            .filter_map(|idx| inner.graph.node_weight(idx).cloned())
            .collect();
        let mut edges = Vec::new();
        for edge_ref in inner.graph.edge_references() {
            let from = inner.graph.node_weight(edge_ref.source()).unwrap();
            let to = inner.graph.node_weight(edge_ref.target()).unwrap();
            edges.push(ProvenanceEdge {
                from: from.id.clone(),
                to: to.id.clone(),
                edge_type: *edge_ref.weight(),
                notes: String::new(),
            });
        }
        ProvenanceGraph {
            records,
            edges,
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now(),
            name: String::new(),
        }
    }

    /// Iterator over node indices (exposed for query engines). Returns an owned vec.
        pub fn graph_indices(&self) -> Vec<NodeIndex> {
        self.inner.read().graph.node_indices().collect()
    }

    /// Get the weight (record) at a node index (exposed for query engines).
        pub fn graph_weight(&self, idx: NodeIndex) -> Option<ProvenanceRecord> {
        self.inner.read().graph.node_weight(idx).cloned()
    }

    /// Acquire a read guard for advanced traversal.
        pub fn read(&self) -> ProvenanceGraphReadGuard<'_> {
        ProvenanceGraphReadGuard { inner: self.inner.read() }
    }
}

/// Read-only guard over a [`ProvenanceGraphEngine`] for advanced traversal.
pub struct ProvenanceGraphReadGuard<'a> {
    inner: parking_lot::RwLockReadGuard<'a, Inner>,
}

impl<'a> ProvenanceGraphReadGuard<'a> {
    /// Direct access to the underlying petgraph.
        pub fn graph(&self) -> &DiGraph<ProvenanceRecord, ProvenanceEdgeType> {
        &self.inner.graph
    }

    /// Iterator over all node indices.
        pub fn node_indices(&self) -> impl Iterator<Item = NodeIndex> + '_ {
        self.inner.graph.node_indices()
    }

    /// Get the weight (record) at a node index.
        pub fn graph_weight(&self, idx: NodeIndex) -> Option<&ProvenanceRecord> {
        self.inner.graph.node_weight(idx)
    }

    /// Look up the internal node index for a record ID.
        pub fn index_of(&self, id: &str) -> Option<NodeIndex> {
        self.inner.index.get(id).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(id: &str, artifact: &str) -> ProvenanceRecord {
        let mut r = ProvenanceRecord::new(artifact, ProvenanceSource::ModelOutput, "model", "p", "c", "g");
        r.id = id.to_string();
        r
    }

    #[test]
    fn add_and_get() {
        let mut e = ProvenanceGraphEngine::new();
        let idx = e.add_record(rec("a", "file.rs"));
        assert!(idx.index() < 1_000_000);
        assert!(e.get("a").is_some());
    }

    #[test]
    fn edge_connects_records() {
        let mut e = ProvenanceGraphEngine::new();
        e.add_record(rec("a", "f.rs"));
        e.add_record(rec("b", "f.rs"));
        assert!(e.add_edge_by_id("a", "b", ProvenanceEdgeType::ProducedBy));
        let children = e.children_of("a");
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].id, "b");
    }

    #[test]
    fn ancestors_walk_transitively() {
        let mut e = ProvenanceGraphEngine::new();
        e.add_record(rec("a", "f"));
        e.add_record(rec("b", "f"));
        e.add_record(rec("c", "f"));
        e.add_edge_by_id("a", "b", ProvenanceEdgeType::ProducedBy);
        e.add_edge_by_id("b", "c", ProvenanceEdgeType::ProducedBy);
        let ancestors = e.ancestors_of("c");
        assert!(ancestors.iter().any(|r| r.id == "a"));
        assert!(ancestors.iter().any(|r| r.id == "b"));
    }

    #[test]
    fn snapshot_roundtrip() {
        let mut e = ProvenanceGraphEngine::new();
        e.add_record(rec("a", "f"));
        e.add_record(rec("b", "f"));
        e.add_edge_by_id("a", "b", ProvenanceEdgeType::ProducedBy);
        let snap = e.snapshot();
        let e2 = ProvenanceGraphEngine::from_graph(snap);
        assert_eq!(e2.len(), 2);
    }

    #[test]
    fn clone_shares_state() {
        let mut e = ProvenanceGraphEngine::new();
        e.add_record(rec("a", "f"));
        let e2 = e.clone();
        assert_eq!(e2.len(), 1);
        assert!(e2.get("a").is_some());
    }
}
