//! Lazy UDID resolution for self-resolving simulator entries.
//!
//! When `[sim.<label>]` has no `id`, but has `device` + `runtime`, the UDID is
//! looked up (or created) via `xcrun simctl`, then cached to
//! `<xc_dev_dir>/.sim-cache.json`.

use crate::error::{Result, XcDevError};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// In-memory + on-disk cache of `sim_name -> UDID` entries.
///
/// Cache file: `<xc_dev_dir>/.sim-cache.json`.
/// Format: `{ "<name>": "<udid>", ... }`.
pub struct SimResolver {
    cache_path: PathBuf,
    /// In-memory map of `"<name>" → <udid>`.
    cache: HashMap<String, String>,
    /// Whether the cache was loaded / validated from disk already.
    loaded: bool,
}

impl SimResolver {
    pub fn new(xc_dev_dir: &Path) -> Self {
        Self {
            cache_path: xc_dev_dir.join(".sim-cache.json"),
            cache: HashMap::new(),
            loaded: false,
        }
    }

    /// Return the UDID for a simulator entry.
    /// `name` is the (template-rendered) `sim.name` value — used as the cache key.
    /// `device` is the device type (e.g. "iPhone 16").
    /// `runtime` is the runtime string (e.g. "iOS 26.2").
    pub fn resolve(&mut self, name: &str, device: &str, runtime: &str) -> Result<String> {
        // Ensure cache is loaded + validated.
        self.ensure_loaded()?;

        if let Some(udid) = self.cache.get(name) {
            return Ok(udid.clone());
        }

        // Not in cache — look it up or create it.
        let udid = self.find_or_create(name, device, runtime)?;
        self.cache.insert(name.to_string(), udid.clone());
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

        let on_disk: HashMap<String, String> = serde_json::from_str(&raw).unwrap_or_default();

        // Validate each cached UDID — drop stale entries.
        let live_udids = live_udid_set()?;
        for (k, v) in on_disk {
            if live_udids.contains(&v) {
                self.cache.insert(k, v);
            }
        }

        Ok(())
    }

    /// Find an existing simulator matching `name` + `runtime`, or create one.
    fn find_or_create(&self, name: &str, device: &str, runtime: &str) -> Result<String> {
        // Try to find an existing device with this name and runtime.
        if let Some(udid) = find_existing(name, runtime)? {
            return Ok(udid);
        }

        // Create a new one.
        create_simulator(name, device, runtime)
    }

    /// Write the in-memory cache to disk.
    fn persist(&self) -> Result<()> {
        let json = serde_json::to_string_pretty(&self.cache)
            .map_err(|e| XcDevError::Other(format!("failed to serialize sim cache: {e}")))?;
        std::fs::write(&self.cache_path, json).map_err(|source| XcDevError::Io {
            path: self.cache_path.clone(),
            source,
        })?;
        Ok(())
    }
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
