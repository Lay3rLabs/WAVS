//! Subprocess runner smoke test — spawn the real `wavs` binary, poll /health,
//! shut it down cleanly.
//!
//! Skips gracefully if the binary can't be located (`WAVS_BINARY` unset and
//! no `target/{debug,release}/wavs` present). Run `cargo build -p wavs` to
//! enable it locally.

#![cfg(feature = "subprocess")]

use std::time::Duration;

use wavs_test_harness::service::{
    runner_subprocess::resolve_wavs_binary_for_tests, ServiceSpec, SubprocessConfig,
    SubprocessRunner,
};

fn locate_or_skip() -> Option<std::path::PathBuf> {
    let Ok(bin) = resolve_wavs_binary_for_tests() else {
        eprintln!(
            "[skipping] wavs binary not found. Run `cargo build -p wavs` or set WAVS_BINARY=/path/to/wavs"
        );
        return None;
    };
    // Quick exec-check: a stale macOS binary on a Linux runner shows up as
    // "Exec format error" — skip rather than fail the test.
    match std::process::Command::new(&bin).arg("--version").output() {
        Ok(out) if out.status.success() || !out.stderr.is_empty() => Some(bin),
        Ok(_) => {
            eprintln!("[skipping] wavs binary at {bin:?} ran but emitted no output");
            None
        }
        Err(e) => {
            eprintln!("[skipping] wavs binary at {bin:?} is not executable on this platform: {e}");
            None
        }
    }
}

#[tokio::test]
async fn subprocess_spawn_and_health() {
    let _ = tracing_subscriber::fmt::try_init();
    let Some(bin) = locate_or_skip() else {
        return;
    };

    let spec = ServiceSpec::new();
    let config = SubprocessConfig::new()
        .wavs_binary(&bin)
        .startup_timeout(Duration::from_secs(45));
    let mut runner = SubprocessRunner::new(config, spec).expect("build runner");
    let base = runner.start().await.expect("start subprocess");
    assert!(base.starts_with("http://127.0.0.1:"));

    let health = runner.health().await.expect("query health");
    eprintln!("[subprocess] /health -> {health}");

    let status = runner.shutdown().await.expect("shutdown");
    eprintln!("[subprocess] exited with {status}");
}
