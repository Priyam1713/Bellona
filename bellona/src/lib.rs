//! The war machine, as a library ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â so tests can drive real tools through a
//! real gate before `main.rs` ever touches a terminal.

pub mod arsenal;
pub mod warroom;
pub mod warroom_html;

use auxilia::OpenAiCompatClient;
use castra::{CampCommand, EnvScrubPolicy, ProcessDriver, SandboxDriver};
use forge::error::ForgeResult;
use forge::id::AgentId;
use forge::primitives::{ActionRequest, EffectKind, ResourceInfo};
use forge::tool::{Tool, ToolContext, ToolRegistry, ToolSpec};
use praetorium::custos::{CustosGateway, EffectExecutor, SnapshotResolver};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Runtime configuration.
#[derive(Debug, Clone)]
pub struct BellonaConfig {
    pub workspace: PathBuf,
    pub base_url: String,
    pub api_key: Option<String>,
    pub model: String,
    /// Auto-approver principal; None means approvals park for a human.
    pub yolo: bool,
    pub allow_shell: bool,
    pub max_steps: usize,
}

impl Default for BellonaConfig {
    fn default() -> Self {
        BellonaConfig {
            workspace: PathBuf::from("."),
            base_url: "http://localhost:11434/v1".into(),
            api_key: None,
            model: "local-model".into(),
            yolo: false,
            allow_shell: false,
            max_steps: 24,
        }
    }
}

// ---------- path discipline ----------

/// Resolve a tool-supplied path against the workspace; refuse escapes.
pub fn resolve_in_workspace(workspace: &Path, supplied: &str) -> ForgeResult<PathBuf> {
    let candidate = PathBuf::from(supplied);
    let full = if candidate.is_absolute() {
        candidate
    } else {
        workspace.join(candidate)
    };
    // Canonicalize what exists; for not-yet-existing files canonicalize the
    // deepest existing ancestor and re-append the tail.
    let canon = match full.canonicalize() {
        Ok(c) => c,
        Err(_) => {
            let mut anc = full.as_path();
            while let Some(parent) = anc.parent() {
                if parent.exists() {
                    let pc = parent.canonicalize().map_err(forge::ForgeError::Io)?;
                    let tail = full.strip_prefix(parent).unwrap_or(&full).to_path_buf();
                    return Ok(pc.join(tail));
                }
                anc = parent;
            }
            return Err(forge::ForgeError::Other(format!(
                "cannot anchor path '{supplied}' inside the workspace"
            )));
        }
    };
    let ws_canon = workspace.canonicalize().map_err(forge::ForgeError::Io)?;
    if !canon.starts_with(&ws_canon) {
        return Err(forge::ForgeError::Other(format!(
            "path '{supplied}' escapes the workspace"
        )));
    }
    Ok(canon)
}

// ---------- the four legionary tools ----------

pub struct ReadFileTool {
    pub workspace: PathBuf,
}

#[async_trait::async_trait]
impl Tool for ReadFileTool {
    fn spec(&self) -> &ToolSpec {
        static SPEC: std::sync::OnceLock<ToolSpec> = std::sync::OnceLock::new();
        SPEC.get_or_init(|| ToolSpec {
            name: "read_file".into(),
            description: "Read a text file from the workspace. Args: {\"path\": str}".into(),
            effect: EffectKind::FileRead,
            read_only: true,
            schema: serde_json::json!({"path": "string"}),
        })
    }
    async fn execute(
        &self,
        _ctx: &ToolContext,
        args: serde_json::Value,
    ) -> ForgeResult<serde_json::Value> {
        let p = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| forge::ForgeError::Other("missing 'path'".into()))?;
        let full = resolve_in_workspace(&self.workspace, p)?;
        let content = std::fs::read_to_string(&full).map_err(forge::ForgeError::Io)?;
        Ok(serde_json::json!({ "path": p, "content": content }))
    }
}

pub struct ListFilesTool {
    pub workspace: PathBuf,
}

#[async_trait::async_trait]
impl Tool for ListFilesTool {
    fn spec(&self) -> &ToolSpec {
        static SPEC: std::sync::OnceLock<ToolSpec> = std::sync::OnceLock::new();
        SPEC.get_or_init(|| ToolSpec {
            name: "list_files".into(),
            description: "List files in the workspace (relative paths). Args: {}".into(),
            effect: EffectKind::Custom("list_files".into()),
            read_only: true,
            schema: serde_json::json!({}),
        })
    }
    async fn execute(
        &self,
        _ctx: &ToolContext,
        _args: serde_json::Value,
    ) -> ForgeResult<serde_json::Value> {
        let mut out = Vec::new();
        fn walk(dir: &Path, base: &Path, out: &mut Vec<String>, depth: usize) {
            if depth > 6 {
                return;
            }
            if let Ok(rd) = std::fs::read_dir(dir) {
                for entry in rd.flatten() {
                    let p = entry.path();
                    if p.is_dir() {
                        walk(&p, base, out, depth + 1);
                    } else if let Ok(rel) = p.strip_prefix(base) {
                        out.push(rel.to_string_lossy().replace('\\', "/"));
                    }
                }
            }
        }
        walk(&self.workspace, &self.workspace, &mut out, 0);
        out.sort();
        Ok(serde_json::json!({ "files": out }))
    }
}

pub struct WriteFileTool {
    pub workspace: PathBuf,
}

#[async_trait::async_trait]
impl Tool for WriteFileTool {
    fn spec(&self) -> &ToolSpec {
        static SPEC: std::sync::OnceLock<ToolSpec> = std::sync::OnceLock::new();
        SPEC.get_or_init(|| ToolSpec {
            name: "write_file".into(),
            description:
                "Write text to a file in the workspace. Args: {\"path\": str, \"content\": str}"
                    .into(),
            effect: EffectKind::FileWrite,
            read_only: false,
            schema: serde_json::json!({"path": "string", "content": "string"}),
        })
    }
    async fn execute(
        &self,
        _ctx: &ToolContext,
        args: serde_json::Value,
    ) -> ForgeResult<serde_json::Value> {
        let p = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| forge::ForgeError::Other("missing 'path'".into()))?;
        let content = args
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| forge::ForgeError::Other("missing 'content'".into()))?;
        let full = resolve_in_workspace(&self.workspace, p)?;
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).map_err(forge::ForgeError::Io)?;
        }
        std::fs::write(&full, content).map_err(forge::ForgeError::Io)?;
        Ok(serde_json::json!({ "written": true, "path": p, "bytes": content.len() }))
    }
}

pub struct ShellTool;

#[async_trait::async_trait]
impl Tool for ShellTool {
    fn spec(&self) -> &ToolSpec {
        static SPEC: std::sync::OnceLock<ToolSpec> = std::sync::OnceLock::new();
        SPEC.get_or_init(|| ToolSpec {
            name: "run_shell".into(),
            description:
                "Run one scrubbed-environment command in the workspace. Args: {\"command\": str}"
                    .into(),
            effect: EffectKind::ShellExec,
            read_only: false,
            schema: serde_json::json!({"command": "string"}),
        })
    }
    async fn execute(
        &self,
        ctx: &ToolContext,
        args: serde_json::Value,
    ) -> ForgeResult<serde_json::Value> {
        let command = args
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| forge::ForgeError::Other("missing 'command'".into()))?
            .to_string();

        #[cfg(windows)]
        let cmd = CampCommand {
            program: "cmd".into(),
            args: vec!["/C".into(), command],
            working_dir: ctx.workspace.clone(),
            timeout_secs: 60,
        };
        #[cfg(not(windows))]
        let cmd = CampCommand {
            program: "sh".into(),
            args: vec!["-c".into(), command],
            working_dir: ctx.workspace.clone(),
            timeout_secs: 60,
        };

        let outcome = ProcessDriver
            .run(&cmd, &EnvScrubPolicy::default())
            .await
            .map_err(|e| forge::ForgeError::Other(e.to_string()))?;
        Ok(serde_json::json!({
            "exit_ok": outcome.exit_ok,
            "stdout": outcome.stdout,
            "stderr": outcome.stderr,
        }))
    }
}

// ---------- executor bridge ----------

/// Performs effects by invoking registered tools in the agent's context.
/// Production Castra Tertia swaps this for container execution; the gate
/// never notices.
pub struct RegistryExecutor {
    pub registry: Arc<ToolRegistry>,
}

#[async_trait::async_trait]
impl EffectExecutor for RegistryExecutor {
    async fn perform(
        &self,
        req: &ActionRequest,
        _resolved: &ResourceInfo,
        ws: &std::path::Path,
    ) -> Result<serde_json::Value, String> {
        let tool = self
            .registry
            .get(&req.tool_name)
            .ok_or_else(|| format!("tool '{}' missing", req.tool_name))?;
        let ctx = ToolContext {
            agent_id: req.agent_id.clone(),
            workspace: ws.to_path_buf(),
        };
        tool.execute(&ctx, req.params.clone())
            .await
            .map_err(|e| e.to_string())
    }
}

// ---------- assembly ----------

pub struct Assembled {
    pub gateway: Arc<CustosGateway<SnapshotResolver, RegistryExecutor>>,
    pub registry: Arc<ToolRegistry>,
}

fn law(yolo: bool, allow_shell: bool) -> praetorium::Lex {
    use praetorium::RuleEffect::{self as Eff, *};
    let mut specs = vec![
        praetorium::RuleSpec {
            id: "deny-shell-by-default".into(),
            effect: Deny,
            expr: format!("attr.tool.name == 'run_shell' && !{}", allow_shell || yolo),
        },
        praetorium::RuleSpec {
            id: "gate-writes".into(),
            effect: if yolo { Allow } else { RequireApproval },
            expr: "attr.effect.kind == 'file_write'".into(),
        },
        praetorium::RuleSpec {
            id: "allow-reads".into(),
            effect: Allow,
            expr: "attr.effect.kind == 'file_read' || attr.effect.kind == 'list_files'".into(),
        },
    ];
    if yolo && allow_shell {
        specs.retain(|s| s.id != "deny-shell-by-default");
        specs.push(praetorium::RuleSpec {
            id: "yolo-shell".into(),
            effect: Eff::Allow,
            expr: "attr.tool.name == 'run_shell'".into(),
        });
    } else if yolo {
        specs.push(praetorium::RuleSpec {
            id: "yolo-shell-gated".into(),
            effect: Allow,
            expr: format!("attr.tool.name == 'run_shell' && {}", allow_shell),
        });
    }
    praetorium::Lex::from_specs(&specs).expect("built-in law compiles")
}

/// Assemble a complete war camp from configuration.
pub fn assemble(cfg: &BellonaConfig) -> anyhow_free::AssemblyResult<Assembled> {
    let workspace = cfg.workspace.canonicalize().map_err(|e| {
        anyhow_free::AssemblyError(format!(
            "workspace '{}' does not exist ({e})",
            cfg.workspace.display()
        ))
    })?;

    let mut resolver = SnapshotResolver::new();
    resolver.upsert(ResourceInfo {
        uri: "file://workspace".into(),
        kind: "workspace".into(),
        label: Some("the campaign workspace".into()),
    });

    let mut reg = ToolRegistry::new();
    reg.register(Arc::new(ReadFileTool {
        workspace: workspace.clone(),
    }));
    reg.register(Arc::new(ListFilesTool {
        workspace: workspace.clone(),
    }));
    reg.register(Arc::new(WriteFileTool {
        workspace: workspace.clone(),
    }));
    reg.register(Arc::new(ShellTool));
    for t in crate::arsenal::git_read_tools() {
        reg.register(t);
    }
    for t in crate::arsenal::git_write_tools() {
        reg.register(t);
    }
    for t in crate::arsenal::search_tools(None) {
        reg.register(t);
    }
    reg.register(crate::arsenal::web_fetch_tool(reqwest::Client::new()));
    for name in [
        "read_file",
        "list_files",
        "write_file",
        "run_shell",
        "git_status",
        "git_log",
        "git_diff",
        "git_commit",
        "git_branch",
        "search_files",
        "read_document",
        "web_fetch",
    ] {
        reg.set_exposed(name, true)?;
    }
    let registry = Arc::new(reg);

    let gw = CustosGateway::new(
        resolver,
        RegistryExecutor {
            registry: registry.clone(),
        },
        workspace.clone(),
    )
    .with_identity_enforced(false);
    gw.install_law(law(cfg.yolo, cfg.allow_shell));

    Ok(Assembled {
        gateway: Arc::new(gw),
        registry,
    })
}

pub fn model_client(cfg: &BellonaConfig) -> Arc<dyn bellum::ModelClient> {
    Arc::new(OpenAiCompatClient::new(
        cfg.base_url.clone(),
        cfg.api_key.clone(),
        cfg.model.clone(),
        "terra",
    ))
}

/// Tiny result type so the bin keeps zero extra deps (no anyhow).
pub mod anyhow_free {
    #[derive(Debug)]
    pub struct AssemblyError(pub String);
    impl std::fmt::Display for AssemblyError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str(&self.0)
        }
    }
    impl std::error::Error for AssemblyError {}
    pub type AssemblyResult<T> = Result<T, AssemblyError>;

    impl From<forge::ForgeError> for AssemblyError {
        fn from(e: forge::ForgeError) -> Self {
            AssemblyError(e.to_string())
        }
    }
}

/// Mint the campaign's agent identity once per process.
pub fn new_agent() -> AgentId {
    AgentId::mint()
}

pub mod colosseum;

/// War-Room console HTML (single embedded file).
pub fn warroom_html() -> &'static str {
    crate::warroom_html::HTML
}

/// Test-support models (compiled always, used by tests/examples).
pub mod tests_support {
    

    pub struct NullModel;

    #[async_trait::async_trait]
    impl bellum::ModelClient for NullModel {
        fn tier(&self) -> &'static str {
            "luna"
        }
        async fn complete(&self, _p: &str) -> Result<bellum::ModelReply, bellum::BellumError> {
            Ok(bellum::ModelReply {
                thought: String::new(),
                tool_calls: vec![],
                final_answer: Some("null model".into()),
                cost_cents: 0,
            })
        }
    }
}
