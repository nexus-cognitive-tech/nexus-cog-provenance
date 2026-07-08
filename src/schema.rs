//! SQL schema for the provenance engine.

use nexus_cog_core::provenance::{
    ProvenanceEdge, ProvenanceEdgeType, ProvenanceGraph, ProvenanceRecord, ProvenanceSource,
};
use nexus_cog_storage::{PersistenceBackend, SqlValue, StorageResult};

pub const OWNER: &str = "nexus_cog_provenance";
pub const SCHEMA_VERSION: i32 = 1;

pub const SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS provenance_records (
    id           TEXT PRIMARY KEY,
    artifact     TEXT NOT NULL,
    source       TEXT NOT NULL,
    origin       TEXT NOT NULL DEFAULT '',
    parent       TEXT NOT NULL DEFAULT '',
    children     TEXT NOT NULL DEFAULT '',
    prompt       TEXT NOT NULL DEFAULT '',
    content      TEXT NOT NULL DEFAULT '',
    content_hash TEXT NOT NULL DEFAULT '',
    agent        TEXT NOT NULL DEFAULT '',
    confidence   REAL NOT NULL DEFAULT 1.0,
    timestamp    INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS provenance_records_artifact_idx ON provenance_records(artifact);
CREATE INDEX IF NOT EXISTS provenance_records_source_idx   ON provenance_records(source);

CREATE TABLE IF NOT EXISTS provenance_edges (
    from_record TEXT NOT NULL REFERENCES provenance_records(id) ON DELETE CASCADE,
    to_record   TEXT NOT NULL REFERENCES provenance_records(id) ON DELETE CASCADE,
    edge_type   TEXT NOT NULL,
    PRIMARY KEY (from_record, to_record),
    CHECK (from_record <> to_record)
);
"#;

pub fn register(backend: &dyn PersistenceBackend) -> StorageResult<()> {
    backend.apply_migrations(OWNER, SCHEMA_VERSION, SCHEMA_SQL)
}

pub fn upsert_record(backend: &dyn PersistenceBackend, r: &ProvenanceRecord) -> StorageResult<()> {
    let parent = r.parent.clone().unwrap_or_default();
    let children_joined = r.children.join(",");
    backend.exec(
        "INSERT INTO provenance_records (id, artifact, source, origin, parent, children, prompt, content, content_hash, agent, confidence, timestamp) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12) \
         ON CONFLICT(id) DO UPDATE SET \
             artifact = excluded.artifact, source = excluded.source, origin = excluded.origin, \
             parent = excluded.parent, children = excluded.children, \
             prompt = excluded.prompt, content = excluded.content, content_hash = excluded.content_hash, \
             agent = excluded.agent, confidence = excluded.confidence, timestamp = excluded.timestamp",
        &[
            SqlValue::text(&r.id),
            SqlValue::text(&r.artifact),
            SqlValue::text(source_id(r.source)),
            SqlValue::text(&r.origin),
            SqlValue::text(parent),
            SqlValue::text(children_joined),
            SqlValue::text(&r.prompt),
            SqlValue::text(&r.content),
            SqlValue::text(&r.content_hash),
            SqlValue::text(&r.agent),
            SqlValue::real(r.confidence.value() as f64),
            SqlValue::int(r.timestamp),
        ],
    )?;
    Ok(())
}

pub fn upsert_edge(
    backend: &dyn PersistenceBackend,
    from: &str,
    to: &str,
    ty: ProvenanceEdgeType,
) -> StorageResult<()> {
    backend.exec(
        "INSERT INTO provenance_edges (from_record, to_record, edge_type) VALUES (?1, ?2, ?3) \
         ON CONFLICT(from_record, to_record) DO UPDATE SET edge_type = excluded.edge_type",
        &[
            SqlValue::text(from),
            SqlValue::text(to),
            SqlValue::text(edge_type_id(ty)),
        ],
    )?;
    Ok(())
}

pub fn load_all_records(backend: &dyn PersistenceBackend) -> StorageResult<Vec<ProvenanceRecord>> {
    let rows = backend.fetch_all(
        "SELECT id, artifact, source, origin, parent, children, prompt, content, content_hash, agent, confidence, timestamp \
         FROM provenance_records ORDER BY timestamp, id",
        &[],
    )?;
    rows.into_iter().map(row_to_record).collect()
}

pub fn load_all_edges(backend: &dyn PersistenceBackend) -> StorageResult<Vec<ProvenanceEdge>> {
    let rows = backend.fetch_all(
        "SELECT from_record, to_record, edge_type FROM provenance_edges ORDER BY from_record, to_record",
        &[],
    )?;
    rows.into_iter().map(row_to_edge).collect()
}

fn row_to_record(row: Vec<SqlValue>) -> StorageResult<ProvenanceRecord> {
    let get_str = |i: usize| row.get(i).and_then(|v| v.as_str()).map(String::from);
    let get_f64 = |i: usize| row.get(i).and_then(|v| v.as_f64()).unwrap_or(1.0);
    let get_i64 = |i: usize| row.get(i).and_then(|v| v.as_i64()).unwrap_or(0);
    let parent = get_str(4).filter(|s| !s.is_empty());
    let children_csv = get_str(5).unwrap_or_default();
    let children: Vec<String> = if children_csv.is_empty() {
        Vec::new()
    } else {
        children_csv.split(',').map(|s| s.to_string()).collect()
    };
    Ok(ProvenanceRecord {
        id: get_str(0).unwrap_or_default(),
        artifact: get_str(1).unwrap_or_default(),
        source: parse_source(&get_str(2).unwrap_or_else(|| "model_output".into()))
            .unwrap_or(ProvenanceSource::ModelOutput),
        origin: get_str(3).unwrap_or_default(),
        parent,
        children,
        prompt: get_str(6).unwrap_or_default(),
        content: get_str(7).unwrap_or_default(),
        content_hash: get_str(8).unwrap_or_default(),
        agent: get_str(9).unwrap_or_default(),
        location: None,
        confidence: nexus_cog_core::common::Confidence::new(get_f64(10) as f32),
        timestamp: get_i64(11),
        metadata: Default::default(),
    })
}

fn row_to_edge(row: Vec<SqlValue>) -> StorageResult<ProvenanceEdge> {
    let get_str = |i: usize| row.get(i).and_then(|v| v.as_str()).map(String::from);
    Ok(ProvenanceEdge {
        from: get_str(0).unwrap_or_default(),
        to: get_str(1).unwrap_or_default(),
        edge_type: parse_edge_type(&get_str(2).unwrap_or_else(|| "produced_by".into()))
            .unwrap_or(ProvenanceEdgeType::ProducedBy),
        notes: String::new(),
    })
}

fn source_id(s: ProvenanceSource) -> &'static str {
    use ProvenanceSource::*;
    match s {
        ModelOutput => "model_output",
        ToolExecution => "tool_execution",
        TestRun => "test_run",
        UserInput => "user_input",
        Reasoning => "reasoning",
        CodeExtraction => "code_extraction",
        FileLoad => "file_load",
        Composition => "composition",
        Inference => "inference",
    }
}

fn parse_source(s: &str) -> Option<ProvenanceSource> {
    use ProvenanceSource::*;
    Some(match s {
        "model_output" => ModelOutput,
        "tool_execution" => ToolExecution,
        "test_run" => TestRun,
        "user_input" => UserInput,
        "reasoning" => Reasoning,
        "code_extraction" => CodeExtraction,
        "file_load" => FileLoad,
        "composition" => Composition,
        "inference" => Inference,
        _ => return None,
    })
}

fn edge_type_id(t: ProvenanceEdgeType) -> &'static str {
    use ProvenanceEdgeType::*;
    match t {
        ProducedBy => "produced_by",
        DerivedFrom => "derived_from",
        RefactoredFrom => "refactored_from",
        TestsAgainst => "tests_against",
        Sibling => "sibling",
        Documents => "documents",
    }
}

fn parse_edge_type(s: &str) -> Option<ProvenanceEdgeType> {
    use ProvenanceEdgeType::*;
    Some(match s {
        "produced_by" => ProducedBy,
        "derived_from" => DerivedFrom,
        "refactored_from" => RefactoredFrom,
        "tests_against" => TestsAgainst,
        "sibling" => Sibling,
        "documents" => Documents,
        _ => return None,
    })
}

pub fn snapshot(backend: &dyn PersistenceBackend) -> StorageResult<ProvenanceGraph> {
    let records = load_all_records(backend)?;
    let edges = load_all_edges(backend)?;
    Ok(ProvenanceGraph {
        records,
        edges,
        id: uuid::Uuid::new_v4().to_string(),
        timestamp: chrono::Utc::now(),
        name: String::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_cog_storage::SqliteBackend;
    use std::sync::Arc;

    fn backend() -> Arc<SqliteBackend> {
        let b = Arc::new(SqliteBackend::open_in_memory().unwrap());
        register(b.as_ref()).unwrap();
        b
    }

    fn rec(id: &str, artifact: &str) -> ProvenanceRecord {
        let mut r = ProvenanceRecord::new(
            artifact,
            ProvenanceSource::ModelOutput,
            "model",
            "p",
            "c",
            "g",
        );
        r.id = id.to_string();
        r
    }

    #[test]
    fn roundtrip_records_and_edges() {
        let b = backend();
        let r1 = rec("a", "file.rs");
        let r2 = rec("b", "file.rs");
        upsert_record(b.as_ref(), &r1).unwrap();
        upsert_record(b.as_ref(), &r2).unwrap();
        upsert_edge(b.as_ref(), "a", "b", ProvenanceEdgeType::DerivedFrom).unwrap();

        let snap = snapshot(b.as_ref()).unwrap();
        assert_eq!(snap.records.len(), 2);
        assert_eq!(snap.edges.len(), 1);
        assert_eq!(snap.edges[0].edge_type, ProvenanceEdgeType::DerivedFrom);
    }

    #[test]
    fn edge_replacement_keeps_unique() {
        let b = backend();
        upsert_record(b.as_ref(), &rec("a", "f")).unwrap();
        upsert_record(b.as_ref(), &rec("b", "f")).unwrap();
        upsert_edge(b.as_ref(), "a", "b", ProvenanceEdgeType::ProducedBy).unwrap();
        upsert_edge(b.as_ref(), "a", "b", ProvenanceEdgeType::RefactoredFrom).unwrap();
        let snap = snapshot(b.as_ref()).unwrap();
        assert_eq!(snap.edges.len(), 1);
        assert_eq!(snap.edges[0].edge_type, ProvenanceEdgeType::RefactoredFrom);
    }
}
