//! Campaign V â€” the expanded arsenal: git operations, document search,
//! HTTP-level web reading. Every tool is a SimpleTool (~15 lines each) and
//! must pass the forge-testkit battery.
//!
//! Handler pattern (mandatory): capture owned values BEFORE `async move`.

use crate::resolve_in_workspace;
use castra::{CampCommand, EnvScrubPolicy, ProcessDriver, SandboxDriver};
use forge::primitives::EffectKind;
use forge::simple_tool::{need_str, opt_str, SimpleTool};
use forge::{ForgeError, ForgeResult};
use serde_json::json;
use std::path::Path;
use std::sync::Arc;

// ---------- shared plumbing ----------

async fn run_camp(
    ws: &Path,
    program: &str,
    args: Vec<String>,
) -> Result<serde_json::Value, String> {
    let cmd = CampCommand {
        program: program.into(),
        args,
        working_dir: ws.to_path_buf(),
        timeout_secs: 60,
    };
    let out = ProcessDriver
        .run(&cmd, &EnvScrubPolicy::default())
        .await
        .map_err(|e| e.to_string())?;
    Ok(json!({
        "exit_ok": out.exit_ok,
        "stdout": out.stdout,
        "stderr": out.stderr,
    }))
}

/// Platform-correct `git <argv>` inside the workspace.
///
/// Direct exec â€” NO shell layer. Shell quoting across cmd/POSIX was proven
/// lossy in tests (`-c user.name=...` arrived with literal quotes); git is
/// a real executable everywhere we care about, so argv goes to it raw.
async fn run_git(ws: &Path, argv: Vec<String>) -> ForgeResult<serde_json::Value> {
    run_camp(ws, "git", argv).await.map_err(ForgeError::Other)
}

// ---------- V2: git ----------

pub fn git_read_tools() -> Vec<Arc<dyn forge::tool::Tool>> {
    vec![
        Arc::new(SimpleTool::new(
            "git_status",
            "Working-tree status, porcelain format. Args: {}",
            EffectKind::FileRead,
            true,
            json!({}),
            move |ctx, _a| {
                let ws = ctx.workspace.clone();
                Box::pin(
                    async move { run_git(&ws, vec!["status".into(), "--porcelain".into()]).await },
                )
            },
        )),
        Arc::new(SimpleTool::new(
            "git_log",
            "Recent commits, oneline graph. Args: {n?: int}",
            EffectKind::FileRead,
            true,
            json!({}),
            move |ctx, a| {
                let ws = ctx.workspace.clone();
                let n = a
                    .get("n")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(20)
                    .to_string();
                Box::pin(async move {
                    run_git(
                        &ws,
                        vec![
                            "log".into(),
                            "--oneline".into(),
                            "--graph".into(),
                            "-n".into(),
                            n,
                        ],
                    )
                    .await
                })
            },
        )),
        Arc::new(SimpleTool::new(
            "git_diff",
            "Working-tree diff. Args: {args?: str}",
            EffectKind::FileRead,
            true,
            json!({}),
            move |ctx, a| {
                let ws = ctx.workspace.clone();
                let extra = opt_str(&a, "args", "--stat");
                Box::pin(async move {
                    let mut argv = vec!["diff".into()];
                    argv.extend(extra.split_whitespace().map(String::from));
                    run_git(&ws, argv).await
                })
            },
        )),
    ]
}

pub fn git_write_tools() -> Vec<Arc<dyn forge::tool::Tool>> {
    vec![
        Arc::new(SimpleTool::new(
            "git_commit",
            "Stage all changes and commit with camp-scoped identity. Args: {message: str}",
            EffectKind::FileWrite,
            false,
            json!({}),
            move |ctx, a| {
                let ws = ctx.workspace.clone();
                let msg = match need_str(&a, "message") {
                    Ok(m) => m,
                    Err(e) => return Box::pin(async move { Err(e) }),
                };
                Box::pin(async move {
                    let ident = vec![
                        "-c".to_string(),
                        "user.name=Bellona Agent".into(),
                        "-c".to_string(),
                        "user.email=agent@bellona.local".into(),
                    ];
                    let mut add = ident.clone();
                    add.extend(["add".to_string(), "-A".to_string()]);
                    run_git(&ws, add).await?;
                    let mut commit = ident;
                    commit.extend(["commit".to_string(), "-m".to_string(), msg]);
                    run_git(&ws, commit).await
                })
            },
        )),
        Arc::new(SimpleTool::new(
            "git_branch",
            "Create or switch branch. Args: {name: str, create?: bool}",
            EffectKind::FileWrite,
            false,
            json!({}),
            move |ctx, a| {
                let ws = ctx.workspace.clone();
                let name = match need_str(&a, "name") {
                    Ok(n) => n,
                    Err(e) => return Box::pin(async move { Err(e) }),
                };
                Box::pin(async move {
                    let mut argv = vec!["switch".to_string()];
                    if a.get("create").and_then(|v| v.as_bool()).unwrap_or(false) {
                        argv.push("-c".to_string());
                    }
                    argv.push(name);
                    run_git(&ws, argv).await
                })
            },
        )),
    ]
}

// ---------- V3: document search ----------

pub fn search_tools(
    store: Option<Arc<dyn memoria::ArchivumStore>>,
) -> Vec<Arc<dyn forge::tool::Tool>> {
    vec![
        Arc::new(SimpleTool::new(
            "search_files",
            "Search file contents (literal by default). Args: {query: str, regex?: bool}. Cap 200 hits.",
            EffectKind::Custom("search_files".into()), true, json!({}),
            move |ctx, a| {
                let ws = ctx.workspace.clone();
                let store = store.clone();
                let query = match need_str(&a, "query") {
                    Ok(q) => q,
                    Err(e) => return Box::pin(async move { Err(e) }),
                };
                let regex_on = a.get("regex").and_then(|v| v.as_bool()).unwrap_or(false);
                Box::pin(async move { search_impl(&ws, &query, regex_on, store).await })
            },
        )),
        Arc::new(SimpleTool::new(
            "read_document",
            "Read file lines [offset, offset+limit). Args: {path: str, offset?: int, limit?: int}",
            EffectKind::FileRead, true, json!({}),
            move |ctx, a| {
                let ws = ctx.workspace.clone();
                let path = match need_str(&a, "path") {
                    Ok(p) => p,
                    Err(e) => return Box::pin(async move { Err(e) }),
                };
                let offset = a.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let limit = a.get("limit").and_then(|v| v.as_u64()).unwrap_or(200).clamp(1, 1000) as usize;
                Box::pin(async move {
                    let full = resolve_in_workspace(&ws, &path)?;
                    let raw = std::fs::read_to_string(&full).map_err(ForgeError::Io)?;
                    if raw.contains('\0') {
                        return Err(ForgeError::Other("refusing binary content".into()));
                    }
                    let total = raw.lines().count();
                    let slice: Vec<&str> = raw.lines().skip(offset).take(limit).collect();
                    Ok(json!({
                        "path": path, "total_lines": total,
                        "offset": offset, "returned": slice.len(),
                        "content": slice.join("\n"),
                    }))
                })
            },
        )),
    ]
}

async fn search_impl(
    ws: &Path,
    query: &str,
    regex_on: bool,
    store: Option<Arc<dyn memoria::ArchivumStore>>,
) -> ForgeResult<serde_json::Value> {
    const CAP: usize = 200;
    let matcher = build_matcher(query, regex_on)?;

    let mut hits: Vec<serde_json::Value> = Vec::new();
    collect_hits(ws, ws, matcher.as_ref(), &mut hits, CAP)?;

    // V3.3: searches become memories â€” top hits land in the Archivum.
    if let Some(s) = &store {
        for h in hits.iter().take(10) {
            let content = format!(
                "{}:{} {}",
                h["path"].as_str().unwrap_or(""),
                h["line"],
                h["text"].as_str().unwrap_or("")
            );
            let _ = s.put(memoria::new_episode("episodic", content)).await;
        }
    }

    Ok(json!({ "matches": hits.len(), "results": hits }))
}

type LineMatcher = Box<dyn Fn(&str) -> bool + Send>;
fn build_matcher(query: &str, regex_on: bool) -> ForgeResult<LineMatcher> {
    if !regex_on {
        let q = query.to_lowercase();
        return Ok(Box::new(move |line| line.to_lowercase().contains(&q)));
    }
    let re = regex::RegexBuilder::new(query)
        .size_limit(1 << 20)
        .dfa_size_limit(1 << 20)
        .build()
        .map_err(|e| ForgeError::Other(format!("bad regex: {e}")))?;
    Ok(Box::new(move |line| re.is_match(line)))
}

fn collect_hits(
    root: &Path,
    dir: &Path,
    matcher: &dyn Fn(&str) -> bool,
    out: &mut Vec<serde_json::Value>,
    cap: usize,
) -> ForgeResult<()> {
    if out.len() >= cap {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir).map_err(ForgeError::Io)? {
        let entry = entry.map_err(ForgeError::Io)?;
        let p = entry.path();
        if p.is_dir() {
            let name = p
                .file_name()
                .map(|s| s.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            if matches!(name.as_str(), ".git" | "target" | "node_modules") {
                continue;
            }
            collect_hits(root, &p, matcher, out, cap)?;
        } else if p.is_file() {
            let Ok(raw) = std::fs::read_to_string(&p) else {
                continue;
            };
            if raw.contains('\0') {
                continue; // binary refused early
            }
            let rel = p
                .strip_prefix(root)
                .unwrap_or(&p)
                .to_string_lossy()
                .replace('\\', "/");
            for (i, line) in raw.lines().enumerate() {
                if matcher(line) {
                    out.push(json!({
                        "path": rel,
                        "line": i + 1,
                        "text": line.chars().take(240).collect::<String>(),
                    }));
                    if out.len() >= cap {
                        return Ok(());
                    }
                }
            }
        }
    }
    Ok(())
}

// ---------- V4: web reading with SSRF shield ----------

/// Refuse loopback / private / link-local / CGNAT / cloud-metadata targets.
pub fn ssrf_check(host: &str) -> Result<(), String> {
    use std::net::{IpAddr, ToSocketAddrs};
    let refused = |ip: IpAddr| -> bool {
        match ip {
            IpAddr::V4(v4) => {
                let o = v4.octets();
                v4.is_loopback()
                    || v4.is_private()
                    || v4.is_link_local()
                    || v4.is_unspecified()
                    || o[0] == 169 && o[1] == 254      // link-local metadata
                    || o[0] == 100 && (o[1] & 0xC0) == 64 // CGNAT 100.64/10
                    || o[0] == 0 // this-network
            }
            IpAddr::V6(v6) => {
                v6.is_loopback()
                    || v6.is_unspecified()
                    || (v6.segments()[0] & 0xfe00) == 0xfc00 // ULA
                    || (v6.segments()[0] & 0xffc0) == 0xfe80 // link-local
            }
        }
    };

    let bare = host.trim_start_matches('[');
    let bare = bare.split(']').next().unwrap_or(bare);
    let bare = bare.rsplit(':').next().unwrap_or(bare);

    if let Ok(ip) = bare.parse::<IpAddr>() {
        return if refused(ip) {
            Err(format!("refused SSRF target {ip}"))
        } else {
            Ok(())
        };
    }
    let addrs: Vec<std::net::SocketAddr> = format!("{bare}:443")
        .to_socket_addrs()
        .map_err(|e| format!("dns resolution failed for '{bare}': {e}"))?
        .collect();
    if addrs.is_empty() {
        return Err(format!("host '{bare}' does not resolve"));
    }
    for sa in addrs {
        if refused(sa.ip()) {
            return Err(format!(
                "'{bare}' resolves into private space ({})",
                sa.ip()
            ));
        }
    }
    Ok(())
}

fn resolve_redirect(base: &str, loc: &str) -> String {
    if loc.starts_with("http://") || loc.starts_with("https://") {
        return loc.to_string();
    }
    let (scheme, rest) = base.split_once("://").unwrap_or(("https", base));
    let authority = rest.split('/').next().unwrap_or_default();
    if loc.starts_with('/') {
        format!("{scheme}://{authority}{loc}")
    } else {
        format!("{base}/{loc}")
    }
}

fn host_of(url: &str) -> String {
    url.split("://")
        .nth(1)
        .unwrap_or_default()
        .split('/')
        .next()
        .unwrap_or_default()
        .to_string()
}

/// Strip HTML to rough readable text without external dependencies.
pub fn strip_html_for_tests(html: &str) -> String {
    let mut stripped = String::with_capacity(html.len());
    {
        let mut rest = html;
        loop {
            let next_script = find_ci(rest, "<script", "</script>");
            let next_style = find_ci(rest, "<style", "</style>");
            match [next_script, next_style]
                .into_iter()
                .flatten()
                .min_by_key(|x| x.0)
            {
                Some((start, end)) => {
                    stripped.push_str(&rest[..start]);
                    rest = &rest[end..];
                }
                None => {
                    stripped.push_str(rest);
                    break;
                }
            }
        }
    }
    let mut out = String::with_capacity(stripped.len());
    let mut in_tag = false;
    for ch in stripped.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                out.push(' ');
            }
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    let out = out
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ");
    out.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn find_ci(hay: &str, open: &str, close: &str) -> Option<(usize, usize)> {
    let start = hay.to_lowercase().find(open)?;
    let end_off = hay[start..].to_lowercase().find(close)?;
    Some((start, start + end_off + close.len()))
}

pub fn web_fetch_tool(http: reqwest::Client) -> Arc<SimpleTool> {
    SimpleTool::into_arc(SimpleTool::new(
        "web_fetch",
        "Fetch a public http(s) page as readable text (GET only; every redirect re-checked against the private-space shield). Args: {url: str}",
        EffectKind::BrowserNavigate, true, json!({}),
        move |_ctx, a| {
            let http = http.clone();
            let url = match need_str(&a, "url") {
                Ok(u) => u,
                Err(e) => return Box::pin(async move { Err(e) }),
            };
            Box::pin(async move {
                if !(url.starts_with("http://") || url.starts_with("https://")) {
                    return Err(ForgeError::Other("only http(s) schemes allowed".into()));
                }
                ssrf_check(&host_of(&url)).map_err(ForgeError::Other)?;

                let mut current = url.clone();
                let mut hops = 0u8;
                let body_text = loop {
                    if hops >= 5 {
                        return Err(ForgeError::Other("too many redirects".into()));
                    }
                    let resp = http.get(&current).send().await.map_err(|e| ForgeError::Other(e.to_string()))?;
                    if resp.status().is_redirection() {
                        let loc = resp
                            .headers()
                            .get("location")
                            .and_then(|v| v.to_str().ok())
                            .ok_or_else(|| ForgeError::Other("redirect without location".into()))?
                            .to_string();
                        let next = resolve_redirect(&current, &loc);
                        ssrf_check(&host_of(&next)).map_err(ForgeError::Other)?;
                        current = next;
                        hops += 1;
                        continue;
                    }
                    let status = resp.status();
                    if !status.is_success() {
                        return Err(ForgeError::Other(format!("http {status}")));
                    }
                    let text = resp.text().await.map_err(|e| ForgeError::Other(e.to_string()))?;
                    break text.chars().take(400_000).collect::<String>();
                };

                let title = body_text
                    .split("<title>")
                    .nth(1)
                    .and_then(|t| t.split("</title>").next())
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                let plain = strip_html_for_tests(&body_text);
                Ok(json!({
                    "url": current,
                    "title": title,
                    "chars": plain.chars().count(),
                    "text": plain.chars().take(60_000).collect::<String>(),
                }))
            })
        },
    ))
}

