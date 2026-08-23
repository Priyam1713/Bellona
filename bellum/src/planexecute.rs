//! Plan-and-Execute with the two documented guards:
//! - **plan validation** before execution (planner hallucination guard),
//! - **replan trigger** on observation mismatch (stale-plan guard), once.

use crate::model::{ModelClient, ToolCall};
use crate::strategy::Step;
use crate::BellumError;
use std::collections::HashSet;

#[derive(Debug, Clone)]
struct PlanStep {
    call: ToolCall,
    #[allow(dead_code)] // surfaced in audit rows by the loop in Milestone II
    description: String,
}

pub struct PlanExecuteStrategy {
    goal: String,
    planner: Option<Box<dyn ModelClient>>,
    plan: Vec<PlanStep>,
    idx: usize,
    allowed_tools: HashSet<String>,
    replanned: bool,
    finished: bool,
    max_plan_len: usize,
}

impl PlanExecuteStrategy {
    /// `allowed_tools` — the exposed registry snapshot. Plans referencing
    /// anything else are hallucinations and refuse before execution.
    pub fn new(
        goal: impl Into<String>,
        planner: Box<dyn ModelClient>,
        allowed_tools: impl IntoIterator<Item = String>,
    ) -> Self {
        PlanExecuteStrategy {
            goal: goal.into(),
            planner: Some(planner),
            plan: Vec::new(),
            idx: 0,
            allowed_tools: allowed_tools.into_iter().collect(),
            replanned: false,
            finished: false,
            max_plan_len: 25,
        }
    }

    fn parse_plan(&self, reply: &str) -> Result<Vec<PlanStep>, BellumError> {
        let parsed: serde_json::Value = serde_json::from_str(reply.trim())
            .map_err(|e| BellumError::Strategy(format!("plan not JSON: {e}")))?;
        let arr = parsed
            .as_array()
            .ok_or_else(|| BellumError::Strategy("plan is not an array".into()))?;
        if arr.len() > self.max_plan_len {
            return Err(BellumError::Strategy(format!(
                "plan exceeds {} steps",
                self.max_plan_len
            )));
        }
        let mut steps = Vec::new();
        for s in arr {
            let tool = s
                .get("tool")
                .and_then(|v| v.as_str())
                .ok_or_else(|| BellumError::Strategy("plan step missing 'tool'".into()))?
                .to_string();
            if !self.allowed_tools.contains(&tool) {
                return Err(BellumError::Strategy(format!(
                    "hallucinated tool '{tool}' in plan"
                )));
            }
            steps.push(PlanStep {
                call: ToolCall {
                    name: tool,
                    args: s.get("args").cloned().unwrap_or(serde_json::Value::Null),
                },
                description: s
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
            });
        }
        Ok(steps)
    }
}

#[async_trait::async_trait]
impl crate::strategy::Strategy for PlanExecuteStrategy {
    async fn begin(&mut self, _goal: &str) -> Result<(), BellumError> {
        let planner = self
            .planner
            .take()
            .ok_or_else(|| BellumError::Strategy("planner already consumed".into()))?;
        let prompt = format!(
            "GOAL: {}\nTOOLS: {:?}\nProduce a JSON array of steps: \
             [{{\"tool\": str, \"args\": obj, \"description\": str}}].",
            self.goal,
            self.allowed_tools.iter().collect::<Vec<_>>()
        );
        let reply = planner.complete(&prompt).await?;
        self.plan = self.parse_plan(&reply.thought)?;
        Ok(())
    }

    async fn next_step(
        &mut self,
        observation: Option<&str>,
        executor: &dyn ModelClient,
    ) -> Result<Step, BellumError> {
        if self.finished {
            return Ok(Step::Finish("already finished".into()));
        }

        // Stale-plan trigger: observations that begin with ERROR/DENIED.
        // Once per campaign — replanning loops are their own failure mode.
        if let Some(obs) = observation {
            let stale = obs.starts_with("ERROR") || obs.starts_with("DENIED");
            if stale && !self.replanned && self.idx > 0 {
                self.replanned = true;
                self.plan.truncate(self.idx);
            }
        }

        if self.idx < self.plan.len() {
            let step = self.plan[self.idx].clone();
            self.idx += 1;
            return Ok(Step::CallTool(step.call));
        }

        // Plan exhausted → summarize against the last observation.
        self.finished = true;
        let mut prompt = format!("GOAL: {}\n", self.goal);
        if let Some(obs) = observation {
            prompt.push_str("FINAL OBSERVATION:\n");
            prompt.push_str(obs);
        }
        let reply = executor.complete(&prompt).await?;
        Ok(match reply.final_answer {
            Some(a) => Step::Finish(a),
            None => Step::Finish(reply.thought),
        })
    }

    fn name(&self) -> &'static str {
        "plan_execute"
    }
}
