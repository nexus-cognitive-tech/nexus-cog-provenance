//! Provenance query engine.

use nexus_cog_core::provenance::{ProvenanceQuery, ProvenanceRecord, ProvenanceSource};

use crate::graph::ProvenanceGraphEngine;

/// Query the provenance graph with structured filters.
#[derive(Debug, Clone)]
pub struct ProvenanceQueryEngine {
    engine: ProvenanceGraphEngine,
}

impl ProvenanceQueryEngine {
    /// Construct a query engine for the given graph.
    #[must_use]
    pub fn new(engine: ProvenanceGraphEngine) -> Self {
        Self { engine }
    }

    /// Find all records for an artifact.
    #[must_use]
    pub fn for_artifact(&self, artifact: &str) -> Vec<ProvenanceRecord> {
        self.engine.records_for_artifact(artifact)
    }

    /// Find records by source type.
    #[must_use]
    pub fn by_source(&self, source: ProvenanceSource) -> Vec<ProvenanceRecord> {
        self.engine.records_with_source(source)
    }

    /// Find records by origin (e.g. specific tool or model).
    #[must_use]
    pub fn by_origin(&self, origin: &str) -> Vec<ProvenanceRecord> {
        self.engine
            .records()
            .into_iter()
            .filter(|r| r.origin == origin)
            .collect()
    }

    /// Free-text search across artifact / origin / content / prompt.
    #[must_use]
    pub fn search(&self, query: &str) -> Vec<ProvenanceRecord> {
        let q = query.to_lowercase();
        self.engine
            .records()
            .into_iter()
            .filter(|r| {
                r.artifact.to_lowercase().contains(&q)
                    || r.origin.to_lowercase().contains(&q)
                    || r.content.to_lowercase().contains(&q)
                    || r.prompt.to_lowercase().contains(&q)
            })
            .collect()
    }

    /// Run a structured query and produce a [`ProvenanceQuery`] bundle.
    #[must_use]
    pub fn run(&self, description: &str, results: Vec<ProvenanceRecord>) -> ProvenanceQuery {
        ProvenanceQuery {
            description: description.to_string(),
            records: results,
            scanned: self.engine.len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_cog_core::provenance::ProvenanceSource;

    fn rec(artifact: &str, origin: &str, source: ProvenanceSource) -> ProvenanceRecord {
        let mut r = ProvenanceRecord::new(artifact, source, origin, "p", "c", "g");
        r.id = uuid::Uuid::new_v4().to_string();
        r
    }

    #[test]
    fn search_finds_by_artifact() {
        let mut e = ProvenanceGraphEngine::new();
        e.add_record(rec("src/lib.rs", "claude", ProvenanceSource::ModelOutput));
        e.add_record(rec("src/main.rs", "claude", ProvenanceSource::ModelOutput));
        e.add_record(rec("Cargo.toml", "tool", ProvenanceSource::ToolExecution));
        let q = ProvenanceQueryEngine::new(e);
        let r = q.search("lib.rs");
        assert_eq!(r.len(), 1);
    }

    #[test]
    fn by_source_filters() {
        let mut e = ProvenanceGraphEngine::new();
        e.add_record(rec("a", "model", ProvenanceSource::ModelOutput));
        e.add_record(rec("b", "tool", ProvenanceSource::ToolExecution));
        let q = ProvenanceQueryEngine::new(e);
        let r = q.by_source(ProvenanceSource::ModelOutput);
        assert_eq!(r.len(), 1);
    }
}
