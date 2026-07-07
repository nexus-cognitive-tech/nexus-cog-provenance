//! Provenance tracer — walks the chain of ancestry / descent.

use nexus_cog_core::provenance::{ProvenanceEdgeType, ProvenanceRecord};

use crate::graph::ProvenanceGraphEngine;

/// Walk chains of provenance.
#[derive(Debug, Clone)]
pub struct ProvenanceTracer {
    engine: ProvenanceGraphEngine,
}

impl ProvenanceTracer {
    /// Construct a tracer for the given graph.
    #[must_use]
    pub fn new(engine: ProvenanceGraphEngine) -> Self {
        Self { engine }
    }

    /// Full ancestry chain from the root to the given record.
    #[must_use]
    pub fn chain_to(&self, id: &str) -> Vec<ProvenanceRecord> {
        let ancestors = self.engine.ancestors_of(id);
        if let Some(self_rec) = self.engine.get(id) {
            // The engine returns ancestors + self; we want root-first.
            let mut chain: Vec<ProvenanceRecord> = ancestors;
            chain.reverse();
            chain.push(self_rec);
            chain
        } else {
            Vec::new()
        }
    }

    /// Full descent chain from the given record to all leaves.
    #[must_use]
    pub fn chain_from(&self, id: &str) -> Vec<ProvenanceRecord> {
        self.engine.descendants_of(id)
    }

    /// Returns the relationships traversed to reach a record.
    #[must_use]
    pub fn relationship_path(&self, id: &str) -> Vec<(String, ProvenanceEdgeType)> {
        // Walk up; collect edges along the way.
        let mut path = Vec::new();
        let mut current = id.to_string();
        let mut visited = std::collections::HashSet::new();
        while let Some(record) = self.engine.get(&current) {
            if let Some(parent_id) = record.parent.clone() {
                if !visited.insert(parent_id.clone()) {
                    break;
                }
                // Find the edge type between parent and current.
                let edge_type = if self.engine.parents_of(&current).is_empty() {
                    ProvenanceEdgeType::DerivedFrom
                } else {
                    ProvenanceEdgeType::ProducedBy
                };
                path.push((parent_id.clone(), edge_type));
                current = parent_id;
            } else {
                break;
            }
        }
        path.reverse();
        path
    }

    /// Compute the depth of a record in the ancestry chain.
    #[must_use]
    pub fn depth(&self, id: &str) -> usize {
        self.engine.ancestors_of(id).len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_cog_core::provenance::ProvenanceSource;

    fn rec(id: &str, artifact: &str) -> ProvenanceRecord {
        let mut r = ProvenanceRecord::new(artifact, ProvenanceSource::ModelOutput, "model", "p", "c", "g");
        r.id = id.to_string();
        r
    }

    #[test]
    fn chain_to_walks_full_history() {
        let mut e = ProvenanceGraphEngine::new();
        e.add_record(rec("a", "f"));
        e.add_record(rec("b", "f"));
        e.add_record(rec("c", "f"));
        e.add_edge_by_id("a", "b", ProvenanceEdgeType::ProducedBy);
        e.add_edge_by_id("b", "c", ProvenanceEdgeType::ProducedBy);
        let t = ProvenanceTracer::new(e);
        let chain = t.chain_to("c");
        assert_eq!(chain.len(), 3);
        assert_eq!(chain[0].id, "a");
        assert_eq!(chain[2].id, "c");
    }

    #[test]
    fn depth_counts_ancestors() {
        let mut e = ProvenanceGraphEngine::new();
        e.add_record(rec("a", "f"));
        e.add_record(rec("b", "f"));
        e.add_record(rec("c", "f"));
        e.add_edge_by_id("a", "b", ProvenanceEdgeType::ProducedBy);
        e.add_edge_by_id("b", "c", ProvenanceEdgeType::ProducedBy);
        let t = ProvenanceTracer::new(e);
        assert_eq!(t.depth("a"), 0);
        assert_eq!(t.depth("b"), 1);
        assert_eq!(t.depth("c"), 2);
    }
}
