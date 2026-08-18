// Codex Pet Provider implementation — Phase 4
// Loads Codex-format pet spritesheets (1536×1872, 8×9 grid) with optional pet.json manifest.

use std::path::{Path, PathBuf};

use crate::error::AppError;
use super::model::{Animation, PetAnimation, PetMetadata, SpriteRect};
use super::PetProvider;

// ---------------------------------------------------------------------------
// Spritesheet layout constants
// ---------------------------------------------------------------------------

const CODEX_COLUMNS: u32 = 8;
const CODEX_ROWS: u32 = 9;
const CODEX_CELL_WIDTH: u32 = 192;
const CODEX_CELL_HEIGHT: u32 = 208;
const CODEX_SHEET_WIDTH: u32 = CODEX_COLUMNS * CODEX_CELL_WIDTH;   // 1536
const CODEX_SHEET_HEIGHT: u32 = CODEX_ROWS * CODEX_CELL_HEIGHT;    // 1872

// ---------------------------------------------------------------------------
// Animation row mapping
// ---------------------------------------------------------------------------

fn codex_animation_def(anim: PetAnimation) -> (u32, u32) {
    match anim {
        PetAnimation::Idle         => (0, 6),
        PetAnimation::RunningRight => (1, 8),
        PetAnimation::RunningLeft  => (2, 8),
        PetAnimation::Waving       => (3, 4),
        PetAnimation::Jumping      => (4, 5),
        PetAnimation::Failed       => (5, 8),
        PetAnimation::Waiting      => (6, 6),
        PetAnimation::Running      => (7, 6),
        PetAnimation::Review       => (8, 6),
    }
}

fn build_animation(anim: PetAnimation) -> Animation {
    let (row, frame_count) = codex_animation_def(anim);
    let frames: Vec<SpriteRect> = (0..frame_count)
        .map(|col| compute_sprite_rect(row, col))
        .collect();

    Animation {
        frames,
        frame_duration_ms: 150,
        looping: matches!(
            anim,
            PetAnimation::Idle
                | PetAnimation::Running
                | PetAnimation::Waiting
                | PetAnimation::RunningRight
                | PetAnimation::RunningLeft
        ),
    }
}

/// Compute the sprite rectangle for a given row and column in the spritesheet atlas.
fn compute_sprite_rect(row: u32, col: u32) -> SpriteRect {
    SpriteRect {
        x: col * CODEX_CELL_WIDTH,
        y: row * CODEX_CELL_HEIGHT,
        width: CODEX_CELL_WIDTH,
        height: CODEX_CELL_HEIGHT,
    }
}

/// Validate that a resource path stays within the pet directory (no traversal).
#[allow(dead_code)]
fn validate_resource_path(pet_dir: &Path, resource_path: &Path) -> Result<PathBuf, AppError> {
    // Reject any path containing parent-directory components
    for component in resource_path.components() {
        if matches!(component, std::path::Component::ParentDir) {
            return Err(AppError::Pet(
                "Resource path contains '..' traversal component".into(),
            ));
        }
    }
    // Reject absolute paths that are not within pet_dir
    if resource_path.is_absolute() {
        return Err(AppError::Pet(
            "Resource path is absolute and not allowed".into(),
        ));
    }
    let full = pet_dir.join(resource_path);
    Ok(full)
}

// ---------------------------------------------------------------------------
// Pixel conversion
// ---------------------------------------------------------------------------

/// Convert RGBA pixel data to premultiplied BGRA.
fn rgba_to_premultiplied_bgra(rgba: &[u8]) -> Vec<u8> {
    let mut bgra = Vec::with_capacity(rgba.len());
    for chunk in rgba.chunks_exact(4) {
        let (r, g, b, a) = (chunk[0], chunk[1], chunk[2], chunk[3]);
        let alpha = a as u16;
        bgra.push(((b as u16 * alpha) / 255) as u8); // B
        bgra.push(((g as u16 * alpha) / 255) as u8); // G
        bgra.push(((r as u16 * alpha) / 255) as u8); // R
        bgra.push(a);                                  // A
    }
    bgra
}

// ---------------------------------------------------------------------------
// Manifest (pet.json)
// ---------------------------------------------------------------------------

#[derive(serde::Deserialize, Default)]
#[serde(default)]
struct PetManifest {
    name: Option<String>,
    description: Option<String>,
}

// ---------------------------------------------------------------------------
// CodexPetProvider
// ---------------------------------------------------------------------------

pub struct CodexPetProvider {
    metadata: PetMetadata,
    pixels: Vec<u8>,
    stride: usize,
    animations: Vec<(PetAnimation, Animation)>,
}

impl CodexPetProvider {
    /// Load a Codex pet from the given directory.
    ///
    /// Expects a spritesheet image (PNG or WebP) with dimensions 1536×1872.
    /// Optionally reads `pet.json` for display name / description metadata.
    pub fn load(pet_dir: &Path) -> Result<Self, AppError> {
        // --- 1. Read optional manifest ---
        let manifest_path = pet_dir.join("pet.json");
        let manifest: PetManifest = if manifest_path.exists() {
            match std::fs::read_to_string(&manifest_path) {
                Ok(content) => match serde_json::from_str(&content) {
                    Ok(m) => m,
                    Err(e) => {
                        tracing::warn!("Failed to parse pet.json: {e}");
                        PetManifest::default()
                    }
                },
                Err(e) => {
                    tracing::warn!("Failed to read pet.json: {e}");
                    PetManifest::default()
                }
            }
        } else {
            PetManifest::default()
        };

        // Derive display name: manifest > directory name > "Codex Pet"
        let dir_name = pet_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Codex Pet")
            .to_string();
        let display_name = manifest
            .name
            .filter(|n| !n.is_empty())
            .unwrap_or(dir_name);

        let metadata = PetMetadata {
            id: display_name.to_lowercase().replace(' ', "_"),
            display_name,
            description: manifest.description,
        };

        // --- 2. Find spritesheet file ---
        let spritesheet_path = resolve_spritesheet(pet_dir)?;

        // --- 3. Path security ---
        let canonical_dir = pet_dir
            .canonicalize()
            .map_err(|e| AppError::Pet(format!("Cannot canonicalize pet dir: {e}")))?;
        let canonical_sheet = spritesheet_path
            .canonicalize()
            .map_err(|e| AppError::Pet(format!("Cannot canonicalize spritesheet: {e}")))?;

        if !canonical_sheet.starts_with(&canonical_dir) {
            return Err(AppError::Pet(
                "Spritesheet path escapes pet directory (traversal rejected)".into(),
            ));
        }

        // --- 4. Load image ---
        tracing::info!("Loading spritesheet: {}", canonical_sheet.display());
        let img = image::open(&canonical_sheet)?.into_rgba8();

        // --- 5. Validate dimensions ---
        if img.width() != CODEX_SHEET_WIDTH || img.height() != CODEX_SHEET_HEIGHT {
            return Err(AppError::Pet(format!(
                "Spritesheet dimensions {}×{} do not match expected {}×{}",
                img.width(),
                img.height(),
                CODEX_SHEET_WIDTH,
                CODEX_SHEET_HEIGHT,
            )));
        }

        // --- 6. Convert to premultiplied BGRA ---
        let pixels = rgba_to_premultiplied_bgra(img.as_raw());
        let stride = CODEX_SHEET_WIDTH as usize * 4;

        // --- 7. Pre-compute animations ---
        let all_variants = [
            PetAnimation::Idle,
            PetAnimation::RunningRight,
            PetAnimation::RunningLeft,
            PetAnimation::Waving,
            PetAnimation::Jumping,
            PetAnimation::Failed,
            PetAnimation::Waiting,
            PetAnimation::Running,
            PetAnimation::Review,
        ];
        let animations: Vec<(PetAnimation, Animation)> = all_variants
            .iter()
            .map(|&anim| (anim, build_animation(anim)))
            .collect();

        tracing::info!(
            "Loaded Codex pet '{}' ({} animations)",
            metadata.display_name,
            animations.len()
        );

        Ok(CodexPetProvider {
            metadata,
            pixels,
            stride,
            animations,
        })
    }
}

impl PetProvider for CodexPetProvider {
    fn metadata(&self) -> &PetMetadata {
        &self.metadata
    }

    fn animation(&self, anim: PetAnimation) -> Option<&Animation> {
        self.animations
            .iter()
            .find(|(variant, _)| *variant == anim)
            .map(|(_, animation)| animation)
    }

    fn spritesheet_pixels(&self) -> &[u8] {
        &self.pixels
    }

    fn spritesheet_stride(&self) -> usize {
        self.stride
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Resolve the spritesheet file path within a pet directory.
///
/// Priority:
/// 1. `spritesheet.webp`
/// 2. `spritesheet.png`
/// 3. Any `.webp` or `.png` file in the directory
fn resolve_spritesheet(pet_dir: &Path) -> Result<PathBuf, AppError> {
    let webp = pet_dir.join("spritesheet.webp");
    if webp.exists() {
        return Ok(webp);
    }

    let png = pet_dir.join("spritesheet.png");
    if png.exists() {
        return Ok(png);
    }

    // Fallback: find any .webp or .png in the directory
    if let Ok(entries) = std::fs::read_dir(pet_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if ext.eq_ignore_ascii_case("webp") || ext.eq_ignore_ascii_case("png") {
                        return Ok(path);
                    }
                }
            }
        }
    }

    Err(AppError::Pet(format!(
        "No spritesheet (.webp or .png) found in {}",
        pet_dir.display()
    )))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Atlas geometry tests --

    #[test]
    fn test_sprite_rect_row0_col0() {
        let rect = compute_sprite_rect(0, 0);
        assert_eq!(rect.x, 0);
        assert_eq!(rect.y, 0);
        assert_eq!(rect.width, 192);
        assert_eq!(rect.height, 208);
    }

    #[test]
    fn test_sprite_rect_row1_col2() {
        let rect = compute_sprite_rect(1, 2);
        assert_eq!(rect.x, 384);
        assert_eq!(rect.y, 208);
        assert_eq!(rect.width, 192);
        assert_eq!(rect.height, 208);
    }

    #[test]
    fn test_sprite_rect_last_cell() {
        // Row 8, Col 7 (last cell in 9×8 grid)
        let rect = compute_sprite_rect(8, 7);
        assert_eq!(rect.x, 1344);
        assert_eq!(rect.y, 1664);
        assert_eq!(rect.width, 192);
        assert_eq!(rect.height, 208);
    }

    // -- Manifest parsing tests --

    #[test]
    fn test_parse_valid_manifest() {
        let json = r#"{"name": "Test Cat", "description": "A test pet"}"#;
        let manifest: PetManifest = serde_json::from_str(json).unwrap();
        assert_eq!(manifest.name, Some("Test Cat".to_string()));
        assert_eq!(manifest.description, Some("A test pet".to_string()));
    }

    #[test]
    fn test_parse_manifest_missing_fields() {
        let json = r#"{}"#;
        let manifest: PetManifest = serde_json::from_str(json).unwrap();
        assert_eq!(manifest.name, None);
        assert_eq!(manifest.description, None);
    }

    #[test]
    fn test_parse_manifest_unknown_fields() {
        let json = r#"{"name": "Cat", "unknown_field": 42, "extra": true}"#;
        let manifest: PetManifest = serde_json::from_str(json).unwrap();
        assert_eq!(manifest.name, Some("Cat".to_string()));
    }

    #[test]
    fn test_parse_invalid_json() {
        let json = "not valid json {{{";
        let result: Result<PetManifest, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    // -- Path validation tests --

    #[test]
    fn test_path_normal_relative() {
        let pet_dir = Path::new("pets").join("cat");
        let resource = Path::new("spritesheet.png");
        let result = validate_resource_path(&pet_dir, resource);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), pet_dir.join("spritesheet.png"));
    }

    #[test]
    fn test_path_traversal_rejected() {
        let pet_dir = Path::new("pets").join("cat");
        let resource = Path::new("..\\..\\..\\windows\\system32\\foo.png");
        let result = validate_resource_path(&pet_dir, resource);
        assert!(result.is_err());
    }

    #[test]
    fn test_path_absolute_external_rejected() {
        let pet_dir = Path::new("pets").join("cat");
        let resource = Path::new("C:\\Windows\\System32\\foo.png");
        let result = validate_resource_path(&pet_dir, resource);
        assert!(result.is_err());
    }

    // -- build_animation sanity --

    #[test]
    fn test_build_idle_animation() {
        let anim = build_animation(PetAnimation::Idle);
        assert_eq!(anim.frames.len(), 6);
        assert!(anim.looping);
        assert_eq!(anim.frame_duration_ms, 150);
        // First frame should be row 0, col 0
        assert_eq!(anim.frames[0].x, 0);
        assert_eq!(anim.frames[0].y, 0);
    }

    #[test]
    fn test_build_waving_animation() {
        let anim = build_animation(PetAnimation::Waving);
        assert_eq!(anim.frames.len(), 4);
        assert!(!anim.looping);
        // Row 3
        assert_eq!(anim.frames[0].y, 3 * 208);
    }
}
