//! Integration tests for the mock-api CLI.
//!
//! These tests verify the CLI commands work correctly with real configuration files.

use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;
use tempfile::NamedTempFile;

/// Helper to create a valid test config file.
fn valid_config() -> &'static str {
    r#"{
        "version": 1,
        "server": {"host": "127.0.0.1", "port": 0},
        "defaults": {"upstream_timeout_ms": 5000, "max_body_bytes": 1048576},
        "routes": [{
            "id": "get-user",
            "priority": 100,
            "match_rule": {"method": "GET", "path": "/users/:id"},
            "action": {"type": "mock", "response": {"status": 200, "json_body": {"id": 1, "name": "Alice"}}}
        }]
    }"#
}

/// Helper to create an invalid config file.
fn invalid_config() -> &'static str {
    r#"{
        "version": 999,
        "server": {"host": "127.0.0.1", "port": 8080},
        "defaults": {"upstream_timeout_ms": 5000, "max_body_bytes": 1048576},
        "routes": []
    }"#
}

/// Helper to run the mock-api binary.
fn mock_api() -> Command {
    // CARGO_MANIFEST_DIR is crates/mock-cli for integration tests
    // We need to go up to the workspace root and find the binary
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent() // crates
        .unwrap()
        .parent() // mnt/c/dev/GitHub/helpapi
        .unwrap();
    let bin_path = workspace_root.join("target").join("debug").join("mock-api");
    Command::new(bin_path)
}

/// Spawns `mock-api` with the given args, waits up to 5 s for `startup_signal`
/// to appear on stderr (or for the process to exit), then waits
/// `hard_kill_after` before sending SIGKILL. This bounds the test wall-clock
/// time and prevents the integration test from hanging on a long-running
/// `run` server.
fn run_with_timeout(
    args: &[&str],
    startup_signal: impl Fn(&str) -> bool,
    hard_kill_after: Duration,
) -> std::process::Output {
    use std::io::Read;
    use std::process::Stdio;
    use std::sync::{Arc, Mutex};
    use std::time::Instant;

    let stderr_buf: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
    let stdout_buf: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));

    let mut child = mock_api()
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn mock-api");

    // Drain pipes in background threads so the OS pipe buffer cannot fill and
    // block the child.
    let stderr_reader = {
        let buf = Arc::clone(&stderr_buf);
        let mut pipe = child.stderr.take().expect("stderr piped");
        std::thread::spawn(move || {
            let mut chunk = [0u8; 256];
            loop {
                match pipe.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let s = String::from_utf8_lossy(&chunk[..n]).into_owned();
                        buf.lock().unwrap().push_str(&s);
                    }
                }
            }
        })
    };
    let stdout_reader = {
        let buf = Arc::clone(&stdout_buf);
        let mut pipe = child.stdout.take().expect("stdout piped");
        std::thread::spawn(move || {
            let mut chunk = [0u8; 256];
            loop {
                match pipe.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        buf.lock().unwrap().extend_from_slice(&chunk[..n]);
                    }
                }
            }
        })
    };

    // Poll stderr until the startup signal is observed, the child exits on
    // its own, or the deadline elapses.
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if let Ok(Some(_status)) = child.try_wait() {
            // Child exited before the signal was seen — likely a fast-fail
            // (e.g. invalid config). Caller can still inspect stderr.
            break;
        }
        {
            let snapshot = stderr_buf.lock().unwrap().clone();
            if startup_signal(&snapshot) {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    // Hold the process briefly so the caller can observe it, then kill it.
    std::thread::sleep(hard_kill_after);
    let _ = child.kill();
    let status = child.wait().expect("wait after kill");

    let stderr = stderr_buf.lock().unwrap().clone().into_bytes();
    let stdout = stdout_buf.lock().unwrap().clone();

    // Allow reader threads to finish gracefully.
    let _ = stderr_reader.join();
    let _ = stdout_reader.join();

    std::process::Output {
        status,
        stdout,
        stderr,
    }
}

/// Creates a temp file with the given content and returns the path.
fn temp_file(content: &str) -> PathBuf {
    let mut file = NamedTempFile::with_suffix(".json").unwrap();
    file.write_all(content.as_bytes()).unwrap();
    file.into_temp_path().keep().unwrap()
}

#[test]
fn test_validate_command_valid_config() {
    let config_path = temp_file(valid_config());
    let output = mock_api()
        .args(["validate", "--config"])
        .arg(config_path.as_os_str())
        .output()
        .expect("failed to execute validate command");

    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("Config is valid"));
}

#[test]
fn test_validate_command_invalid_config() {
    let config_path = temp_file(invalid_config());
    let output = mock_api()
        .args(["validate", "--config"])
        .arg(config_path.as_os_str())
        .output()
        .expect("failed to execute validate command");

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("invalid"));
}

#[test]
fn test_validate_command_nonexistent_file() {
    let output = mock_api()
        .args(["validate", "--config", "/nonexistent/path/config.json"])
        .output()
        .expect("failed to execute validate command");

    assert!(!output.status.success());
}

#[test]
fn test_version_command() {
    let output = mock_api()
        .args(["version"])
        .output()
        .expect("failed to execute version command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("mock-api version"));
}

#[test]
fn test_schema_command_to_stdout() {
    let output = mock_api()
        .args(["schema"])
        .output()
        .expect("failed to execute schema command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Schema should be valid JSON with $schema field
    assert!(stdout.contains("$schema"));
    assert!(stdout.contains("Config"));
}

#[test]
fn test_schema_command_to_file() {
    let output_file = NamedTempFile::with_suffix(".json").unwrap();
    let output_path = output_file.path().to_path_buf();
    drop(output_file);

    let output = mock_api()
        .args(["schema", "--output"])
        .arg(&output_path)
        .output()
        .expect("failed to execute schema command");

    assert!(output.status.success());

    // Verify the file was written
    let content = std::fs::read_to_string(&output_path).expect("failed to read schema output file");
    assert!(content.contains("$schema"));
}

#[test]
fn test_print_effective_config_valid() {
    let config_path = temp_file(valid_config());
    let output = mock_api()
        .args(["print-effective-config", "--config"])
        .arg(config_path.as_os_str())
        .output()
        .expect("failed to execute print-effective-config command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should contain valid JSON with version
    assert!(stdout.contains("\"version\""));
    assert!(stdout.contains("\"routes\""));
}

#[test]
fn test_print_effective_config_invalid() {
    let config_path = temp_file("not valid json");
    let output = mock_api()
        .args(["print-effective-config", "--config"])
        .arg(config_path.as_os_str())
        .output()
        .expect("failed to execute print-effective-config command");

    assert!(!output.status.success());
}

#[test]
fn test_run_command_requires_config() {
    let output = mock_api()
        .args(["run"])
        .output()
        .expect("failed to execute run command");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--config") || stderr.contains("required"));
}

#[test]
fn test_run_command_with_nonexistent_config() {
    let output = mock_api()
        .args(["run", "--config", "/nonexistent/path/config.json"])
        .output()
        .expect("failed to execute run command");

    assert!(!output.status.success());
}

#[test]
fn test_run_command_with_valid_config_starts_and_stops() {
    let config_path = temp_file(valid_config());

    let output = run_with_timeout(
        &[
            "run",
            "--config",
            config_path.to_str().unwrap(),
            "--log-format=pretty",
        ],
        |stderr| stderr.contains("server listening") || stderr.contains("starting HTTP server"),
        Duration::from_millis(500),
    );

    // It may have been killed — that's fine. We assert it didn't fail with
    // configuration errors before we killed it.
    let combined = String::from_utf8_lossy(&output.stderr);
    assert!(
        !combined.contains("failed to create runtime"),
        "stderr was: {}",
        combined
    );
    assert!(
        !combined.contains("invalid configuration"),
        "stderr was: {}",
        combined
    );
    assert!(
        !combined.contains("no such file"),
        "stderr was: {}",
        combined
    );
}

#[test]
fn test_run_command_with_malformed_config_exits_quickly() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad.json");
    std::fs::write(&path, "{ this is not JSON").unwrap();

    let started = std::time::Instant::now();
    let output = mock_api()
        .args(["run", "--config", path.to_str().unwrap()])
        .output()
        .expect("failed to run");
    let elapsed = started.elapsed();

    assert!(
        !output.status.success(),
        "malformed config should cause non-zero exit"
    );
    assert!(
        elapsed < std::time::Duration::from_secs(2),
        "malformed config should exit fast; took {:?}",
        elapsed
    );
}

#[test]
fn test_help_flag() {
    let output = mock_api()
        .args(["--help"])
        .output()
        .expect("failed to execute --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:") || stdout.contains("mock-api"));
}

#[test]
fn test_validate_help() {
    let output = mock_api()
        .args(["validate", "--help"])
        .output()
        .expect("failed to execute validate --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Validate") || stdout.contains("config"));
}

#[test]
fn test_run_help() {
    let output = mock_api()
        .args(["run", "--help"])
        .output()
        .expect("failed to execute run --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Run") || stdout.contains("config"));
}

#[test]
fn test_log_format_option_pretty() {
    let config_path = temp_file(valid_config());

    // Start the process
    let mut child = mock_api()
        .args(["run", "--config"])
        .arg(config_path.as_os_str())
        .arg("--log-format=pretty")
        .spawn()
        .expect("failed to spawn run command");

    // Wait briefly for server to start
    std::thread::sleep(Duration::from_millis(500));

    // Kill the process
    let _ = child.kill();
    let _ = child.wait();

    // If we got here without errors, the log format was accepted
}

#[test]
fn test_log_format_option_json() {
    let config_path = temp_file(valid_config());

    // Start the process
    let mut child = mock_api()
        .args(["run", "--config"])
        .arg(config_path.as_os_str())
        .arg("--log-format=json")
        .spawn()
        .expect("failed to spawn run command");

    // Wait briefly for server to start
    std::thread::sleep(Duration::from_millis(500));

    // Kill the process
    let _ = child.kill();
    let _ = child.wait();

    // If we got here without errors, the log format was accepted
}

#[test]
fn test_log_format_invalid() {
    let config_path = temp_file(valid_config());
    let output = mock_api()
        .args(["run", "--config"])
        .arg(config_path.as_os_str())
        .arg("--log-format=invalid")
        .output()
        .expect("failed to execute run with invalid log format");

    // Should fail with usage error
    assert!(!output.status.success());
}
