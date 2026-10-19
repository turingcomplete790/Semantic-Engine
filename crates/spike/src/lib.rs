//! Phase 1 spike (specs/003-engine-design-spike). Measurement-only code: it is never built by
//! the release package scripts and never ships (spec FR-013, research R9).

pub mod bench;
pub mod measure;
pub mod plugin_ops;
pub mod store;
pub mod synth;
