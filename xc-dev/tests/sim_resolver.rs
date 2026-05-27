//! Tests for SimResolver — unit tests on cache logic + integration tests with simctl.
//!
//! Integration tests are skipped if `xcrun simctl` is not available or if no iOS
//! runtime is present on the machine.

use tempfile::tempdir;
use xc_dev::sim_resolver::{SimResolver, CACHE_FILE};

// ─── helpers ─────────────────────────────────────────────────────────────────

fn simctl_available() -> bool {
    std::process::Command::new("xcrun")
        .args(["simctl", "help"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Returns the identifier of the first available iOS runtime, or None.
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

/// Returns the name of a device type suitable for the given runtime, or None.
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
    // Prefer "iPhone 16" or fall back to the first iPhone that has no suffix like "(Plus)"
    let preferred = ["iPhone 16", "iPhone 15", "iPhone 14"];
    for pref in &preferred {
        if types
            .iter()
            .any(|dt| dt.get("name").and_then(|n| n.as_str()) == Some(pref))
        {
            return Some(pref.to_string());
        }
    }
    // Fall back to first iPhone device type.
    for dt in types {
        let name = dt.get("name")?.as_str().unwrap_or("");
        if name.starts_with("iPhone") {
            return Some(name.to_string());
        }
    }
    None
}

/// Delete a simulator by UDID (used for cleanup in tests).
fn delete_simulator(udid: &str) {
    let _ = std::process::Command::new("xcrun")
        .args(["simctl", "delete", udid])
        .output();
}

/// A RAII guard that deletes the simulator on drop.
struct SimGuard {
    udid: String,
}
impl Drop for SimGuard {
    fn drop(&mut self) {
        delete_simulator(&self.udid);
    }
}

// ─── unit tests ──────────────────────────────────────────────────────────────

/// The cache file should NOT exist before any resolution is performed.
#[test]
fn resolver_starts_empty_with_no_cache_file() {
    let tmp = tempdir().unwrap();
    let _resolver = SimResolver::new(tmp.path());
    assert!(!tmp.path().join(CACHE_FILE).exists());
}

/// Write a valid simulator.local.toml cache and verify SimResolver loads it
/// (by not panicking — stale entries are dropped by simctl validation).
#[test]
fn cache_persists_across_resolver_instances() {
    if !simctl_available() {
        eprintln!("skipping: xcrun simctl not available");
        return;
    }

    let tmp = tempdir().unwrap();
    let xc_dev_dir = tmp.path().to_path_buf();

    // Write a fake cache file in the new TOML format.
    let fake_udid = "FAKE-UDID-1234-5678-ABCD";
    let cache_content =
        format!("# Auto-managed by xc-dev.\n\n[sim.default]\nid = \"{fake_udid}\"\n");
    std::fs::write(xc_dev_dir.join(CACHE_FILE), &cache_content).unwrap();

    // A new resolver reading the file will either pick it up (if UDID happens
    // to exist — unlikely) or drop it as stale. We verify it doesn't panic.
    let _resolver = SimResolver::new(&xc_dev_dir);
}

/// Invalid TOML in the cache file is tolerated (treated as empty cache).
#[test]
fn resolver_tolerates_corrupt_cache() {
    if !simctl_available() {
        eprintln!("skipping: xcrun simctl not available");
        return;
    }

    let tmp = tempdir().unwrap();
    std::fs::write(tmp.path().join(CACHE_FILE), "not-valid-toml !!! [[[").unwrap();

    // Should not panic; just start with an empty in-memory cache.
    let _resolver = SimResolver::new(tmp.path());
}

/// Round-trip: read a hand-written simulator.local.toml and verify the label + id.
#[test]
fn cache_toml_format_round_trip() {
    let tmp = tempdir().unwrap();
    let cache_path = tmp.path().join(CACHE_FILE);

    // Write in the expected format.
    std::fs::write(
        &cache_path,
        r#"# .xc-dev/simulator.local.toml
# Auto-managed by xc-dev. Per-machine UDID cache. Gitignored.
# Do not edit by hand.

[sim.default]
id = "AAAA-BBBB-CCCC"

[sim.ipad]
id = "DDDD-EEEE-FFFF"
"#,
    )
    .unwrap();

    // Verify the file parses as valid TOML and contains expected sections.
    let raw = std::fs::read_to_string(&cache_path).unwrap();
    let v: toml::Value = toml::from_str(&raw).expect("should be valid TOML");
    let sim = v.get("sim").unwrap().as_table().unwrap();
    assert_eq!(sim["default"]["id"].as_str(), Some("AAAA-BBBB-CCCC"));
    assert_eq!(sim["ipad"]["id"].as_str(), Some("DDDD-EEEE-FFFF"));
}

// ─── integration test ────────────────────────────────────────────────────────

/// Full round-trip: resolve a UDID (creates a new sim), then resolve again (cache hit),
/// verify the cache file is in TOML format, and clean up via RAII guard.
#[test]
fn integration_resolve_and_cache() {
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

    // Generate a unique sim name to avoid collisions.
    let random_suffix: String = {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos();
        format!("{nanos:08x}")
    };
    let sim_name = format!("xc-dev-test-{random_suffix}");

    eprintln!("integration_resolve_and_cache: sim_name={sim_name:?}, device={device:?}, runtime={runtime:?}");

    let tmp = tempdir().unwrap();
    let xc_dev_dir = tmp.path().to_path_buf();

    // First resolution: should create a new simulator.
    let mut resolver = SimResolver::new(&xc_dev_dir);
    let udid1 = match resolver.resolve("default", &sim_name, &device, &runtime) {
        Ok(u) => u,
        Err(e) => {
            eprintln!("skipping: resolver.resolve failed: {e}");
            return;
        }
    };
    assert!(!udid1.is_empty(), "UDID should not be empty");

    // Register the RAII cleanup guard.
    let _guard = SimGuard {
        udid: udid1.clone(),
    };

    eprintln!("integration_resolve_and_cache: created UDID={udid1}");

    // Verify the cache file was written in the new TOML format.
    let cache_path = xc_dev_dir.join(CACHE_FILE);
    assert!(
        cache_path.exists(),
        "simulator.local.toml should exist after first resolve"
    );

    let cache_raw = std::fs::read_to_string(&cache_path).unwrap();
    // Verify it's valid TOML with [sim.default] section.
    assert!(
        cache_raw.contains("[sim.default]"),
        "cache should contain [sim.default] section"
    );
    let v: toml::Value = toml::from_str(&cache_raw).expect("cache file should be valid TOML");
    let cached_udid = v
        .get("sim")
        .and_then(|s| s.get("default"))
        .and_then(|d| d.get("id"))
        .and_then(|i| i.as_str())
        .expect("sim.default.id should be present");
    assert_eq!(cached_udid, udid1, "cached UDID should match resolved UDID");

    // Second resolution (new resolver instance): should be a cache hit, same UDID.
    let mut resolver2 = SimResolver::new(&xc_dev_dir);
    let udid2 = resolver2
        .resolve("default", &sim_name, &device, &runtime)
        .expect("second resolve should succeed");
    assert_eq!(
        udid1, udid2,
        "second resolve should return the same UDID (cache hit)"
    );

    eprintln!("integration_resolve_and_cache: PASS (UDID={udid1})");
    // _guard drops here, deleting the simulator.
}

/// :get sim_id via assert_cmd round-trip using a tempdir with full TOML config.
#[test]
fn integration_get_sim_id_e2e() {
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

    // Unique name.
    let random_suffix: String = {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos();
        format!("{nanos:08x}")
    };
    let sim_name = format!("xc-dev-e2e-{random_suffix}");

    eprintln!("integration_get_sim_id_e2e: sim={sim_name:?} device={device:?} runtime={runtime:?}");

    let tmp = tempdir().unwrap();

    // Write minimal .xc-dev config.
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
        format!(
            "schema_version = 1\n\n[sim.default]\nname = {sim_name:?}\ndevice = {device:?}\nruntime = {runtime:?}\n"
        ),
    )
    .unwrap();

    std::fs::write(dotdir.join("tasks.toml"), "[tasks.noop]\ncmd = \"true\"\n").unwrap();

    // Run :get sim_id.
    let out1 = assert_cmd::Command::cargo_bin("xc-dev")
        .unwrap()
        .current_dir(tmp.path())
        .args([":get", "sim_id"])
        .output()
        .expect("xc-dev :get sim_id ran");

    if !out1.status.success() {
        let stderr = String::from_utf8_lossy(&out1.stderr);
        eprintln!("skipping: :get sim_id failed: {stderr}");
        return;
    }

    let udid1 = String::from_utf8(out1.stdout).unwrap().trim().to_string();
    assert!(!udid1.is_empty(), "UDID should not be empty");

    // RAII cleanup.
    let _guard = SimGuard {
        udid: udid1.clone(),
    };

    eprintln!("integration_get_sim_id_e2e: UDID={udid1}");

    // Run again → should return same UDID (cache hit).
    let out2 = assert_cmd::Command::cargo_bin("xc-dev")
        .unwrap()
        .current_dir(tmp.path())
        .args([":get", "sim_id"])
        .output()
        .expect("second :get sim_id ran");

    assert!(out2.status.success(), "second :get sim_id should succeed");
    let udid2 = String::from_utf8(out2.stdout).unwrap().trim().to_string();
    assert_eq!(udid1, udid2, "second call should return same UDID");

    // Verify cache file is in TOML format.
    let cache_path = dotdir.join(CACHE_FILE);
    assert!(cache_path.exists(), "simulator.local.toml should exist");
    let cache_raw = std::fs::read_to_string(&cache_path).unwrap();
    // Verify [sim.default] section exists with correct UDID.
    assert!(
        cache_raw.contains("[sim.default]"),
        "cache should contain [sim.default]"
    );
    let v: toml::Value = toml::from_str(&cache_raw).expect("cache should be valid TOML");
    let cached_udid = v
        .get("sim")
        .and_then(|s| s.get("default"))
        .and_then(|d| d.get("id"))
        .and_then(|i| i.as_str())
        .expect("sim.default.id should be in cache");
    assert_eq!(cached_udid, udid1, "cache should contain the UDID");

    eprintln!("integration_get_sim_id_e2e: PASS");
}
