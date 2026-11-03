use crate::config::project::ProjectFile;
use chrono::Local;
use std::path::PathBuf;

pub struct AutoVars {
    pub project_root: PathBuf,
    pub xc_dev_dir: PathBuf,
    pub derived_data: PathBuf,
    pub build_products_dir: PathBuf,
    pub app_path: PathBuf,
    pub date: String,
    pub timestamp: String,
    pub user: Option<String>,
    pub home: Option<String>,
}

impl AutoVars {
    pub fn build(
        project_root: PathBuf,
        pf: &ProjectFile,
        effective_config: &str,
        effective_product_name: &str,
    ) -> Self {
        let xc_dev_dir = project_root.join(".xc-dev");
        let derived_data = pf
            .project
            .derived_data
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| xc_dev_dir.join("DerivedData"));
        let build_products_dir = derived_data
            .join("Build/Products")
            .join(format!("{}-iphonesimulator", effective_config));
        let app_path = build_products_dir.join(format!("{}.app", effective_product_name));
        let now = Local::now();
        Self {
            project_root,
            xc_dev_dir,
            derived_data,
            build_products_dir,
            app_path,
            date: now.format("%Y-%m-%d").to_string(),
            timestamp: now.timestamp().to_string(),
            user: std::env::var("USER").ok(),
            home: std::env::var("HOME").ok(),
        }
    }

    pub fn get(&self, name: &str) -> Option<String> {
        match name {
            "project_root" => Some(self.project_root.display().to_string()),
            "xc_dev_dir" => Some(self.xc_dev_dir.display().to_string()),
            "derived_data" => Some(self.derived_data.display().to_string()),
            "build_products_dir" => Some(self.build_products_dir.display().to_string()),
            "app_path" => Some(self.app_path.display().to_string()),
            "date" => Some(self.date.clone()),
            "timestamp" => Some(self.timestamp.clone()),
            "user" => self.user.clone(),
            "home" => self.home.clone(),
            _ => None,
        }
    }
}

pub const RESERVED: &[&str] = &[
    "project_root",
    "xc_dev_dir",
    "derived_data",
    "build_products_dir",
    "app_path",
    "date",
    "timestamp",
    "user",
    "home",
    "git_root",
    "git_branch",
    "git_sha",
    "git_worktree_dir",
    "git_worktree_name",
    "git_worktree_slug",
    "git_main_root",
    "sim_id",
    "sim_name",
    "sim_runtime",
];

pub fn is_reserved(name: &str) -> bool {
    RESERVED.contains(&name)
}
