use std::path::PathBuf;

pub use lao_core_api::{Fault, Status};

pub struct Artifact {
    pub id: &'static str,
    pub url: &'static str,
    pub revision: &'static str,
    pub file: &'static str,
    pub bytes: u64,
    pub sha256: &'static str,
    pub license: &'static str,
    pub template: &'static str,
    pub context: u32,
    pub runtime: &'static str,
    pub working_set: u64,
}

pub struct Verified {
    pub artifact: &'static Artifact,
    pub path: PathBuf,
}
