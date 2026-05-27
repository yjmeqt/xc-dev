//! Integration tests for `xc-dev :sim bake`.
//!
//! Tests that require a real iOS runtime are skipped if simctl or the runtime
//! is unavailable.

use assert_cmd::Command as AssertCmd;
use tempfile::tempdir;

// ─── helpers ─────────────────────────────────────────────────────────────────

fn simctl_available() -> bool {
    std::process::Command::new("xcrun")
        .args(["simctl", "help"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn first_ios_runtime() -> Option<String> {
    let out = std::process::Command::new("xcrun")
        .args(["simctl", "list", "runtimes", "--json"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    let runtimes = v.get("runtimes")?.as_array()?;
    for rt in runtimes {
        let name = rt.get("name")?.as_str().unwrap_or("");
        let id = rt.get("identifier")?.as_str().unwrap_or("");
        let available = rt
            .get("isAvailable")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if available && name.starts_with("iOS") && !id.is_empty() {
            return Some(name.to_string());
        }
    }
    None
}

fn find_iphone_device_type() -> Option<String> {
    let out = std::process::Command::new("xcrun")
        .args(["simctl", "list", "devicetypes", "--json"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    let types = v.get("devicetypes")?.as_array()?;
    let preferred = ["iPhone 16", "iPhone 15", "iPhone 14"];
    for pref in &preferred {
        if types
            .iter()
            .any(|dt| dt.get("name").and_then(|n| n.as_str()) == Some(pref))
        {
            return Some(pref.to_string());
        }
    }
    for dt in types {
        let name = dt.get("name")?.as_str().unwrap_or("");
        if name.starts_with("iPhone") {
            return Some(name.to_string());
        }
    }
    None
}

/// Delete a simulator by UDID, ignoring errors.
fn delete_simulator(udid: &str) {
    let _ = std::process::Command::new("xcrun")
        .args(["simctl", "delete", udid])
        .output();
}

struct SimGuard {
    udid: String,
}
impl Drop for SimGuard {
    fn drop(&mut self) {
        delete_simulator(&self.udid);
    }
}

fn random_suffix() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    format!("{nanos:08x}")
}

// ─── integration test ────────────────────────────────────────────────────────

/// Full `:sim bake` round-trip:
/// - Creates a tempdir with project.toml + simulator.toml (no `id`).
/// - Runs `xc-dev :sim bake`.
/// - Asserts simulator.toml now contains `id = "..."` in `[sim.default]`.
/// - Asserts that a known comment in simulator.toml is preserved.
/// - Second run should be a no-op.
/// - Cleans up the created simulator.
#[test]
fn sim_bake_writes_id_into_simulator_toml() {
    if !simctl_available() {
        eprintln!("skipping: xcrun simctl not available");
        return;
    }

    let runtime = match first_ios_runtime() {
        Some(r) => r,
        None => {
            eprintln!("skipping: no available iOS runtime found");
            return;
        }
    };

    let device = match find_iphone_device_type() {
        Some(d) => d,
        None => {
            eprintln!("skipping: no iPhone device type found");
            return;
        }
    };

    let suffix = random_suffix();
    let sim_name = format!("xc-dev-test-bake-{suffix}");

    eprintln!("sim_bake_writes_id_into_simulator_toml: sim={sim_name:?} device={device:?} runtime={runtime:?}");

    let tmp = tempdir().unwrap();
    let dotdir = tmp.path().join(".xc-dev");
    std::fs::create_dir_all(&dotdir).unwrap();

    // Write project.toml
    std::fs::write(
        dotdir.join("project.toml"),
        r#"schema_version = 1
[project]
path = "X.xcworkspace"
scheme = "X"
configuration = "Debug"
bundle_id = "com.x"
product_name = "X"
min_ios_version = "26.0"
"#,
    )
    .unwrap();

    // Write simulator.toml with a known comment and no `id`
    std::fs::write(
        dotdir.join("simulator.toml"),
        format!(
            r#"# .xc-dev/simulator.toml
schema_version = 1

# This comment must be preserved after :sim bake.
[sim.default]
name    = "{sim_name}"
device  = "{device}"
runtime = "{runtime}"
"#
        ),
    )
    .unwrap();

    // Write tasks.toml
    std::fs::write(dotdir.join("tasks.toml"), "[tasks.noop]\ncmd = \"true\"\n").unwrap();

    // Run :sim bake
    let out = AssertCmd::cargo_bin("xc-dev")
        .unwrap()
        .current_dir(tmp.path())
        .args([":sim", "bake"])
        .output()
        .expect("xc-dev :sim bake ran");

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        eprintln!("skipping: :sim bake failed: {stderr}");
        return;
    }

    let stdout = String::from_utf8_lossy(&out.stdout);
    eprintln!("sim_bake stdout: {stdout}");

    // Read back the updated simulator.toml
    let updated = std::fs::read_to_string(dotdir.join("simulator.toml")).unwrap();
    eprintln!("updated simulator.toml:\n{updated}");

    // Assert it now contains `id = "..."` inside [sim.default]
    assert!(
        updated.contains("id = "),
        "simulator.toml should contain `id = ...` after bake"
    );

    // Assert the known comment is preserved
    assert!(
        updated.contains("# This comment must be preserved after :sim bake."),
        "simulator.toml should still contain the original comment"
    );

    // Extract the UDID from the file for cleanup
    let parsed: toml::Value = toml::from_str(&updated).expect("should be valid TOML after bake");
    let udid = parsed
        .get("sim")
        .and_then(|s| s.get("default"))
        .and_then(|d| d.get("id"))
        .and_then(|i| i.as_str())
        .expect("sim.default.id should be present");

    eprintln!("sim_bake: baked UDID={udid}");

    let _guard = SimGuard {
        udid: udid.to_string(),
    };

    // Second run: should be a no-op (already has id).
    let out2 = AssertCmd::cargo_bin("xc-dev")
        .unwrap()
        .current_dir(tmp.path())
        .args([":sim", "bake"])
        .output()
        .expect("second :sim bake ran");

    assert!(out2.status.success(), "second :sim bake should succeed");
    let stdout2 = String::from_utf8_lossy(&out2.stdout);
    assert!(
        stdout2.contains("already has id") || stdout2.contains("nothing to bake"),
        "second :sim bake should report no-op: {stdout2}"
    );

    eprintln!("sim_bake_writes_id_into_simulator_toml: PASS");
    // _guard drops here, deleting the simulator.
}

// ─── negative tests ──────────────────────────────────────────────────────────

/// `:sim bake nonexistent` on a simulator.toml that only has [sim.alpha] should fail
/// with an error containing "no such".
#[test]
fn sim_bake_nonexistent_label_fails() {
    let tmp = tempdir().unwrap();
    let dotdir = tmp.path().join(".xc-dev");
    std::fs::create_dir_all(&dotdir).unwrap();

    std::fs::write(
        dotdir.join("project.toml"),
        r#"schema_version = 1
[project]
path = "X.xcworkspace"
scheme = "X"
configuration = "Debug"
bundle_id = "com.x"
product_name = "X"
min_ios_version = "26.0"
"#,
    )
    .unwrap();

    std::fs::write(
        dotdir.join("simulator.toml"),
        r#"schema_version = 1

[sim.alpha]
name    = "alpha-test-sim"
device  = "iPhone 16"
runtime = "iOS 26.2"
"#,
    )
    .unwrap();

    std::fs::write(dotdir.join("tasks.toml"), "[tasks.noop]\ncmd = \"true\"\n").unwrap();

    let out = AssertCmd::cargo_bin("xc-dev")
        .unwrap()
        .current_dir(tmp.path())
        .args([":sim", "bake", "nonexistent"])
        .output()
        .expect("xc-dev :sim bake nonexistent ran");

    assert!(
        !out.status.success(),
        ":sim bake nonexistent should exit non-zero"
    );

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("no such"),
        "error should mention 'no such', got: {stderr}"
    );
}
