//! Lex â€” the law of the camp.
//!
//! CEL rules evaluated over a nested attribute map rooted at `attr`:
//!
//! ```text
//! attr.tool.name == "shell" && attr.effect.kind != "file_read"
//! attr.page.host == "internal.corp"
//! ```
//!
//! Ordering is doctrine (Law IV): **explicit denies first, then approvals,
//! then allows, then default-deny.** A missing policy permits nothing; a
//! broken rule refuses rather than opens.

use forge::primitives::{Decision, PolicyAttrs};
use serde::{Deserialize, Serialize};
use serde_json::json;

/// What a matching rule commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleEffect {
    Allow,
    Deny,
    RequireApproval,
}

/// Source form of a rule â€” what deployments author.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleSpec {
    pub id: String,
    pub effect: RuleEffect,
    /// CEL expression over `attr.*`.
    pub expr: String,
}

struct Rule {
    id: String,
    effect: RuleEffect,
    program: cel_interpreter::Program,
}

/// The policy engine. Rules are grouped by verdict class so
/// deny-before-allow ordering is structural, not conventional.
#[derive(Default)]
pub struct Lex {
    denies: Vec<Rule>,
    approvals: Vec<Rule>,
    allows: Vec<Rule>,
}

/// Sentinel rule ids used for structural refusals.
pub const RULE_BROKEN: &str = "__lex_broken_rule__";
pub const RULE_DEFAULT_DENY: &str = "__lex_default_deny__";
pub const RULE_UNRESOLVED: &str = "__lex_unresolvable_target__";

impl Lex {
    /// An empty Lex permits nothing (Law IV).
    pub fn empty() -> Self {
        Self::default()
    }

    /// Compile a rule set. Compilation failure aborts deployment rather
    /// than silently weakening it â€” half-configured policies refuse to start.
    pub fn from_specs(specs: &[RuleSpec]) -> Result<Self, String> {
        let mut lex = Lex::empty();
        for s in specs {
            let program = cel_interpreter::Program::compile(&s.expr)
                .map_err(|e| format!("rule '{}' failed to compile: {e}", s.id))?;
            let rule = Rule {
                id: s.id.clone(),
                effect: s.effect,
                program,
            };
            match s.effect {
                RuleEffect::Deny => lex.denies.push(rule),
                RuleEffect::RequireApproval => lex.approvals.push(rule),
                RuleEffect::Allow => lex.allows.push(rule),
            }
        }
        Ok(lex)
    }

    pub fn len_rules(&self) -> usize {
        self.denies.len() + self.approvals.len() + self.allows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len_rules() == 0
    }

    /// Evaluate attributes against the rule set, fail-closed.
    ///
    /// Any evaluation error yields `Deny { rule_id: RULE_BROKEN }`.
    /// No match yields `Deny { rule_id: RULE_DEFAULT_DENY }`.
    pub fn decide(&self, attrs: &PolicyAttrs) -> Decision {
        // Hot path: nest the attribute map ONCE per decision, not per rule.
        let root = nested_json(attrs);
        for class in [&self.denies, &self.approvals, &self.allows] {
            for rule in class {
                match self.eval(rule, &root) {
                    Ok(true) => {
                        return match rule.effect {
                            RuleEffect::Deny => Decision::Deny {
                                rule_id: rule.id.clone(),
                                reason: format!("matched deny rule '{}'", rule.id),
                            },
                            RuleEffect::Allow => Decision::Allow {
                                rule_id: rule.id.clone(),
                            },
                            RuleEffect::RequireApproval => Decision::RequireApproval {
                                rule_id: rule.id.clone(),
                            },
                        };
                    }
                    Ok(false) => continue,
                    Err(e) => {
                        return Decision::Deny {
                            rule_id: RULE_BROKEN.to_string(),
                            reason: format!("rule '{}' evaluation error: {e}", rule.id),
                        };
                    }
                }
            }
        }
        Decision::Deny {
            rule_id: RULE_DEFAULT_DENY.to_string(),
            reason: "no rule matched â€” default deny".to_string(),
        }
    }

    fn eval(&self, rule: &Rule, root: &serde_json::Value) -> Result<bool, String> {
        let mut ctx = cel_interpreter::Context::default();
        ctx.add_variable("attr", root).map_err(|e| e.to_string())?;
        let value = rule.program.execute(&ctx).map_err(|e| e.to_string())?;
        cel_truthy(&value).ok_or_else(|| format!("non-boolean result from '{}'", rule.id))
    }
}

/// Convert dotted attribute keys (`tool.name`) into nested JSON maps
/// (`attr.tool.name`) â€” CEL resolves attribute chains through them.
fn nested_json(attrs: &PolicyAttrs) -> serde_json::Value {
    fn insert(
        map: &mut serde_json::Map<String, serde_json::Value>,
        path: &[&str],
        value: serde_json::Value,
    ) {
        if path.len() == 1 {
            map.insert(path[0].to_string(), value);
            return;
        }
        let entry = map.entry(path[0].to_string()).or_insert_with(|| json!({}));
        if !entry.is_object() {
            *entry = json!({});
        }
        if let serde_json::Value::Object(inner) = entry {
            insert(inner, &path[1..], value);
        }
    }

    let mut root = serde_json::Map::new();
    for (k, v) in &attrs.0 {
        let parts: Vec<&str> = k.split('.').collect();
        insert(&mut root, &parts, v.clone());
    }
    serde_json::Value::Object(root)
}

/// Interpret a CEL result as truth; `None` means "not boolean".
fn cel_truthy(v: &cel_interpreter::Value) -> Option<bool> {
    use cel_interpreter::Value;
    match v {
        Value::Bool(b) => Some(*b),
        Value::Int(i) => Some(*i != 0),
        Value::UInt(u) => Some(*u != 0),
        Value::Float(f) => Some(*f != 0.0),
        Value::String(s) => Some(!s.is_empty()),
        Value::Bytes(b) => Some(!b.is_empty()),
        Value::List(l) => Some(!l.is_empty()),
        Value::Map(m) => Some(!m.map.is_empty()),
        Value::Null => Some(false),
        // Temporal values and bound functions are always meaningful.
        Value::Duration(_) | Value::Timestamp(_) | Value::Function(_, _) => Some(true),
    }
}
