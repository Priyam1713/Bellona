//! ReAct with the failure-mode catalog encoded as breakers:
//! max-steps cap, no-progress detector (identical action â‰¥ 3 times).

use crate::model::{ModelClient, ToolCall};
use crate::strategy::{Step, Strategy};
use crate::BellumError;
use std::collections::VecDeque;

pub struct ReActStrategy {
    goal: String,
    max_steps: usize,
    steps_used: usize,
    recent_action_sigs: VecDeque<String>,
    cost_cents: u64,
    finished: bool,
}

const NO_PROGRESS_THRESHOLD: usize = 3;

impl ReActStrategy {
    pub fn new(goal: impl Into<String>, max_steps: usize) -> Self {
        ReActStrategy {
            goal: goal.into(),
            max_steps,
            steps_used: 0,
            recent_action_sigs: VecDeque::new(),
            cost_cents: 0,
            finished: false,
        }
    }

    fn action_signature(tc: &ToolCall) -> String {
        format!(
            "{}|{}",
            tc.name,
            serde_json::to_string(&tc.args).unwrap_or_default()
        )
    }
}

#[async_trait::async_trait]
impl Strategy for ReActStrategy {
    async fn begin(&mut self, _goal: &str) -> Result<(), BellumError> {
        Ok(())
    }

    async fn next_step(
        &mut self,
        observation: Option<&str>,
        model: &dyn ModelClient,
    ) -> Result<Step, BellumError> {
        if self.finished {
            return Ok(Step::Finish("already finished".into()));
        }

        // Breaker: step budget.
        if self.steps_used >= self.max_steps {
            self.finished = true;
            return Ok(Step::Breaker(format!(
                "max_steps={} reached",
                self.max_steps
            )));
        }

        // Breaker: no-progress detector.
        if self.recent_action_sigs.len() >= NO_PROGRESS_THRESHOLD {
            let window: Vec<&String> = self
                .recent_action_sigs
                .iter()
                .rev()
                .take(NO_PROGRESS_THRESHOLD)
                .collect();
            if window.iter().all(|s| *s == window[0]) {
                self.finished = true;
                return Ok(Step::Breaker(
                    "no-progress: identical action repeated".into(),
                ));
            }
        }

        // Build the scratchpad prompt: goal + observations + step count.
        let mut prompt = format!("GOAL: {}\n\n", self.goal);
        if let Some(obs) = observation {
            prompt.push_str("LAST OBSERVATION:\n");
            prompt.push_str(obs);
            prompt.push_str("\n\n");
        }
        prompt.push_str(&format!(
            "STEPS USED: {}/{}\n",
            self.steps_used, self.max_steps
        ));
        prompt.push_str(
            "Respond as JSON: {\"thought\": str, \"tool\": {\"name\": str, \"args\": obj}} \
             or {\"thought\": str, \"final_answer\": str}\n",
        );

        let reply = model.complete(&prompt).await?;

        self.cost_cents += reply.cost_cents;

        if let Some(ans) = reply.final_answer {
            self.finished = true;
            return Ok(Step::Finish(ans));
        }

        match reply.tool_calls.first() {
            Some(tc) => {
                let sig = Self::action_signature(tc);
                self.recent_action_sigs.push_back(sig);
                if self.recent_action_sigs.len() > 16 {
                    self.recent_action_sigs.pop_front();
                }
                self.steps_used += 1;
                Ok(Step::CallTool(tc.clone()))
            }
            None => {
                self.steps_used += 1;
                Ok(Step::Think(reply.thought))
            }
        }
    }

    fn name(&self) -> &'static str {
        "react"
    }

    fn accrued_cost_cents(&self) -> u64 {
        self.cost_cents
    }
}
