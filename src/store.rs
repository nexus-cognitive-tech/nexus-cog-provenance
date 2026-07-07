//! Persistence for the provenance graph.

use std::path::Path;

#[cfg(test)]
use nexus_cog_core::provenance::ProvenanceRecord;

use crate::graph::ProvenanceGraphEngine;

/// JSON-backed provenance store.
#[derive(Debug, Clone, Default)]
pub struct ProvenanceStore;

impl ProvenanceStore {
    /// Construct a new store.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Save the graph to a JSON file.
    pub fn save(&self, engine: &ProvenanceGraphEngine, path: impl AsRef<Path>) -> Result<(), String> {
        let snap = engine.snapshot();
        let json = serde_json::to_string_pretty(&snap).map_err(|e| e.to_string())?;
        std::fs::write(path, json).map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Load the graph from a JSON file.
    pub fn load(&self, path: impl AsRef<Path>) -> Result<ProvenanceGraphEngine, String> {
        let json = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let graph: nexus_cog_core::provenance::ProvenanceGraph = serde_json::from_str(&json).map_err(|e| e.to_string())?;
        Ok(ProvenanceGraphEngine::from_graph(graph))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_cog_core::provenance::ProvenanceSource;
    use tempfile::TempDir;

    #[test]
    fn save_load_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("prov.json");
        let mut e = ProvenanceGraphEngine::new();
        e.add_record(ProvenanceRecord::new("f.rs", ProvenanceSource::ModelOutput, "model", "p", "c", "g"));
        let store = ProvenanceStore::new();
        store.save(&e, &path).unwrap();
        let loaded = store.load(&path).unwrap();
        assert_eq!(loaded.len(), 1);
    }
}
