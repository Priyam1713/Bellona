//! Spans following the OpenTelemetry GenAI semantic-convention spirit:
//! `gen_ai.agent.*` naming, hierarchical, with timing. A vendor-neutral
//! OTLP exporter is an adapter away; the in-memory recorder keeps the
//! kernel honest and dependency-free.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpanKind {
    AgentRun,
    AgentStep,
    LlmCall,
    ToolCall,
    Retrieval,
}

impl SpanKind {
    pub fn convention_name(&self) -> &'static str {
        match self {
            SpanKind::AgentRun => "gen_ai.agent.run",
            SpanKind::AgentStep => "gen_ai.agent.step",
            SpanKind::LlmCall => "gen_ai.llm.call",
            SpanKind::ToolCall => "gen_ai.tool.call",
            SpanKind::Retrieval => "gen_ai.retrieval.search",
        }
    }
}

/// One finished span.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Span {
    pub name: String,
    pub kind: SpanKind,
    pub started_ms: u64,
    pub ended_ms: u64,
    /// Attributes: model, tool name, cost cents, outcome…
    pub attrs: BTreeMap<String, String>,
}

impl Span {
    pub fn duration_ms(&self) -> u64 {
        self.ended_ms.saturating_sub(self.started_ms)
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Sink for spans.
pub trait Recorder: Send + Sync {
    fn record(&self, span: Span);
}

/// Thread-safe in-memory sink; export adapters read from `snapshot()`.
#[derive(Default, Clone)]
pub struct TraceRecorder {
    spans: Arc<Mutex<Vec<Span>>>,
}

impl TraceRecorder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Time an operation into a completed span.
    pub fn time<T>(
        &self,
        kind: SpanKind,
        attrs: BTreeMap<String, String>,
        f: impl FnOnce() -> T,
    ) -> T {
        let started = now_ms();
        let out = f();
        let ended = now_ms();
        self.record(Span {
            name: kind.convention_name().to_string(),
            kind,
            started_ms: started,
            ended_ms: ended,
            attrs,
        });
        out
    }

    pub fn snapshot(&self) -> Vec<Span> {
        self.spans.lock().expect("trace poisoned").clone()
    }
}

impl Recorder for TraceRecorder {
    fn record(&self, span: Span) {
        self.spans.lock().expect("trace poisoned").push(span);
    }
}
