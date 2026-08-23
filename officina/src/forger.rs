//! A forged tool: what an agent proposes. Not yet trusted for anything.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScriptLang {
    /// Executed via the platform shell inside the camp.
    Shell,
    /// Python if the camp provides it.
    Python,
}

impl ScriptLang {
    pub fn interpreter(&self) -> &'static str {
        match self {
            // Windows-safe default; camps translate per platform.
            ScriptLang::Shell => "cmd",
            ScriptLang::Python => "python",
        }
    }
}

/// The proposal itself. Carries provenance of WHO forged it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgedTool {
    /// snake_case name; must be unique in the registry namespace.
    pub name: String,
    pub description: String,
    pub lang: ScriptLang,
    /// Command-line template. `{input}` is substituted with the JSON args.
    pub script_template: String,
    /// The agent that forged this.
    pub forged_by: String,
}

impl ForgedTool {
    pub fn validate(&self) -> Result<(), crate::OfficinaError> {
        let ok_name = !self.name.is_empty()
            && self
                .name
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
        if !ok_name {
            return Err(crate::OfficinaError(
                "tool name must be non-empty snake_case".into(),
            ));
        }
        if self.description.len() < 8 {
            return Err(crate::OfficinaError(
                "description too short to audit".into(),
            ));
        }
        if !self.script_template.contains("{input}") {
            return Err(crate::OfficinaError(
                "script template must reference {input}".into(),
            ));
        }
        Ok(())
    }

    /// Render the concrete command for one invocation.
    pub fn render(&self, input_json: &str) -> Vec<String> {
        let filled = self.script_template.replace("{input}", input_json);
        match self.lang {
            ScriptLang::Shell => vec!["/C".to_string(), filled],
            ScriptLang::Python => vec!["-c".to_string(), filled],
        }
    }
}
