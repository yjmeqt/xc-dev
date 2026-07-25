//! Locating and classifying Xcode DerivedData directories for `:clean`.

mod match_;
mod scan;

pub use match_::{belongs_to_project, classify_dir_name, Classify};
pub use scan::{read_workspace_path, scan_system_derived_data, Candidate};
