//! Cognitive Provenance Graph (CPG).
//!
//! Every AI-generated artifact — a code block, a file change, a tool result — is
//! tracked back to the prompt, tool, and thought that produced it. This enables:
//!
//! - **Audit**: "Where did this code come from? Was it produced by a model or a tool?"
//! - **Debug**: "Why was this change made? What thought led to it?"
//! - **Replay**: "Reconstruct the chain of events that produced this artifact."
//! - **Blame**: "Which prompt generated this line of code?"
//! - **Trust**: "Which agent produced this? How confident are we?"

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod explain;
pub mod graph;
pub mod query;
pub mod record;
pub mod store;
pub mod trace;

pub use explain::ProvenanceExplainer;
pub use graph::ProvenanceGraphEngine;
pub use query::ProvenanceQueryEngine;
pub use record::ProvenanceRecorder;
pub use store::ProvenanceStore;
pub use trace::ProvenanceTracer;
