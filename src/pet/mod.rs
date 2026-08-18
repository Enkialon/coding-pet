pub mod model;
pub mod codex;

use std::path::PathBuf;
use model::{Animation, PetAnimation, PetMetadata};

pub trait PetProvider {
    fn metadata(&self) -> &PetMetadata;
    fn animation(&self, anim: PetAnimation) -> Option<&Animation>;
    /// Returns the full spritesheet pixel data in premultiplied BGRA format
    fn spritesheet_pixels(&self) -> &[u8];
    /// Returns the stride (bytes per row) of the spritesheet
    fn spritesheet_stride(&self) -> usize;
}

/// Scan `%USERPROFILE%\.codex\pets\` for directories containing spritesheet files.
pub fn discover_codex_pets() -> Vec<PathBuf> {
    let home = std::env::var("USERPROFILE")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("C:\\Users\\Default"));
    let pets_dir = home.join(".codex").join("pets");

    if !pets_dir.is_dir() {
        return Vec::new();
    }

    let mut pets = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&pets_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.join("spritesheet.webp").exists()
                    || path.join("spritesheet.png").exists()
                {
                    pets.push(path);
                }
            }
        }
    }
    pets.sort();
    pets
}
