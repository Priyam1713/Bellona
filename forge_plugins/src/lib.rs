//! # forge-plugins — the Armory's locked boxes.
//!
//! Third-party power without third-party trust (Law II + IV):
//! - bundles are `manifest.json` + `plugin.wasm`, hash-bound;
//! - capabilities are DENY-BY-DEFAULT: an ungranted import means the plugin
//!   cannot even instantiate;
//! - the only granted capability today is `cap_log`; `cap_http`/`cap_fs`
//!   land when their gate-backed implementations exist (never ambient).
//!
//! ABI v0.1 (deliberately toy, component-model comes later):
//! - export `mem` (memory, ≥1 page)
//! - export `call(in_ptr: i32, in_len: i32) -> i32` (returns result length)
//! - result bytes live at offset 1024
//! - input bytes are written by the host at offset 0 (≤ 64 KiB)

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;
use wasmtime::Engine;

pub const ABI_VERSION: &str = "0.1";
pub const RESULT_OFFSET: u32 = 1024;
pub const MAX_INPUT: usize = 64 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("manifest invalid: {0}")]
    Manifest(String),

    #[error("hash mismatch: manifest {declared}, actual {actual}")]
    HashMismatch { declared: String, actual: String },

    #[error("capability '{0}' required but not granted")]
    CapabilityDenied(String),

    #[error("abi violation: {0}")]
    Abi(String),

    #[error("runtime: {0}")]
    Runtime(String),
}

/// Signed-at-publish metadata; `wasm_sha256` binds it to the bytes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    /// Capabilities the plugin REQUIRES. Anything outside this list is
    /// impossible — imports are absent from the module.
    pub caps: Vec<String>,
    pub abi_version: String,
    pub wasm_sha256: String,
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    hex::encode(h.finalize())
}

/// Verify a bundle directory: manifest ↔ bytes binding.
pub fn verify_bundle(dir: &Path) -> Result<(PluginManifest, Vec<u8>), PluginError> {
    let manifest_raw = std::fs::read(dir.join("manifest.json"))
        .map_err(|e| PluginError::Manifest(format!("read: {e}")))?;
    let manifest: PluginManifest = serde_json::from_slice(&manifest_raw)
        .map_err(|e| PluginError::Manifest(format!("parse: {e}")))?;

    if manifest.abi_version != ABI_VERSION {
        return Err(PluginError::Manifest(format!(
            "abi {} != host {}",
            manifest.abi_version, ABI_VERSION
        )));
    }

    let wasm = std::fs::read(dir.join("plugin.wasm"))
        .map_err(|e| PluginError::Manifest(format!("missing plugin.wasm: {e}")))?;
    let actual = sha256_hex(&wasm);
    if actual != manifest.wasm_sha256.to_lowercase() {
        return Err(PluginError::HashMismatch {
            declared: manifest.wasm_sha256,
            actual,
        });
    }
    Ok((manifest, wasm))
}

// ---------- the host ----------

pub struct WasmHost {
    engine: Engine,
}

/// A loaded plugin bound to exactly its granted capabilities.
#[derive(Debug)]
pub struct LoadedPlugin {
    #[allow(dead_code)]
    manifest: PluginManifest,
    module: wasmtime::Module,
    granted_caps: Vec<String>,
}

impl WasmHost {
    pub fn new() -> Result<Self, PluginError> {
        let engine = Engine::default();
        Ok(WasmHost { engine })
    }

    /// Compile a verified bundle with its grant set. Compilation is lazy-safe:
    /// capability enforcement happens at link/instantiate time per instance.
    pub fn load(
        &self,
        manifest: PluginManifest,
        wasm: Vec<u8>,
        granted_caps: &[&str],
    ) -> Result<LoadedPlugin, PluginError> {
        // Deny-by-default: every REQUIRED cap must be explicitly granted.
        for c in &manifest.caps {
            if !granted_caps.contains(&c.as_str()) {
                return Err(PluginError::CapabilityDenied(c.clone()));
            }
        }
        let module = wasmtime::Module::new(&self.engine, &wasm)
            .map_err(|e| PluginError::Abi(format!("compile: {e}")))?;
        Ok(LoadedPlugin {
            manifest,
            module,
            granted_caps: granted_caps.iter().map(|s| s.to_string()).collect(),
        })
    }
}

impl LoadedPlugin {
    /// One isolated call. Input ≤ MAX_INPUT is written at offset 0; the
    /// plugin's answer lives at RESULT_OFFSET with length = call's return.
    pub fn call(&self, input: &[u8]) -> Result<Vec<u8>, PluginError> {
        if input.len() > MAX_INPUT {
            return Err(PluginError::Abi(format!(
                "input {} > max {MAX_INPUT}",
                input.len()
            )));
        }
        let mut store = wasmtime::Store::new(self.module.engine(), ());
        let mut linker = wasmtime::Linker::new(self.module.engine());

        // Grant cap_log ONLY if it was granted at load.
        if self.granted_caps.contains(&"cap_log".to_string()) {
            linker
                .func_wrap(
                    "bellona",
                    "log",
                    |_caller: wasmtime::Caller<'_, ()>, _level: i32, _ptr: i32, _len: i32| {
                        // v0.1: log lines route to tracing in M-V; sink now.
                    },
                )
                .map_err(|e| PluginError::Runtime(e.to_string()))?;
        }

        let instance = linker.instantiate(&mut store, &self.module).map_err(|e| {
            // Missing imports surface here — the deny-by-default proof.
            PluginError::CapabilityDenied(format!("link failed: {e}"))
        })?;

        let mem = instance
            .get_memory(&mut store, "mem")
            .ok_or_else(|| PluginError::Abi("export 'mem' missing".into()))?;

        let call = instance
            .get_typed_func::<(i32, i32), i32>(&mut store, "call")
            .map_err(|e| PluginError::Abi(format!("export 'call' missing: {e}")))?;

        mem.write(&mut store, 0, input)
            .map_err(|e| PluginError::Abi(format!("input write: {e}")))?;

        let out_len = call
            .call(&mut store, (0, input.len() as i32))
            .map_err(|e| PluginError::Runtime(format!("trap: {e}")))?;
        let out_len = out_len.max(0) as u32 as usize;

        let mut out = vec![0u8; out_len];
        mem.read(&store, RESULT_OFFSET as usize, &mut out)
            .map_err(|e| PluginError::Abi(format!("result read: {e}")))?;
        Ok(out)
    }
}
