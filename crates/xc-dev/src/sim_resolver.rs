//! Lazy UDID resolution for self-resolving simulator entries.
//!
//! When `[sim.<label>]` has no `id`, but has `device` + `runtime`, the UDID is
//! looked up (or created) via `xcrun simctl`, then cached to
//! `<xc_dev_dir>/simulator.local.toml`.

use crate::error::{Result, XcDevError};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// File name of the on-disk UDID cache.
pub const CACHE_FILE: &str = "simulator.local.toml";

/// In-memory + on-disk cache of `label -> UDID` entries.
///
/// Cache file: `<xc_dev_dir>/simulator.local.toml`.
/// Format:
/// ```toml
/// [sim.default]
/// id = "ABC-DEF-..."
/// ```
pub struct SimResolver {
    cache_path: PathBuf,
    /// In-memory map of `"<label>" → <udid>`.
    cache: HashMap<String, String>,
    /// Whether the cache was loaded / validated from disk already.
    loaded: bool,
}

impl SimResolver {
    pub fn new(xc_dev_dir: &Path) -> Self {
        Self {
            cache_path: xc_dev_dir.join(CACHE_FILE),
            cache: HashMap::new(),
            loaded: false,
        }
    }

    /// Return the UDID for a simulator entry.
    /// `label` is the `[sim.<label>]` key from `simulator.toml` (e.g. "default", "ipad").
    /// `rendered_name` is the template-rendered `sim.name` value.
    /// `device` is the device type (e.g. "iPhone 16").
    /// `runtime` is the runtime string (e.g. "iOS 26.2").
    pub fn resolve(
        &mut self,
        label: &str,
        rendered_name: &str,
        device: &str,
        runtime: &str,
    ) -> Result<String> {
        // Ensure cache is loaded + validated.
        self.ensure_loaded()?;

        if let Some(udid) = self.cache.get(label) {
            return Ok(udid.clone());
        }

        // Not in cache — look it up or create it.
        let udid = self.find_or_create(rendered_name, device, runtime)?;
        self.cache.insert(label.to_string(), udid.clone());
        self.persist()?;
        Ok(udid)
    }

    /// Load the cache from disk and validate every cached UDID against simctl.
    fn ensure_loaded(&mut self) -> Result<()> {
        if self.loaded {
            return Ok(());
        }
        self.loaded = true;

        if !self.cache_path.exists() {
            return Ok(());
        }

        let raw = std::fs::read_to_string(&self.cache_path).map_err(|source| XcDevError::Io {
            path: self.cache_path.clone(),
            source,
        })?;

        let on_disk = parse_cache_toml(&raw);

        // Validate each cached UDID — drop stale entries.
        let live_udids = live_udid_set()?;
        for (k, v) in on_disk {
            if live_udids.contains(&v) {
                self.cache.insert(k, v);
            }
        }

        Ok(())
    }

    /// Find an existing simulator matching `rendered_name` + `runtime`, or create one.
    fn find_or_create(&self, rendered_name: &str, device: &str, runtime: &str) -> Result<String> {
        // Try to find an existing device with this name and runtime.
        if let Some(udid) = find_existing(rendered_name, runtime)? {
            return Ok(udid);
        }

        // Create a new one.
        create_simulator(rendered_name, device, runtime)
    }

    /// Write the in-memory cache to disk atomically.
    fn persist(&self) -> Result<()> {
        let contents = serialize_cache_toml(&self.cache);
        // Atomic write: write to temp file then rename.
        let dir = self.cache_path.parent().unwrap_or(Path::new("."));
        let tmp_path = dir.join(format!(".sim-cache-tmp-{}.toml", std::process::id()));
        std::fs::write(&tmp_path, &contents).map_err(|source| XcDevError::Io {
            path: tmp_path.clone(),
            source,
        })?;
        std::fs::rename(&tmp_path, &self.cache_path).map_err(|source| XcDevError::Io {
            path: self.cache_path.clone(),
            source,
        })?;
        Ok(())
    }
}

// ─── TOML cache serialization ────────────────────────────────────────────────

/// Parse the on-disk TOML cache into a `HashMap<label, udid>`.
/// Tolerates missing file or malformed TOML (returns empty map).
fn parse_cache_toml(raw: &str) -> HashMap<String, String> {
    // Expected structure: { sim: { <label>: { id: "<udid>" } } }
    let value: toml::Value = match toml::from_str(raw) {
        Ok(v) => v,
        Err(_) => return HashMap::new(),
    };
    let mut map = HashMap::new();
    if let Some(sim_table) = value.get("sim").and_then(|s| s.as_table()) {
        for (label, entry) in sim_table {
            if let Some(id) = entry.get("id").and_then(|v| v.as_str()) {
                map.insert(label.clone(), id.to_string());
            }
        }
    }
    map
}

/// Serialize a `HashMap<label, udid>` into the TOML cache format.
fn serialize_cache_toml(cache: &HashMap<String, String>) -> String {
    let mut out = String::from(
        "# .xc-dev/simulator.local.toml\n\
         # Auto-managed by xc-dev. Per-machine UDID cache. Gitignored.\n\
         # Do not edit by hand.\n",
    );
    // Sort by label for deterministic output.
    let mut labels: Vec<&String> = cache.keys().collect();
    labels.sort();
    for label in labels {
        let udid = &cache[label];
        out.push('\n');
        out.push_str(&format!("[sim.{label}]\n"));
        out.push_str(&format!("id = \"{udid}\"\n"));
    }
    out
}

// ─── simctl helpers ──────────────────────────────────────────────────────────

/// Returns the set of UDIDs that simctl currently knows about.
fn live_udid_set() -> Result<std::collections::HashSet<String>> {
    let out = std::process::Command::new("xcrun")
        .args(["simctl", "list", "devices", "--json"])
        .output()
        .map_err(|e| XcDevError::Other(format!("xcrun simctl not available: {e}")))?;

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(XcDevError::Other(format!(
            "xcrun simctl list devices failed: {stderr}"
        )));
    }

    let v: serde_json::Value = serde_json::from_slice(&out.stdout)
        .map_err(|e| XcDevError::Other(format!("failed to parse simctl output: {e}")))?;

    let mut udids = std::collections::HashSet::new();
    if let Some(devices) = v.get("devices").and_then(|d| d.as_object()) {
        for runtime_devices in devices.values() {
            if let Some(arr) = runtime_devices.as_array() {
                for dev in arr {
                    if let Some(udid) = dev.get("udid").and_then(|u| u.as_str()) {
                        udids.insert(udid.to_string());
                    }
                }
            }
        }
    }
    Ok(udids)
}

/// `xcrun simctl list devices --json` output for finding by name + runtime.
fn find_existing(name: &str, runtime: &str) -> Result<Option<String>> {
    let out = std::process::Command::new("xcrun")
        .args(["simctl", "list", "devices", "--json"])
        .output()
        .map_err(|e| XcDevError::Other(format!("xcrun simctl not available: {e}")))?;

    if !out.status.success() {
        return Ok(None);
    }

    let v: serde_json::Value = match serde_json::from_slice(&out.stdout) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };

    let devices = match v.get("devices").and_then(|d| d.as_object()) {
        Some(d) => d,
        None => return Ok(None),
    };

    // Runtime key in simctl output looks like "com.apple.CoreSimulator.SimRuntime.iOS-17-0"
    // or sometimes the human-readable "iOS 17.0".
    // We match by normalizing both sides.
    let normalized_runtime = normalize_runtime(runtime);

    for (rt_key, rt_devices) in devices {
        if !runtime_key_matches(rt_key, &normalized_runtime) {
            continue;
        }
        if let Some(arr) = rt_devices.as_array() {
            for dev in arr {
                let dev_name = dev.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let udid = dev.get("udid").and_then(|u| u.as_str()).unwrap_or("");
                let available = dev
                    .get("availability")
                    .or_else(|| dev.get("availabilityError"))
                    .is_none();
                // Check "isAvailable" field (newer simctl)
                let is_available = dev
                    .get("isAvailable")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(available);

                if dev_name == name && !udid.is_empty() && is_available {
                    return Ok(Some(udid.to_string()));
                }
            }
        }
    }
    Ok(None)
}

/// Create a new simulator using `xcrun simctl create`.
fn create_simulator(name: &str, device: &str, runtime: &str) -> Result<String> {
    // Resolve device type identifier.
    let device_type_id = resolve_device_type(device)?;
    // Resolve runtime identifier (device-aware so duplicate display names like
    // two "iOS 27.2" runtimes — general vs device-specific — pick the right one).
    let runtime_id = resolve_runtime_id(runtime, Some(&device_type_id))?;

    let out = std::process::Command::new("xcrun")
        .args(["simctl", "create", name, &device_type_id, &runtime_id])
        .output()
        .map_err(|e| XcDevError::Other(format!("xcrun simctl create failed: {e}")))?;

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(XcDevError::VarUnavailable {
            name: "sim_id".to_string(),
            reason: format!(
                "xcrun simctl create {name:?} {device_type_id:?} {runtime_id:?} failed: {stderr}"
            ),
        });
    }

    let udid = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if udid.is_empty() {
        return Err(XcDevError::VarUnavailable {
            name: "sim_id".to_string(),
            reason: "xcrun simctl create returned empty UDID".to_string(),
        });
    }
    Ok(udid)
}

/// Look up the device type identifier for a human-readable name like "iPhone 16".
fn resolve_device_type(device: &str) -> Result<String> {
    let out = std::process::Command::new("xcrun")
        .args(["simctl", "list", "devicetypes", "--json"])
        .output()
        .map_err(|e| XcDevError::Other(format!("xcrun simctl not available: {e}")))?;

    if !out.status.success() {
        return Err(XcDevError::VarUnavailable {
            name: "sim_id".to_string(),
            reason: "xcrun simctl list devicetypes failed".to_string(),
        });
    }

    let v: serde_json::Value = serde_json::from_slice(&out.stdout)
        .map_err(|e| XcDevError::Other(format!("failed to parse devicetypes: {e}")))?;

    let types = v
        .get("devicetypes")
        .and_then(|t| t.as_array())
        .ok_or_else(|| XcDevError::Other("unexpected devicetypes format".to_string()))?;

    // Try exact name match first, then case-insensitive.
    for dt in types {
        let dt_name = dt.get("name").and_then(|n| n.as_str()).unwrap_or("");
        if dt_name == device {
            if let Some(id) = dt.get("identifier").and_then(|i| i.as_str()) {
                return Ok(id.to_string());
            }
        }
    }
    // Case-insensitive fallback.
    let device_lower = device.to_lowercase();
    for dt in types {
        let dt_name = dt
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or("")
            .to_lowercase();
        if dt_name == device_lower {
            if let Some(id) = dt.get("identifier").and_then(|i| i.as_str()) {
                return Ok(id.to_string());
            }
        }
    }

    Err(XcDevError::VarUnavailable {
        name: "sim_id".to_string(),
        reason: format!(
            "no device type found matching {device:?}; check `xcrun simctl list devicetypes`"
        ),
    })
}

/// Look up the runtime identifier for a human-readable name like "iOS 17.0",
/// a SimRuntime identifier, or an identifier suffix like `iOS-27-2-iPhone19-4`.
///
/// When several runtimes share a display name (e.g. a general `iOS 27.2` and a
/// device-specific `iOS 27.2` for iPhone Duo), `device_type_id` narrows to a
/// runtime that lists that device under `supportedDeviceTypes`. Among remaining
/// ties, prefer an exact identifier-suffix match to the normalized query.
fn resolve_runtime_id(runtime: &str, device_type_id: Option<&str>) -> Result<String> {
    let out = std::process::Command::new("xcrun")
        .args(["simctl", "list", "runtimes", "--json"])
        .output()
        .map_err(|e| XcDevError::Other(format!("xcrun simctl not available: {e}")))?;

    if !out.status.success() {
        return Err(XcDevError::VarUnavailable {
            name: "sim_id".to_string(),
            reason: "xcrun simctl list runtimes failed".to_string(),
        });
    }

    let v: serde_json::Value = serde_json::from_slice(&out.stdout)
        .map_err(|e| XcDevError::Other(format!("failed to parse runtimes: {e}")))?;

    let runtimes = v
        .get("runtimes")
        .and_then(|r| r.as_array())
        .ok_or_else(|| XcDevError::Other("unexpected runtimes format".to_string()))?;

    let normalized = normalize_runtime(runtime);

    let mut available: Vec<&serde_json::Value> = Vec::new();
    let mut unavailable: Vec<&serde_json::Value> = Vec::new();
    for rt in runtimes.iter() {
        if !runtime_matches(rt, runtime, &normalized) {
            continue;
        }
        let is_avail = rt
            .get("isAvailable")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if is_avail {
            available.push(rt);
        } else {
            unavailable.push(rt);
        }
    }

    if let Some(id) = pick_runtime_id(&available, runtime, &normalized, device_type_id) {
        return Ok(id);
    }
    if let Some(id) = pick_runtime_id(&unavailable, runtime, &normalized, device_type_id) {
        return Ok(id);
    }

    Err(XcDevError::VarUnavailable {
        name: "sim_id".to_string(),
        reason: format!(
            "no runtime found matching {runtime:?}; check `xcrun simctl list runtimes`"
        ),
    })
}

/// Choose one runtime from a pre-filtered match list.
fn pick_runtime_id(
    matches: &[&serde_json::Value],
    runtime: &str,
    normalized: &str,
    device_type_id: Option<&str>,
) -> Option<String> {
    if matches.is_empty() {
        return None;
    }

    // Exact full-identifier or suffix hit wins immediately.
    for rt in matches {
        let Some(id) = rt.get("identifier").and_then(|i| i.as_str()) else {
            continue;
        };
        if id == runtime || identifier_suffix(id).eq_ignore_ascii_case(runtime) {
            return Some(id.to_string());
        }
    }

    let mut candidates: Vec<&serde_json::Value> = matches.to_vec();
    if let Some(device_id) = device_type_id {
        let compatible: Vec<&serde_json::Value> = candidates
            .iter()
            .copied()
            .filter(|rt| runtime_supports_device(rt, device_id))
            .collect();
        if !compatible.is_empty() {
            candidates = compatible;
        }
    }

    if candidates.len() == 1 {
        return candidates[0]
            .get("identifier")
            .and_then(|i| i.as_str())
            .map(str::to_string);
    }

    // Among remaining ties (same display name), prefer the runtime whose
    // identifier suffix equals the normalized query exactly — e.g. query
    // "iOS 27.2" → "ios-27-2" prefers `…iOS-27-2` over `…iOS-27-2-iPhone19-4`.
    for rt in &candidates {
        let id = rt.get("identifier").and_then(|i| i.as_str()).unwrap_or("");
        if runtime_key_matches(id, normalized) {
            return Some(id.to_string());
        }
    }

    candidates[0]
        .get("identifier")
        .and_then(|i| i.as_str())
        .map(str::to_string)
}

fn runtime_supports_device(rt: &serde_json::Value, device_type_id: &str) -> bool {
    let Some(types) = rt.get("supportedDeviceTypes").and_then(|t| t.as_array()) else {
        // Older simctl payloads may omit the list — don't eliminate the candidate.
        return true;
    };
    types.iter().any(|dt| {
        dt.get("identifier")
            .and_then(|i| i.as_str())
            .is_some_and(|id| id == device_type_id)
    })
}

fn runtime_matches(rt: &serde_json::Value, runtime: &str, normalized: &str) -> bool {
    let name = rt.get("name").and_then(|n| n.as_str()).unwrap_or("");
    let id = rt.get("identifier").and_then(|i| i.as_str()).unwrap_or("");
    id == runtime
        || identifier_suffix(id).eq_ignore_ascii_case(runtime)
        || name == runtime
        || normalize_runtime(name) == normalized
        || runtime_key_matches(id, normalized)
}

fn identifier_suffix(id: &str) -> &str {
    id.split('.').next_back().unwrap_or(id)
}

/// Normalize "iOS 17.0" → "ios-17-0" for loose matching.
fn normalize_runtime(rt: &str) -> String {
    rt.to_lowercase().replace([' ', '.'], "-")
}

/// Check if a simctl runtime key (e.g. "com.apple.CoreSimulator.SimRuntime.iOS-17-0")
/// matches a normalized runtime string like "ios-17-0".
fn runtime_key_matches(key: &str, normalized: &str) -> bool {
    // The last segment of the identifier is like "iOS-17-0".
    let suffix = key.split('.').next_back().unwrap_or(key);
    suffix.to_lowercase() == normalized
        || suffix.to_lowercase().replace('-', ".") == normalized.replace('-', ".")
}

// ─── unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_toml_round_trip() {
        let mut cache = HashMap::new();
        cache.insert("default".to_string(), "ABC-DEF-1234".to_string());
        cache.insert("ipad".to_string(), "GHI-JKL-5678".to_string());

        let serialized = serialize_cache_toml(&cache);
        // Should contain [sim.default] and [sim.ipad] sections.
        assert!(serialized.contains("[sim.default]"));
        assert!(serialized.contains("[sim.ipad]"));
        assert!(serialized.contains("id = \"ABC-DEF-1234\""));
        assert!(serialized.contains("id = \"GHI-JKL-5678\""));

        // Round-trip: parse it back.
        let parsed = parse_cache_toml(&serialized);
        assert_eq!(
            parsed.get("default").map(String::as_str),
            Some("ABC-DEF-1234")
        );
        assert_eq!(parsed.get("ipad").map(String::as_str), Some("GHI-JKL-5678"));
    }

    #[test]
    fn cache_toml_missing_file_is_empty() {
        let parsed = parse_cache_toml("");
        assert!(parsed.is_empty());
    }

    #[test]
    fn cache_toml_invalid_toml_is_empty() {
        let parsed = parse_cache_toml("not valid toml !!! [[[");
        assert!(parsed.is_empty());
    }

    #[test]
    fn cache_toml_single_entry() {
        let raw = r#"
# comment
[sim.default]
id = "MY-UDID-XXXX"
"#;
        let parsed = parse_cache_toml(raw);
        assert_eq!(
            parsed.get("default").map(String::as_str),
            Some("MY-UDID-XXXX")
        );
        assert_eq!(parsed.len(), 1);
    }

    #[test]
    fn resolver_starts_empty_with_no_cache_file() {
        let tmp = tempfile::tempdir().unwrap();
        let _resolver = SimResolver::new(tmp.path());
        // Cache file should not exist yet.
        assert!(!tmp.path().join(CACHE_FILE).exists());
    }

    fn runtime_json(name: &str, id: &str, devices: &[&str]) -> serde_json::Value {
        serde_json::json!({
            "name": name,
            "identifier": id,
            "supportedDeviceTypes": devices.iter().map(|d| serde_json::json!({
                "identifier": d
            })).collect::<Vec<_>>(),
        })
    }

    #[test]
    fn pick_runtime_prefers_device_compatible_among_same_display_name() {
        let general = runtime_json(
            "iOS 27.2",
            "com.apple.CoreSimulator.SimRuntime.iOS-27-2",
            &["com.apple.CoreSimulator.SimDeviceType.iPhone-17"],
        );
        let duo = runtime_json(
            "iOS 27.2",
            "com.apple.CoreSimulator.SimRuntime.iOS-27-2-iPhone19-4",
            &["com.apple.CoreSimulator.SimDeviceType.iPhone-Duo"],
        );
        // Duo-specific runtime listed first — old code would grab it for iPhone 17.
        let matches = vec![&duo, &general];
        let id = pick_runtime_id(
            &matches,
            "iOS 27.2",
            &normalize_runtime("iOS 27.2"),
            Some("com.apple.CoreSimulator.SimDeviceType.iPhone-17"),
        );
        assert_eq!(
            id.as_deref(),
            Some("com.apple.CoreSimulator.SimRuntime.iOS-27-2")
        );

        let id = pick_runtime_id(
            &matches,
            "iOS 27.2",
            &normalize_runtime("iOS 27.2"),
            Some("com.apple.CoreSimulator.SimDeviceType.iPhone-Duo"),
        );
        assert_eq!(
            id.as_deref(),
            Some("com.apple.CoreSimulator.SimRuntime.iOS-27-2-iPhone19-4")
        );
    }

    #[test]
    fn pick_runtime_accepts_full_identifier_and_suffix() {
        let general = runtime_json(
            "iOS 27.2",
            "com.apple.CoreSimulator.SimRuntime.iOS-27-2",
            &["com.apple.CoreSimulator.SimDeviceType.iPhone-17"],
        );
        let duo = runtime_json(
            "iOS 27.2",
            "com.apple.CoreSimulator.SimRuntime.iOS-27-2-iPhone19-4",
            &["com.apple.CoreSimulator.SimDeviceType.iPhone-Duo"],
        );
        let matches = vec![&duo, &general];

        let id = pick_runtime_id(
            &matches,
            "com.apple.CoreSimulator.SimRuntime.iOS-27-2",
            &normalize_runtime("com.apple.CoreSimulator.SimRuntime.iOS-27-2"),
            None,
        );
        assert_eq!(
            id.as_deref(),
            Some("com.apple.CoreSimulator.SimRuntime.iOS-27-2")
        );

        let id = pick_runtime_id(
            &matches,
            "iOS-27-2-iPhone19-4",
            &normalize_runtime("iOS-27-2-iPhone19-4"),
            None,
        );
        assert_eq!(
            id.as_deref(),
            Some("com.apple.CoreSimulator.SimRuntime.iOS-27-2-iPhone19-4")
        );
    }

    #[test]
    fn runtime_matches_full_identifier() {
        let general = runtime_json(
            "iOS 27.2",
            "com.apple.CoreSimulator.SimRuntime.iOS-27-2",
            &[],
        );
        let q = "com.apple.CoreSimulator.SimRuntime.iOS-27-2";
        assert!(runtime_matches(&general, q, &normalize_runtime(q)));
        assert!(runtime_matches(
            &general,
            "iOS-27-2",
            &normalize_runtime("iOS-27-2")
        ));
    }

    #[test]
    fn resolver_tolerates_corrupt_cache() {
        if !simctl_available() {
            eprintln!("skipping: xcrun simctl not available");
            return;
        }

        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join(CACHE_FILE), "not-valid-toml !!! [[[").unwrap();

        // Should not panic; just start with an empty in-memory cache.
        let _resolver = SimResolver::new(tmp.path());
    }

    fn simctl_available() -> bool {
        std::process::Command::new("xcrun")
            .args(["simctl", "help"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}
