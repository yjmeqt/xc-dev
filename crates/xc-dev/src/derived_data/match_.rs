//! Pure matching: does a DerivedData WorkspacePath belong to this project root?

/// Return true if `workspace_path` is exactly `project_root` or a path under it.
///
/// Uses a trailing-slash boundary so `/foo` does not match `/foo-bar`.
pub fn belongs_to_project(workspace_path: &str, project_root: &str) -> bool {
    let root = project_root.trim_end_matches('/');
    let wp = workspace_path.trim_end_matches('/');
    if wp == root {
        return true;
    }
    let prefix = format!("{root}/");
    wp.starts_with(&prefix)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Classify {
    /// Global Xcode caches (`ModuleCache.noindex`, …) — never delete.
    SkipNoIndex,
    /// Project entry to evaluate via WorkspacePath (caller supplies path).
    Candidate,
}

/// Classify a DerivedData child directory by name alone (before reading plist).
pub fn classify_dir_name(name: &str) -> Classify {
    if name.ends_with(".noindex") {
        Classify::SkipNoIndex
    } else {
        Classify::Candidate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_project_and_packages() {
        let root = "/Users/u/app/.worktrees/feat";
        assert!(belongs_to_project(
            &format!("{root}/Pulse.xcodeproj"),
            root
        ));
        assert!(belongs_to_project(
            &format!("{root}/Packages/PulseComment"),
            root
        ));
        assert!(belongs_to_project(root, root));
    }

    #[test]
    fn rejects_sibling_worktree_and_prefix_collision() {
        let root = "/Users/u/app/.worktrees/feat";
        assert!(!belongs_to_project(
            "/Users/u/app/.worktrees/other/Pulse.xcodeproj",
            root
        ));
        assert!(!belongs_to_project(
            "/Users/u/app/Pulse.xcodeproj",
            root
        ));
        // /foo must not match /foo-bar
        assert!(!belongs_to_project(
            "/Users/u/app/.worktrees/feat-extra/Pulse.xcodeproj",
            root
        ));
    }

    #[test]
    fn trailing_slash_on_root_is_ok() {
        let root = "/Users/u/app/.worktrees/feat/";
        assert!(belongs_to_project(
            "/Users/u/app/.worktrees/feat/Packages/X",
            root
        ));
    }

    #[test]
    fn noindex_skipped_by_name() {
        assert_eq!(
            classify_dir_name("ModuleCache.noindex"),
            Classify::SkipNoIndex
        );
        assert_eq!(classify_dir_name("Pulse-abc"), Classify::Candidate);
    }
}
