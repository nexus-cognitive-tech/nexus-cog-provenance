//! Human-readable explanations of provenance.

use nexus_cog_core::provenance::{ProvenanceEdgeType, ProvenanceSource};

#[cfg(test)]
use nexus_cog_core::provenance::ProvenanceRecord;

use crate::graph::ProvenanceGraphEngine;

/// Generates human-readable explanations of provenance chains.
#[derive(Debug, Clone)]
pub struct ProvenanceExplainer {
    engine: ProvenanceGraphEngine,
}

impl ProvenanceExplainer {
    /// Construct an explainer.
    #[must_use]
    pub fn new(engine: ProvenanceGraphEngine) -> Self {
        Self { engine }
    }

    /// Explain a single record.
    #[must_use]
    pub fn explain_record(&self, id: &str) -> Option<String> {
        let record = self.engine.get(id)?;
        Some(self.format_record(&record))
    }

    /// Explain the full ancestry chain of a record.
    #[must_use]
    pub fn explain_chain(&self, id: &str) -> String {
        let mut lines = Vec::new();
        let record = match self.engine.get(id) {
            Some(r) => r,
            None => return format!("No provenance record found for `{id}`."),
        };
        lines.push(format!("## Provenance for `{}`", record.artifact));
        lines.push(String::new());
        lines.push(self.format_record(&record));

        let ancestors = self.engine.ancestors_of(id);
        if !ancestors.is_empty() {
            lines.push(String::new());
            lines.push(format!("### Ancestry chain ({} step(s))", ancestors.len()));
            for (i, a) in ancestors.iter().rev().enumerate() {
                lines.push(format!("{}. {}", i + 1, self.format_record(a)));
            }
        }

        let descendants = self.engine.descendants_of(id);
        if !descendants.is_empty() {
            lines.push(String::new());
            lines.push(format!("### Descendants ({} step(s))", descendants.len()));
            for (i, d) in descendants.iter().enumerate() {
                lines.push(format!("{}. {}", i + 1, self.format_record(d)));
            }
        }

        lines.join("\n")
    }

    /// Generate a one-line summary suitable for inclusion in tool output.
    #[must_use]
    pub fn one_line(&self, id: &str) -> String {
        match self.engine.get(id) {
            Some(r) => {
                let conf = r.confidence.value();
                format!(
                    "{} via {} ({}) [agent={}, confidence={}]",
                    r.artifact, r.source.id(), r.origin, r.agent, conf
                )
            }
            None => format!("(no provenance for `{id}`)"),
        }
    }

    fn format_record(&self, r: &nexus_cog_core::provenance::ProvenanceRecord) -> String {
        format!(
            "- **{}** ({})\n  - origin: `{}`\n  - agent: `{}`\n  - timestamp: `{}`\n  - confidence: `{}`\n  - prompt: `{}`",
            r.artifact,
            r.source.id(),
            r.origin,
            r.agent,
            r.timestamp,
            r.confidence,
            truncate(&r.prompt, 80)
        )
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max).collect();
        format!("{truncated}…")
    }
}

/// Human-readable label for an edge type.
#[must_use]
pub fn edge_label(edge_type: ProvenanceEdgeType) -> &'static str {
    match edge_type {
        ProvenanceEdgeType::ProducedBy => "was produced by",
        ProvenanceEdgeType::DerivedFrom => "was derived from",
        ProvenanceEdgeType::RefactoredFrom => "was refactored from",
        ProvenanceEdgeType::TestsAgainst => "tests against",
        ProvenanceEdgeType::Sibling => "is a sibling of",
        ProvenanceEdgeType::Documents => "documents",
    }
}

/// Human-readable label for a source.
#[must_use]
pub fn source_label(source: ProvenanceSource) -> &'static str {
    match source {
        ProvenanceSource::ModelOutput => "model output",
        ProvenanceSource::ToolExecution => "tool execution",
        ProvenanceSource::TestRun => "test run",
        ProvenanceSource::UserInput => "user input",
        ProvenanceSource::Reasoning => "agent reasoning",
        ProvenanceSource::CodeExtraction => "code extraction",
        ProvenanceSource::FileLoad => "file load",
        ProvenanceSource::Composition => "composition",
        ProvenanceSource::Inference => "inference",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_cog_core::provenance::ProvenanceSource;

    #[test]
    fn explain_chain_returns_string() {
        let mut e = ProvenanceGraphEngine::in_memory().unwrap();
        let r = ProvenanceRecord::new("f.rs", ProvenanceSource::ModelOutput, "model", "p", "c", "g");
        let id = r.id.clone();
        e.add_record(r);
        let explainer = ProvenanceExplainer::new(e);
        let s = explainer.explain_chain(&id);
        assert!(s.contains("Provenance"));
        assert!(s.contains("f.rs"));
    }

    #[test]
    fn one_line_includes_origin() {
        let mut e = ProvenanceGraphEngine::in_memory().unwrap();
        let r = ProvenanceRecord::new("f.rs", ProvenanceSource::ModelOutput, "claude-opus-4", "p", "c", "g");
        let id = r.id.clone();
        e.add_record(r);
        let s = ProvenanceExplainer::new(e).one_line(&id);
        assert!(s.contains("claude-opus-4"));
    }

    #[test]
    fn edge_label_returns_human_readable() {
        assert_eq!(edge_label(ProvenanceEdgeType::ProducedBy), "was produced by");
        assert_eq!(edge_label(ProvenanceEdgeType::RefactoredFrom), "was refactored from");
    }
}
