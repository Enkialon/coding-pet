pub mod codex;
pub mod model;

use model::{Animation, PetAnimation, PetMetadata};
use std::path::PathBuf;

pub trait PetProvider {
    fn metadata(&self) -> &PetMetadata;
    fn animation(&self, anim: PetAnimation) -> Option<&Animation>;
    /// Returns the full spritesheet pixel data in premultiplied BGRA format
    fn spritesheet_pixels(&self) -> &[u8];
    /// Returns the stride (bytes per row) of the spritesheet
    fn spritesheet_stride(&self) -> usize;
}

/// Return the Codex pets directory under the current user's home directory.
pub fn codex_pets_dir() -> Option<PathBuf> {
    directories::UserDirs::new().map(|dirs| dirs.home_dir().join(".codex").join("pets"))
}

/// Scan `~/.codex/pets/` for directories containing spritesheet files.
pub fn discover_codex_pets() -> Vec<PathBuf> {
    let Some(pets_dir) = codex_pets_dir() else {
        return Vec::new();
    };

    if !pets_dir.is_dir() {
        return Vec::new();
    }

    let mut pets = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&pets_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.join("spritesheet.webp").exists() || path.join("spritesheet.png").exists() {
                    pets.push(path);
                }
            }
        }
    }
    pets.sort();
    pets
}
