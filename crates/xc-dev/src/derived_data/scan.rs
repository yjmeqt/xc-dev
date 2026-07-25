//! Scan a DerivedData root for entries belonging to a project.

use super::match_::{belongs_to_project, classify_dir_name, Classify};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Candidate {
    /// Delete this directory — WorkspacePath is under project_root.
    Delete {
        dir: PathBuf,
        workspace_path: String,
    },
    /// Keep — belongs to another project / worktree.
    Keep {
        dir: PathBuf,
        workspace_path: String,
    },
    /// No info.plist (or unreadable WorkspacePath).
    SkipOrphan { dir: PathBuf },
    /// Global `*.noindex` cache.
    SkipNoIndex { dir: PathBuf },
}

/// Read `WorkspacePath` from an Xcode DerivedData `info.plist`.
///
/// Handles XML plists in-process; falls back to `plutil` for binary plists.
pub fn read_workspace_path(plist: &Path) -> Option<String> {
    let bytes = std::fs::read(plist).ok()?;
    if bytes.starts_with(b"bplist") {
        return read_workspace_path_plutil(plist);
    }
    let text = String::from_utf8_lossy(&bytes);
    extract_workspace_path_xml(&text).or_else(|| read_workspace_path_plutil(plist))
}

fn extract_workspace_path_xml(text: &str) -> Option<String> {
    // <key>WorkspacePath</key>
    // <string>/path/here</string>
    let key = "<key>WorkspacePath</key>";
    let idx = text.find(key)?;
    let after = &text[idx + key.len()..];
    let start_tag = "<string>";
    let s = after.find(start_tag)?;
    let rest = &after[s + start_tag.len()..];
    let end = rest.find("</string>")?;
    let path = rest[..end].trim();
    if path.is_empty() {
        None
    } else {
        Some(path.to_string())
    }
}

fn read_workspace_path_plutil(plist: &Path) -> Option<String> {
    let out = Command::new("plutil")
        .args(["-extract", "WorkspacePath", "raw"])
        .arg(plist)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Scan `derived_data_root` (e.g. `~/Library/Developer/Xcode/DerivedData`) for
/// entries owned by `project_root`.
pub fn scan_system_derived_data(derived_data_root: &Path, project_root: &Path) -> Vec<Candidate> {
    let root_str = project_root.to_string_lossy();
    let mut out = Vec::new();

    let entries = match std::fs::read_dir(derived_data_root) {
        Ok(e) => e,
        Err(_) => return out,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };

        match classify_dir_name(name) {
            Classify::SkipNoIndex => {
                out.push(Candidate::SkipNoIndex { dir: path });
                continue;
            }
            Classify::Candidate => {}
        }

        let plist = path.join("info.plist");
        let wp = match read_workspace_path(&plist) {
            Some(w) => w,
            None => {
                out.push(Candidate::SkipOrphan { dir: path });
                continue;
            }
        };

        if belongs_to_project(&wp, &root_str) {
            out.push(Candidate::Delete {
                dir: path,
                workspace_path: wp,
            });
        } else {
            out.push(Candidate::Keep {
                dir: path,
                workspace_path: wp,
            });
        }
    }

    out.sort_by(|a, b| a.dir().cmp(b.dir()));
    out
}

impl Candidate {
    pub fn dir(&self) -> &Path {
        match self {
            Candidate::Delete { dir, .. }
            | Candidate::Keep { dir, .. }
            | Candidate::SkipOrphan { dir }
            | Candidate::SkipNoIndex { dir } => dir,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_xml_plist(path: &Path, workspace_path: &str) {
        let xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>WorkspacePath</key>
	<string>{workspace_path}</string>
</dict>
</plist>
"#
        );
        fs::write(path, xml).unwrap();
    }

    #[test]
    fn extract_xml_workspace_path() {
        let xml = r#"<key>WorkspacePath</key>
	<string>/tmp/proj/Packages/X</string>"#;
        assert_eq!(
            extract_workspace_path_xml(xml).as_deref(),
            Some("/tmp/proj/Packages/X")
        );
    }

    #[test]
    fn scan_deletes_project_and_package_keeps_sibling() {
        let tmp = tempfile::tempdir().unwrap();
        let dd = tmp.path().join("DerivedData");
        let root = tmp.path().join("proj");
        fs::create_dir_all(&dd).unwrap();
        fs::create_dir_all(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let root_s = root.to_string_lossy().to_string();

        let main = dd.join("App-aaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        fs::create_dir_all(&main).unwrap();
        write_xml_plist(&main.join("info.plist"), &format!("{root_s}/App.xcodeproj"));

        let pkg = dd.join("Lib-bbbbbbbbbbbbbbbbbbbbbbbbbbbb");
        fs::create_dir_all(&pkg).unwrap();
        write_xml_plist(&pkg.join("info.plist"), &format!("{root_s}/Packages/Lib"));

        let other = dd.join("App-cccccccccccccccccccccccccccc");
        fs::create_dir_all(&other).unwrap();
        write_xml_plist(&other.join("info.plist"), "/somewhere/else/App.xcodeproj");

        let noindex = dd.join("ModuleCache.noindex");
        fs::create_dir_all(&noindex).unwrap();

        let orphan = dd.join("Orphan-dddddddddddddddddddddddddddd");
        fs::create_dir_all(&orphan).unwrap();

        let results = scan_system_derived_data(&dd, &root);
        let deletes: Vec<_> = results
            .iter()
            .filter_map(|c| match c {
                Candidate::Delete {
                    dir,
                    workspace_path,
                } => Some((
                    dir.file_name().unwrap().to_string_lossy().to_string(),
                    workspace_path.clone(),
                )),
                _ => None,
            })
            .collect();
        assert_eq!(deletes.len(), 2);
        assert!(deletes.iter().any(|(n, _)| n.starts_with("App-aaa")));
        assert!(deletes.iter().any(|(n, _)| n.starts_with("Lib-bbb")));

        assert!(results.iter().any(|c| matches!(c, Candidate::Keep { .. })));
        assert!(results
            .iter()
            .any(|c| matches!(c, Candidate::SkipNoIndex { .. })));
        assert!(results
            .iter()
            .any(|c| matches!(c, Candidate::SkipOrphan { .. })));
    }
}
