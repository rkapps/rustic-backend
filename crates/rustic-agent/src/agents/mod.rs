//! High-level agent orchestration: drives the LLM completion loop and dispatches tool calls.
//!
//! ## Module layout
//!
//! | Module | Contents |
//! |--------|----------|
//! | [`agent`] | [`Agent`] — single-model completion and tool-use loops |
//! | [`domain`] | Value types: [`AgentInput`](domain::AgentInput), [`CompletionTurn`](domain::CompletionTurn), [`StageDecision`](domain::StageDecision) |
//! | [`runner`] | [`Runnable`](runner::Runnable) trait, [`SingleAgent`](runner::SingleAgent), [`PipeLineAgent`](runner::PipeLineAgent) |
//! | [`helper`] | Pure functions for building messages, merging responses, and parsing decisions |

pub mod agent;
pub mod domain;
pub mod helper;
pub mod runner;
pub use agent::Agent;
