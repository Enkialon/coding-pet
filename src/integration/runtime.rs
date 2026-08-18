use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeDescriptor {
    pub version: u16,
    pub pid: u32,
    pub address: String,
    pub token: String,
}

pub fn runtime_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", "CodingPet")
        .map(|dirs| dirs.config_dir().join("integration.json"))
}
