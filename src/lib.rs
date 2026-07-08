//! Provenance tracking: artifact lineage, graph queries, blast radius.
//!
//! Persisted via `nexus-cog-storage`.

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod explain;
pub mod graph;
pub mod query;
pub mod record;
pub mod schema;
pub mod trace;
