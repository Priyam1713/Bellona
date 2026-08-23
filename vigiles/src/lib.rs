//! # vigiles — the watchmen.
//!
//! Tracing (OTel GenAI-style span names) and the Colosseum: harness-controlled
//! evaluation with **pass^k** reliability gates (Law VII).

pub mod colosseum;
pub mod trace;

pub use colosseum::{
    compute_pass_at_k, CaseResult, CaseVerdict, Gate, GateVerdict, SuiteCase, SuiteReport, Verifier,
};
pub use trace::{Recorder, Span, SpanKind, TraceRecorder};
