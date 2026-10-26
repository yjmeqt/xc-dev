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

/// One cached resolution: the UDID and the rendered name it was resolved for.
///
/// `name` is `None` for caches written before names were recorded. Such an
/// entry never matches, so the label re-resolves once and records its name.
#[derive(Debug, Clone, PartialEq, Eq)]
struct CachedSim {
    id: String,
    name: Option<String>,
}

/// In-memory + on-disk cache of `label -> UDID` entries.
///
/// Cache file: `<xc_dev_dir>/simulator.local.toml`.
/// Format:
/// ```toml
/// [sim.default]
/// id = "ABC-DEF-..."
/// name = "MyApp-feat"
/// ```
///
/// A hit requires the cached `name` to equal the currently rendered name, so a
/// `simulator.toml` whose name template changes moves to the new simulator
/// instead of staying on the one it resolved first.
pub struct SimResolver {
    cache_path: PathBuf,
    /// In-memory map of `"<label>" → cached resolution`.
    cache: HashMap<String, CachedSim>,
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

        if let Some(cached) = self.cache.get(label) {
            if cached.name.as_deref() == Some(rendered_name) {
                return Ok(cached.id.clone());
            }
        }

        // Not in cache (or cached under another name) — look it up or create it.
        let udid = self.find_or_create(rendered_name, device, runtime)?;
        self.cache.insert(
            label.to_string(),
            CachedSim {
                id: udid.clone(),
                name: Some(rendered_name.to_string()),
            },
        );
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
            if live_udids.contains(&v.id) {
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

/// Parse the on-disk TOML cache into a `HashMap<label, CachedSim>`.
/// Tolerates missing file or malformed TOML (returns empty map).
fn parse_cache_toml(raw: &str) -> HashMap<String, CachedSim> {
    // Expected structure: { sim: { <label>: { id: "<udid>", name: "<name>" } } }
    let value: toml::Value = match toml::from_str(raw) {
        Ok(v) => v,
        Err(_) => return HashMap::new(),
    };
    let mut map = HashMap::new();
    if let Some(sim_table) = value.get("sim").and_then(|s| s.as_table()) {
        for (label, entry) in sim_table {
            if let Some(id) = entry.get("id").and_then(|v| v.as_str()) {
                let name = entry
                    .get("name")
                    .and_then(|v| v.as_str())
                    .map(str::to_string);
                map.insert(
                    label.clone(),
                    CachedSim {
                        id: id.to_string(),
                        name,
                    },
                );
            }
        }
    }
    map
}

/// Serialize a `HashMap<label, CachedSim>` into the TOML cache format.
fn serialize_cache_toml(cache: &HashMap<String, CachedSim>) -> String {
    let mut out = String::from(
        "# .xc-dev/simulator.local.toml\n\
         # Auto-managed by xc-dev. Per-machine UDID cache. Gitignored.\n\
         # Do not edit by hand.\n",
    );
    // Sort by label for deterministic output.
    let mut labels: Vec<&String> = cache.keys().collect();
    labels.sort();
    for label in labels {
        let cached = &cache[label];
        out.push('\n');
        out.push_str(&format!("[sim.{label}]\n"));
        out.push_str(&format!("id = \"{}\"\n", cached.id));
        if let Some(name) = &cached.name {
            out.push_str(&format!("name = {}\n", toml::Value::String(name.clone())));
        }
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
    // Resolve runtime identifier.
    let runtime_id = resolve_runtime_id(runtime)?;

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

/// Look up the runtime identifier for a human-readable name like "iOS 17.0".
fn resolve_runtime_id(runtime: &str) -> Result<String> {
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

    // Prefer available runtimes.
    for rt in runtimes.iter() {
        let is_avail = rt
            .get("isAvailable")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if !is_avail {
            continue;
        }
        if runtime_matches(rt, runtime, &normalized) {
            if let Some(id) = rt.get("identifier").and_then(|i| i.as_str()) {
                return Ok(id.to_string());
            }
        }
    }
    // Fall back to unavailable.
    for rt in runtimes.iter() {
        if runtime_matches(rt, runtime, &normalized) {
            if let Some(id) = rt.get("identifier").and_then(|i| i.as_str()) {
                return Ok(id.to_string());
            }
        }
    }

    Err(XcDevError::VarUnavailable {
        name: "sim_id".to_string(),
        reason: format!(
            "no runtime found matching {runtime:?}; check `xcrun simctl list runtimes`"
        ),
    })
}

fn runtime_matches(rt: &serde_json::Value, runtime: &str, normalized: &str) -> bool {
    let name = rt.get("name").and_then(|n| n.as_str()).unwrap_or("");
    let id = rt.get("identifier").and_then(|i| i.as_str()).unwrap_or("");
    name == runtime || normalize_runtime(name) == normalized || runtime_key_matches(id, normalized)
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

    fn entry(id: &str, name: Option<&str>) -> CachedSim {
        CachedSim {
            id: id.to_string(),
            name: name.map(str::to_string),
        }
    }

    #[test]
    fn cache_toml_round_trip() {
        let mut cache = HashMap::new();
        cache.insert(
            "default".to_string(),
            entry("ABC-DEF-1234", Some("App-feat-main")),
        );
        cache.insert(
            "ipad".to_string(),
            entry("GHI-JKL-5678", Some("App \"quoted\"")),
        );

        let serialized = serialize_cache_toml(&cache);
        // Should contain [sim.default] and [sim.ipad] sections.
        assert!(serialized.contains("[sim.default]"));
        assert!(serialized.contains("[sim.ipad]"));
        assert!(serialized.contains("id = \"ABC-DEF-1234\""));
        assert!(serialized.contains("id = \"GHI-JKL-5678\""));

        // Round-trip: parse it back.
        let parsed = parse_cache_toml(&serialized);
        assert_eq!(
            parsed.get("default"),
            Some(&entry("ABC-DEF-1234", Some("App-feat-main")))
        );
        assert_eq!(
            parsed.get("ipad"),
            Some(&entry("GHI-JKL-5678", Some("App \"quoted\"")))
        );
    }

    #[test]
    fn legacy_entry_without_name_parses_with_no_name() {
        let parsed = parse_cache_toml("[sim.default]\nid = \"OLD-UDID\"\n");
        assert_eq!(parsed.get("default"), Some(&entry("OLD-UDID", None)));
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
        assert_eq!(parsed.get("default"), Some(&entry("MY-UDID-XXXX", None)));
        assert_eq!(parsed.len(), 1);
    }

    #[test]
    fn resolver_starts_empty_with_no_cache_file() {
        let tmp = tempfile::tempdir().unwrap();
        let _resolver = SimResolver::new(tmp.path());
        // Cache file should not exist yet.
        assert!(!tmp.path().join(CACHE_FILE).exists());
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
