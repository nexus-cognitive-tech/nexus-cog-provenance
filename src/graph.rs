//! Provenance graph engine, persisted via [`nexus-cog-storage`].

use std::sync::Arc;

use indexmap::IndexMap;
use nexus_cog_core::provenance::{
    ProvenanceEdgeType, ProvenanceGraph, ProvenanceRecord, ProvenanceSource,
};
use nexus_cog_storage::{PersistenceBackend, SqliteBackend, StorageResult};
use parking_lot::RwLock;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;

#[derive(Debug)]
struct Inner {
    graph: DiGraph<ProvenanceRecord, ProvenanceEdgeType>,
    index: IndexMap<String, NodeIndex>,
}

#[derive(Clone)]
pub struct ProvenanceGraphEngine {
    backend: Arc<dyn PersistenceBackend>,
    inner: Arc<RwLock<Inner>>,
}

impl std::fmt::Debug for ProvenanceGraphEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProvenanceGraphEngine")
            .field("backend", &self.backend.describe())
            .field("records", &self.len())
            .finish()
    }
}

impl ProvenanceGraphEngine {
    /// Construct an engine backed by `backend`.
    pub fn with_backend(backend: Arc<dyn PersistenceBackend>) -> StorageResult<Self> {
        super::schema::register(backend.as_ref())?;
        let records = super::schema::load_all_records(backend.as_ref())?;
        let edges = super::schema::load_all_edges(backend.as_ref())?;
        let mut graph: DiGraph<ProvenanceRecord, ProvenanceEdgeType> = DiGraph::new();
        let mut index: IndexMap<String, NodeIndex> = IndexMap::new();
        for r in records {
            let id = r.id.clone();
            let idx = graph.add_node(r);
            index.insert(id, idx);
        }
        for e in edges {
            let Some(&from) = index.get(&e.from) else { continue };
            let Some(&to) = index.get(&e.to) else { continue };
            graph.add_edge(from, to, e.edge_type);
        }
        Ok(Self {
            backend,
            inner: Arc::new(RwLock::new(Inner { graph, index })),
        })
    }

    /// Convenience constructor for in-memory SQLite.
    pub fn in_memory() -> StorageResult<Self> {
        Self::with_backend(Arc::new(SqliteBackend::open_in_memory()?))
    }

    /// Backend description.
    #[must_use]
    pub fn backend_info(&self) -> String {
        self.backend.describe()
    }

    /// Number of records.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.read().graph.node_count()
    }

    /// Returns `true` if there are no records.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.read().graph.node_count() == 0
    }

    /// Add a record. Persists to backend first; in-memory cache is updated
    /// only after the SQL write succeeds.
    pub fn add_record(&self, record: ProvenanceRecord) -> StorageResult<NodeIndex> {
        super::schema::upsert_record(self.backend.as_ref(), &record)?;
        let id = record.id.clone();
        let mut inner = self.inner.write();
        let idx = inner.graph.add_node(record);
        inner.index.insert(id, idx);
        Ok(idx)
    }

    /// Add an edge between two existing records by ID. Returns `false` if
    /// either endpoint is missing.
    pub fn add_edge_by_id(
        &self,
        from: &str,
        to: &str,
        edge_type: ProvenanceEdgeType,
    ) -> StorageResult<bool> {
        {
            let inner = self.inner.read();
            if inner.index.get(from).is_none() || inner.index.get(to).is_none() {
                return Ok(false);
            }
        }
        super::schema::upsert_edge(self.backend.as_ref(), from, to, edge_type)?;
        let mut inner = self.inner.write();
        let Some(&from_idx) = inner.index.get(from) else { return Ok(false) };
        let Some(&to_idx) = inner.index.get(to) else { return Ok(false) };
        if let Some(existing) = inner.graph.find_edge(from_idx, to_idx) {
            inner.graph.remove_edge(existing);
        }
        inner.graph.add_edge(from_idx, to_idx, edge_type);
        Ok(true)
    }

    /// Get a record by ID.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<ProvenanceRecord> {
        let inner = self.inner.read();
        inner.index.get(id).and_then(|idx| inner.graph.node_weight(*idx).cloned())
    }

    /// Records for a specific artifact.
    #[must_use]
    pub fn records_for_artifact(&self, artifact: &str) -> Vec<ProvenanceRecord> {
        self.inner
            .read()
            .graph
            .node_indices()
            .filter_map(|idx| self.inner.read().graph.node_weight(idx).cloned())
            .filter(|r| r.artifact == artifact)
            .collect()
    }

    /// Records produced by a specific source type.
    #[must_use]
    pub fn records_with_source(&self, source: ProvenanceSource) -> Vec<ProvenanceRecord> {
        self.inner
            .read()
            .graph
            .node_indices()
            .filter_map(|idx| self.inner.read().graph.node_weight(idx).cloned())
            .filter(|r| r.source == source)
            .collect()
    }

    /// All records (cloned).
    #[must_use]
    pub fn records(&self) -> Vec<ProvenanceRecord> {
        self.inner
            .read()
            .graph
            .node_indices()
            .filter_map(|idx| self.inner.read().graph.node_weight(idx).cloned())
            .collect()
    }

    /// Direct children of a record.
    #[must_use]
    pub fn children_of(&self, id: &str) -> Vec<ProvenanceRecord> {
        let inner = self.inner.read();
        let Some(&idx) = inner.index.get(id) else { return Vec::new() };
        inner
            .graph
            .edges(idx)
            .filter_map(|e| inner.graph.node_weight(e.target()).cloned())
            .collect()
    }

    /// Direct parents of a record.
    #[must_use]
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
    #[must_use]
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
    #[must_use]
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

    /// Snapshot the in-memory cache.
    #[must_use]
    pub fn snapshot(&self) -> ProvenanceGraph {
        let inner = self.inner.read();
        let records: Vec<ProvenanceRecord> = inner
            .graph
            .node_indices()
            .filter_map(|idx| inner.graph.node_weight(idx).cloned())
            .collect();
        let edges: Vec<nexus_cog_core::provenance::ProvenanceEdge> = inner
            .graph
            .edge_references()
            .map(|e| nexus_cog_core::provenance::ProvenanceEdge {
                from: inner.graph.node_weight(e.source()).unwrap().id.clone(),
                to: inner.graph.node_weight(e.target()).unwrap().id.clone(),
                edge_type: *e.weight(),
                notes: String::new(),
            })
            .collect();
        ProvenanceGraph {
            records,
            edges,
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now(),
            name: String::new(),
        }
    }

    /// Iterator over node indices (exposed for query engines).
    #[must_use]
    pub fn graph_indices(&self) -> Vec<NodeIndex> {
        self.inner.read().graph.node_indices().collect()
    }

    /// Get the weight (record) at a node index.
    #[must_use]
    pub fn graph_weight(&self, idx: NodeIndex) -> Option<ProvenanceRecord> {
        self.inner.read().graph.node_weight(idx).cloned()
    }

    /// Acquire a read guard for advanced traversal.
    #[must_use]
    pub fn read(&self) -> ProvenanceGraphReadGuard<'_> {
        ProvenanceGraphReadGuard { inner: self.inner.read() }
    }
}

pub struct ProvenanceGraphReadGuard<'a> {
    inner: parking_lot::RwLockReadGuard<'a, Inner>,
}

impl<'a> ProvenanceGraphReadGuard<'a> {
    #[must_use]
    pub fn graph(&self) -> &DiGraph<ProvenanceRecord, ProvenanceEdgeType> {
        &self.inner.graph
    }
    #[must_use]
    pub fn node_indices(&self) -> impl Iterator<Item = NodeIndex> + '_ {
        self.inner.graph.node_indices()
    }
    #[must_use]
    pub fn graph_weight(&self, idx: NodeIndex) -> Option<&ProvenanceRecord> {
        self.inner.graph.node_weight(idx)
    }
    #[must_use]
    pub fn index_of(&self, id: &str) -> Option<NodeIndex> {
        self.inner.index.get(id).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_cog_core::provenance::{ProvenanceEdgeType, ProvenanceRecord, ProvenanceSource};

    fn rec(id: &str, artifact: &str) -> ProvenanceRecord {
        let mut r = ProvenanceRecord::new(artifact, ProvenanceSource::ModelOutput, "model", "p", "c", "g");
        r.id = id.to_string();
        r
    }

    #[test]
    fn add_and_get() {
        let e = ProvenanceGraphEngine::in_memory().unwrap();
        let _idx = e.add_record(rec("a", "file.rs")).unwrap();
        assert!(e.get("a").is_some());
    }

    #[test]
    fn edge_connects_records() {
        let e = ProvenanceGraphEngine::in_memory().unwrap();
        e.add_record(rec("a", "f.rs")).unwrap();
        e.add_record(rec("b", "f.rs")).unwrap();
        let ok = e.add_edge_by_id("a", "b", ProvenanceEdgeType::ProducedBy).unwrap();
        assert!(ok);
        let children = e.children_of("a");
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].id, "b");
    }

    #[test]
    fn ancestors_walk_transitively() {
        let e = ProvenanceGraphEngine::in_memory().unwrap();
        e.add_record(rec("a", "f")).unwrap();
        e.add_record(rec("b", "f")).unwrap();
        e.add_record(rec("c", "f")).unwrap();
        e.add_edge_by_id("a", "b", ProvenanceEdgeType::ProducedBy).unwrap();
        e.add_edge_by_id("b", "c", ProvenanceEdgeType::ProducedBy).unwrap();
        let ancestors = e.ancestors_of("c");
        assert!(ancestors.iter().any(|r| r.id == "a"));
        assert!(ancestors.iter().any(|r| r.id == "b"));
    }

    #[test]
    fn persists_across_reopen() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("prov.db");
        {
            let backend = Arc::new(SqliteBackend::open(&path).unwrap());
            let e = ProvenanceGraphEngine::with_backend(backend).unwrap();
            e.add_record(rec("a", "f")).unwrap();
            e.add_record(rec("b", "f")).unwrap();
            e.add_edge_by_id("a", "b", ProvenanceEdgeType::DerivedFrom).unwrap();
        }
        let backend = Arc::new(SqliteBackend::open(&path).unwrap());
        let e = ProvenanceGraphEngine::with_backend(backend).unwrap();
        assert_eq!(e.len(), 2);
        let snap = e.snapshot();
        assert_eq!(snap.edges[0].edge_type, ProvenanceEdgeType::DerivedFrom);
    }
}
