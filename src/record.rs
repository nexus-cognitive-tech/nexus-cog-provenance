//! Record a new provenance entry.

use nexus_cog_core::provenance::{ProvenanceEdge, ProvenanceEdgeType, ProvenanceRecord, ProvenanceSource};

/// Records new provenance entries.
#[derive(Debug, Clone, Default)]
pub struct ProvenanceRecorder {
    counter: u64,
}

impl ProvenanceRecorder {
    /// Construct a new recorder.
    #[must_use]
    pub const fn new() -> Self {
        Self { counter: 0 }
    }

    /// Construct a record for a model-generated artifact.
    pub fn record_model_output(
        &mut self,
        artifact: impl Into<String>,
        model: impl Into<String>,
        prompt: impl Into<String>,
        content: impl Into<String>,
        agent: impl Into<String>,
    ) -> ProvenanceRecord {
        self.counter += 1;
        ProvenanceRecord::new(artifact, ProvenanceSource::ModelOutput, model, prompt, content, agent)
    }

    /// Construct a record for a tool execution.
    pub fn record_tool_execution(
        &mut self,
        artifact: impl Into<String>,
        tool_name: impl Into<String>,
        prompt: impl Into<String>,
        content: impl Into<String>,
        agent: impl Into<String>,
    ) -> ProvenanceRecord {
        self.counter += 1;
        ProvenanceRecord::new(artifact, ProvenanceSource::ToolExecution, tool_name, prompt, content, agent)
    }

    /// Construct a record for a test run.
    pub fn record_test_run(
        &mut self,
        artifact: impl Into<String>,
        test_name: impl Into<String>,
        content: impl Into<String>,
        agent: impl Into<String>,
    ) -> ProvenanceRecord {
        self.counter += 1;
        ProvenanceRecord::new(artifact, ProvenanceSource::TestRun, test_name, "", content, agent)
    }

    /// Construct a record for user input.
    pub fn record_user_input(
        &mut self,
        artifact: impl Into<String>,
        content: impl Into<String>,
    ) -> ProvenanceRecord {
        self.counter += 1;
        ProvenanceRecord::new(artifact, ProvenanceSource::UserInput, "user", "", content, "user")
    }

    /// Construct a record for an inference / deduction.
    pub fn record_inference(
        &mut self,
        artifact: impl Into<String>,
        reasoning: impl Into<String>,
        content: impl Into<String>,
        agent: impl Into<String>,
    ) -> ProvenanceRecord {
        self.counter += 1;
        ProvenanceRecord::new(artifact, ProvenanceSource::Inference, "agent-inference", reasoning, content, agent)
    }

    /// Build an edge linking two records.
    #[must_use]
    pub fn make_edge(&self, from: impl Into<String>, to: impl Into<String>, edge_type: ProvenanceEdgeType) -> ProvenanceEdge {
        ProvenanceEdge {
            from: from.into(),
            to: to.into(),
            edge_type,
            notes: String::new(),
        }
    }

    /// Total records created so far.
    #[must_use]
    pub fn counter(&self) -> u64 {
        self.counter
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_model_output_has_correct_source() {
        let mut r = ProvenanceRecorder::new();
        let rec = r.record_model_output("fn foo() {}", "claude-opus-4", "write foo", "fn foo() {}", "agent-1");
        assert_eq!(rec.source, ProvenanceSource::ModelOutput);
        assert_eq!(rec.origin, "claude-opus-4");
    }

    #[test]
    fn counter_increments() {
        let mut r = ProvenanceRecorder::new();
        r.record_model_output("a", "m", "p", "c", "g");
        r.record_tool_execution("b", "t", "p", "c", "g");
        assert_eq!(r.counter(), 2);
    }

    #[test]
    fn make_edge_links_records() {
        let r = ProvenanceRecorder::new();
        let e = r.make_edge("a", "b", ProvenanceEdgeType::ProducedBy);
        assert_eq!(e.from, "a");
        assert_eq!(e.to, "b");
    }
}
