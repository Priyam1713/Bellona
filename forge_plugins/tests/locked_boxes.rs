//! Campaign XII: the locked boxes. Capability denial, hash binding, and a
//! real WAT plugin executing inside wasmtime.

use forge_plugins::{sha256_hex, verify_bundle, PluginManifest, WasmHost};

const PLUGIN_WAT: &str = r#"
(module
  (import "bellona" "log" (func $log (param i32 i32 i32)))
  (memory (export "mem") 1)
  (data (i32.const 1024) "plugin-ok")
  (func (export "call") (param $p i32) (param $l i32) (result i32)
    (if (i32.gt_s (local.get $l) (i32.const 0))
      (then (call $log (i32.const 1) (i32.const 1024) (i32.const 9))))
    (i32.const 9))
)"#;

fn wat_bytes() -> Vec<u8> {
    // wasmtime's Module::new auto-detects and compiles text format.
    PLUGIN_WAT.as_bytes().to_vec()
}

fn write_bundle(dir: &std::path::Path, caps: &[&str], mutate_hash: Option<&str>) -> PluginManifest {
    std::fs::create_dir_all(dir).unwrap();
    let wasm = wat_bytes();
    let hash = mutate_hash
        .map(String::from)
        .unwrap_or_else(|| sha256_hex(&wasm));
    let manifest = PluginManifest {
        name: "greeter".into(),
        version: "0.1.0".into(),
        caps: caps.iter().map(|s| s.to_string()).collect(),
        abi_version: "0.1".into(),
        wasm_sha256: hash,
    };
    std::fs::write(
        dir.join("manifest.json"),
        serde_json::to_string_pretty(&manifest).unwrap(),
    )
    .unwrap();
    std::fs::write(dir.join("plugin.wasm"), &wasm).unwrap();
    manifest
}

#[test]
fn bundle_verification_binds_manifest_to_bytes() {
    let dir = std::env::temp_dir().join(format!("fp-verify-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    let manifest = write_bundle(&dir, &["cap_log"], None);
    let (_, wasm) = verify_bundle(&dir).unwrap();
    assert!(!wasm.is_empty());
    assert_eq!(manifest.name, "greeter");

    // Tamper: declare a different hash â†’ refused.
    let tampered: Option<&str> = Some(&"deadbeef".repeat(8));
    write_bundle(&dir, &["cap_log"], tampered);
    assert!(matches!(
        verify_bundle(&dir),
        Err(forge_plugins::PluginError::HashMismatch { .. })
    ));
}

#[test]
fn granted_capability_executes_end_to_end() {
    let dir = std::env::temp_dir().join(format!("fp-exec-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (manifest, wasm) = verify_bundle_ok(&dir);

    let host = WasmHost::new().unwrap();
    let plugin = host.load(manifest, wasm, &["cap_log"]).unwrap();
    let out = plugin.call(b"hello").unwrap();
    assert_eq!(String::from_utf8_lossy(&out), "plugin-ok");
}

#[test]
fn ungranted_capability_is_denied_at_instantiation() {
    let dir = std::env::temp_dir().join(format!("fp-deny-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (manifest, wasm) = verify_bundle_ok(&dir); // requires cap_log

    let host = WasmHost::new().unwrap();
    // Grant NOTHING → load refuses before compilation even happens.
    // This is the deny-by-default proof: ungranted caps cannot start.
    match host.load(manifest, wasm, &[]) {
        Err(forge_plugins::PluginError::CapabilityDenied(c)) => assert_eq!(c, "cap_log"),
        other => panic!("expected capability denial, got {other:?}"),
    }
}

fn verify_bundle_ok(dir: &std::path::Path) -> (PluginManifest, Vec<u8>) {
    write_bundle(dir, &["cap_log"], None);
    verify_bundle(dir).unwrap()
}
