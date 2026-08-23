//! Campaign XIV-6: skills installer â€” clone, parse foreign frontmatter,
//! list, remove.

use bellona::skills_cli;
use std::path::PathBuf;
use std::process::Command;

fn temp(tag: &str) -> (PathBuf, Guard) {
    let dir = std::env::temp_dir().join(format!(
        "bellona-skills-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    (dir.clone(), Guard(dir))
}
struct Guard(PathBuf);
impl Drop for Guard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn git(dir: &std::path::Path, args: &[&str]) {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@t")
        .output()
        .expect("git");
    assert!(out.status.success(), "git {:?} failed: {}", args,
        String::from_utf8_lossy(&out.stderr));
}

#[test]
fn install_from_local_git_url_round_trip() {
    let (_src, srcg) = temp("src");
    let src = PathBuf::from(&srcg.0);

    // A foreign-style pack with slightly imperfect frontmatter.
    let pack = src.join("packs").join("hello-skill");
    std::fs::create_dir_all(&pack).unwrap();
    std::fs::write(
        pack.join("SKILL.md"),
        "---\nname: hello-skill\nversion: 1.2.0\ndescription: says hello on demand\n---\n\n# Hello\nSay hello when triggered.\n",
    )
    .unwrap();
    // Second pack, missing description (tolerant parser).
    let pack2 = src.join("minimal");
    std::fs::create_dir_all(&pack2).unwrap();
    std::fs::write(pack2.join("SKILL.md"), "---\nname: minimal\n---\nbody\n").unwrap();

    git(&src, &["init", "-q"]);
    git(&src, &["add", "-A"]);
    git(&src, &["commit", "-q", "-m", "packs"]);

    let (root, rootg) = temp("root");

    let url = format!("file:///{}", src.to_string_lossy().replace('\\', "/"));
    let installed = skills_cli::install_from_git(&url, &root).unwrap();
    assert_eq!(installed.len(), 2);

    let all = skills_cli::scan(&root);
    assert_eq!(all.len(), 2);
    assert!(all.iter().any(|s| s.name == "hello-skill" && s.version == "1.2.0"));
    assert!(all.iter().any(|s| s.name == "minimal" && s.version == "0.0.0"),
        "missing version tolerated as 0.0.0");

    // Remove one; the other survives.
    assert!(skills_cli::remove(&root, "hello-skill").unwrap());
    let after = skills_cli::scan(&root);
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].name, "minimal");
}

#[test]
fn frontmatter_parser_is_tolerant() {
    let ok = skills_cli::parse_frontmatter("---\nname: x\nversion: 2.0.0\n---\n");
    assert!(ok.is_ok());

    let no_name = skills_cli::parse_frontmatter("---\nversion: 1.0.0\n---\n");
    assert!(no_name.is_err(), "name is the only hard requirement");

    let garbage = skills_cli::parse_frontmatter("not frontmatter at all");
    assert!(garbage.is_err());
}

