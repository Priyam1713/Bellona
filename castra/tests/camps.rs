use castra::{CampCommand, CastraLevel, EnvScrubPolicy, ProcessDriver, SandboxDriver};

#[test]
fn scrub_policy_strips_secrets_but_keeps_basics() {
    let mut env = std::collections::HashMap::new();
    env.insert("PATH".to_string(), "C:/bin".into());
    env.insert("TERM".to_string(), "xterm".into());
    env.insert("AWS_SECRET_ACCESS_KEY".to_string(), "hunter2".into());
    env.insert("OPENAI_API_KEY".to_string(), "sk-nope".into());

    let scrubbed = EnvScrubPolicy::default().scrub(&env);
    assert!(scrubbed.contains_key("PATH"));
    assert!(scrubbed.contains_key("TERM"));
    assert!(!scrubbed.contains_key("AWS_SECRET_ACCESS_KEY"));
    assert!(!scrubbed.contains_key("OPENAI_API_KEY"));
}

#[tokio::test]
async fn process_driver_runs_scrubbed_and_reports() {
    // Use a command that exists on both Windows and Unix-ish worlds.
    #[cfg(windows)]
    let (prog, args) = ("cmd.exe", vec!["/C".into(), "echo".into(), "hail".into()]);
    #[cfg(not(windows))]
    let (prog, args) = ("echo", vec!["hail".to_string()]);

    let cmd = CampCommand {
        program: prog.into(),
        args,
        working_dir: std::env::temp_dir(),
        timeout_secs: 10,
    };
    let out = ProcessDriver
        .run(&cmd, &EnvScrubPolicy::default())
        .await
        .unwrap();
    assert!(out.exit_ok);
    assert!(out.stdout.to_lowercase().contains("hail"));
    assert_eq!(ProcessDriver.level(), CastraLevel::Prima);
}
