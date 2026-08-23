//! Campaign V: arsenal conformance â€” git on a REAL repo, search, chunked
//! reads, SSRF shield, and the forge-testkit battery over every new tool.

use bellona::{assemble, BellonaConfig};
use forge::testkit::conform_fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
static UNIQ_SEQ: AtomicU64 = AtomicU64::new(1);
fn uniq() -> u64 {
    UNIQ_SEQ.fetch_add(1, Ordering::Relaxed)
}
use std::sync::{Arc, Mutex};

// ---------- hermetic model ----------

struct Scripted {
    steps: Mutex<std::collections::VecDeque<bellum::ModelReply>>,
}

impl Scripted {
    fn new(replies: Vec<bellum::ModelReply>) -> Arc<Self> {
        Arc::new(Scripted {
            steps: Mutex::new(replies.into()),
        })
    }
}

#[async_trait::async_trait]
impl bellum::ModelClient for Scripted {
    fn tier(&self) -> &'static str {
        "terra"
    }
    async fn complete(&self, _p: &str) -> Result<bellum::ModelReply, bellum::BellumError> {
        self.steps
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| bellum::BellumError::Model("script exhausted".into()))
    }
}

fn reply_tool(name: &str, args: serde_json::Value) -> bellum::ModelReply {
    bellum::ModelReply {
        thought: "step".into(),
        tool_calls: vec![bellum::ToolCall {
            name: name.into(),
            args,
        }],
        final_answer: None,
        cost_cents: 1,
    }
}

fn reply_answer(a: &str) -> bellum::ModelReply {
    bellum::ModelReply {
        thought: String::new(),
        tool_calls: vec![],
        final_answer: Some(a.into()),
        cost_cents: 1,
    }
}

fn temp_ws(tag: &str) -> (PathBuf, TempGuard) {
    let dir = std::env::temp_dir().join(format!(
        "bellona-c5-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos() as u64
            * 1_000_000
            + uniq()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let g = TempGuard(dir.clone());
    (dir, g)
}

struct TempGuard(PathBuf);
impl Drop for TempGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn git(dir: &std::path::Path, args: &[&str]) -> String {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@t")
        .output()
        .expect("git");
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

fn ctx(ws: &std::path::Path) -> forge::tool::ToolContext {
    forge::tool::ToolContext {
        agent_id: forge::AgentId::mint(),
        workspace: ws.to_path_buf(),
    }
}

// ---------- the battles ----------

#[tokio::test]
async fn git_campaign_commits_for_real() {
    let (ws, _g) = temp_ws("git");
    git(&ws, &["init", "-q"]);
    std::fs::write(ws.join("seed.txt"), "seed\n").unwrap();

    let cfg = BellonaConfig {
        workspace: ws.clone(),
        yolo: true,
        ..Default::default()
    };
    let a = assemble(&cfg).unwrap();

    let model = Scripted::new(vec![
        reply_tool(
            "write_file",
            serde_json::json!({"path": "src/lib.rs", "content": "fn main(){}"}),
        ),
        reply_tool("git_status", serde_json::json!({})),
        reply_tool(
            "git_commit",
            serde_json::json!({"message": "feat: first blood"}),
        ),
        reply_tool("git_log", serde_json::json!({ "n": 3 })),
        reply_answer("committed"),
    ]);

    let loop_ = bellum::WarLoop::new(
        a.gateway.clone(),
        a.registry.clone(),
        bellum::CascadeRouter::new(vec![model]),
        bellum::Aerarium::default(),
    )
    .with_auto_approver("t");

    let report = loop_
        .run(
            "commit the lib",
            Box::new(bellum::ReActStrategy::new("g", 12)),
            None,
        )
        .await
        .unwrap();

    assert!(report.ok, "breaker: {:?}", report.breaker);

    // The commit REALLY exists.
    let log = git(&ws, &["log", "--oneline", "-n", "5"]);
    assert!(log.contains("first blood"), "log was: {log}");
    // Written file tracked in the commit.
    let show = git(&ws, &["show", "--stat", "--oneline", "HEAD"]);
    assert!(show.contains("src/lib.rs"), "show was: {show}");
}

#[tokio::test]
async fn testkit_battery_over_core_tools() {
    let (ws, _g) = temp_ws("tk");
    let cfg = BellonaConfig {
        workspace: ws.clone(),
        yolo: true,
        ..Default::default()
    };
    let a = assemble(&cfg).unwrap();

    for name in ["read_file", "list_files", "write_file"] {
        let tool = a.registry.get(name).expect(name);
        let rep = conform_fs(tool.as_ref(), &ws).await;
        assert!(rep.ok(), "{name}: {:?}", rep.failures);
    }
}

#[tokio::test]
async fn search_and_chunked_reads_work() {
    let (ws, _g) = temp_ws("search");
    std::fs::write(ws.join("code.rs"), "fn alpha() {}\nfn beta() {}\n").unwrap();

    let tools = bellona::arsenal::search_tools(None);
    let find = |n: &str| tools.iter().find(|t| t.spec().name == n).unwrap().clone();
    let c = ctx(&ws);

    let out = find("search_files")
        .execute(&c, serde_json::json!({"query": "alpha"}))
        .await
        .unwrap();
    assert_eq!(out["matches"], 1);
    assert_eq!(out["results"][0]["line"], 1);

    let out = find("search_files")
        .execute(&c, serde_json::json!({"query": "fn \\w+", "regex": true}))
        .await
        .unwrap();
    assert_eq!(out["matches"], 2);

    // Regex DoS guard refuses pathological patterns at compile time.
    let bad = find("search_files")
        .execute(
            &c,
            serde_json::json!({"query": "((((a{100}){100}){100}){100}){100}", "regex": true}),
        )
        .await;
    assert!(bad.is_err(), "pathological regex must be refused");

    let out = find("read_document")
        .execute(
            &c,
            serde_json::json!({"path": "code.rs", "offset": 1, "limit": 1}),
        )
        .await
        .unwrap();
    assert_eq!(out["returned"], 1);
    assert_eq!(out["total_lines"], 2);
    assert_eq!(out["content"], "fn beta() {}");

    // Binary refusal.
    std::fs::write(ws.join("blob.bin"), [0u8, 1, 2]).unwrap();
    let bin = find("read_document")
        .execute(&c, serde_json::json!({"path": "blob.bin"}))
        .await;
    assert!(bin.is_err(), "binary content must be refused");
}

#[test]
fn ssrf_shield_refuses_the_dark_side() {
    use bellona::arsenal::ssrf_check;
    for host in [
        "127.0.0.1",
        "169.254.169.254",
        "10.0.0.5",
        "192.168.1.1",
        "100.100.1.1",
        "[::1]",
        "[fe80::1]",
        "0.0.0.0",
    ] {
        assert!(ssrf_check(host).is_err(), "{host} must be refused");
    }
    // Structural passes (no network performed for literal IPs).
    assert!(ssrf_check("8.8.8.8").is_ok());
}

#[test]
fn html_stripper_is_sane() {
    let html = "<html><head><style>body{}</style><script>alert(1)</script></head>\
                <body><h1>Hail</h1> <p>Bellona&nbsp;&amp; the <b>legions</b></p></body></html>";
    let text = bellona::arsenal::strip_html_for_tests(html);
    assert!(text.contains("Hail"));
    assert!(text.contains("Bellona &"));
    assert!(!text.contains("alert(1)"));
    assert!(!text.contains('<'));
}
