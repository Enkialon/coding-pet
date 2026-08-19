// Codex Pet Provider implementation — Phase 4
// Loads Codex-format pet spritesheets. V2 (8×11) is the primary contract;
// legacy V1 (8×9) remains readable so existing pets keep working.

use std::path::{Path, PathBuf};

use crate::error::AppError;
use super::model::{Animation, PetAnimation, PetMetadata, SpriteRect};
use super::PetProvider;

// ---------------------------------------------------------------------------
// Spritesheet layout constants
// ---------------------------------------------------------------------------

const CODEX_COLUMNS: u32 = 8;
const CODEX_V1_ROWS: u32 = 9;
const CODEX_V2_ROWS: u32 = 11;
const CODEX_CELL_WIDTH: u32 = 192;
const CODEX_CELL_HEIGHT: u32 = 208;
const CODEX_SHEET_WIDTH: u32 = CODEX_COLUMNS * CODEX_CELL_WIDTH;   // 1536
const CODEX_V1_SHEET_HEIGHT: u32 = CODEX_V1_ROWS * CODEX_CELL_HEIGHT; // 1872
const CODEX_V2_SHEET_HEIGHT: u32 = CODEX_V2_ROWS * CODEX_CELL_HEIGHT; // 2288

// ---------------------------------------------------------------------------
// Animation row mapping
// ---------------------------------------------------------------------------

fn codex_animation_def(anim: PetAnimation) -> Option<(u32, Vec<u64>, bool)> {
    match anim {
        PetAnimation::Idle => Some((0, vec![280, 110, 110, 140, 140, 320], true)),
        PetAnimation::RunningRight => {
            Some((1, vec![120, 120, 120, 120, 120, 120, 120, 220], true))
        }
        PetAnimation::RunningLeft => {
            Some((2, vec![120, 120, 120, 120, 120, 120, 120, 220], true))
        }
        PetAnimation::Waving => Some((3, vec![140, 140, 140, 280], false)),
        PetAnimation::Jumping => Some((4, vec![140, 140, 140, 140, 280], false)),
        PetAnimation::Failed => {
            Some((5, vec![140, 140, 140, 140, 140, 140, 140, 240], false))
        }
        PetAnimation::Waiting => Some((6, vec![150, 150, 150, 150, 150, 260], true)),
        PetAnimation::Running => Some((7, vec![120, 120, 120, 120, 120, 220], true)),
        PetAnimation::Review => Some((8, vec![150, 150, 150, 150, 150, 280], false)),
        PetAnimation::Look(_) => None,
    }
}

fn build_animation(anim: PetAnimation, sprite_version: u32) -> Option<Animation> {
    if let PetAnimation::Look(index) = anim {
        if sprite_version < 2 || index >= 16 {
            return None;
        }
        return Some(Animation {
            frames: vec![compute_sprite_rect(
                9 + u32::from(index / 8),
                u32::from(index % 8),
            )],
            frame_durations_ms: vec![1_000],
            looping: false,
        });
    }

    let (row, frame_durations_ms, looping) = codex_animation_def(anim)?;
    let frames = (0..frame_durations_ms.len() as u32)
        .map(|col| compute_sprite_rect(row, col))
        .collect();

    Some(Animation {
        frames,
        frame_durations_ms,
        looping,
    })
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

#[derive(serde::Deserialize)]
#[serde(default)]
struct PetManifest {
    id: Option<String>,
    name: Option<String>,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    description: Option<String>,
    #[serde(rename = "spriteVersionNumber", default = "default_sprite_version")]
    sprite_version_number: u32,
    #[serde(rename = "spritesheetPath")]
    spritesheet_path: Option<String>,
}

fn default_sprite_version() -> u32 {
    1
}

impl Default for PetManifest {
    fn default() -> Self {
        Self {
            id: None,
            name: None,
            display_name: None,
            description: None,
            sprite_version_number: default_sprite_version(),
            spritesheet_path: None,
        }
    }
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
    /// Expects a Codex V2 spritesheet (1536×2288) or a legacy V1 sheet
    /// (1536×1872). `spriteVersionNumber` selects the contract.
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
            .display_name
            .clone()
            .or_else(|| manifest.name.clone())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| dir_name.clone());
        let pet_id = manifest
            .id
            .clone()
            .filter(|id| !id.is_empty())
            .unwrap_or_else(|| dir_name.to_lowercase().replace(' ', "_"));
        let sprite_version = manifest.sprite_version_number;
        if !matches!(sprite_version, 1 | 2) {
            return Err(AppError::Pet(format!(
                "Unsupported spriteVersionNumber {sprite_version}; expected 1 or 2"
            )));
        }

        let metadata = PetMetadata {
            id: pet_id,
            display_name,
            description: manifest.description.clone(),
        };

        // --- 2. Find spritesheet file ---
        let spritesheet_path = resolve_spritesheet(pet_dir, manifest.spritesheet_path.as_deref())?;

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
        let expected_height = if sprite_version == 2 {
            CODEX_V2_SHEET_HEIGHT
        } else {
            CODEX_V1_SHEET_HEIGHT
        };
        if img.width() != CODEX_SHEET_WIDTH || img.height() != expected_height {
            return Err(AppError::Pet(format!(
                "Spritesheet dimensions {}×{} do not match expected {}×{}",
                img.width(),
                img.height(),
                CODEX_SHEET_WIDTH,
                expected_height,
            )));
        }

        // --- 6. Convert to premultiplied BGRA ---
        let pixels = rgba_to_premultiplied_bgra(img.as_raw());
        let stride = CODEX_SHEET_WIDTH as usize * 4;

        // --- 7. Pre-compute animations ---
        let mut all_variants = vec![
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
        if sprite_version >= 2 {
            all_variants.extend((0..16).map(PetAnimation::Look));
        }
        let animations: Vec<(PetAnimation, Animation)> = all_variants
            .iter()
            .filter_map(|&anim| build_animation(anim, sprite_version).map(|data| (anim, data)))
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
fn resolve_spritesheet(
    pet_dir: &Path,
    manifest_path: Option<&str>,
) -> Result<PathBuf, AppError> {
    if let Some(resource) = manifest_path.filter(|path| !path.is_empty()) {
        let path = validate_resource_path(pet_dir, Path::new(resource))?;
        if path.is_file() {
            return Ok(path);
        }
        return Err(AppError::Pet(format!(
            "Manifest spritesheetPath does not exist: {}",
            path.display()
        )));
    }
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

    fn unique_test_dir(name: &str) -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("coding-pet-{name}-{}-{nonce}", std::process::id()))
    }

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
        // Row 10, Col 7 (last cell in the V2 11×8 grid)
        let rect = compute_sprite_rect(10, 7);
        assert_eq!(rect.x, 1344);
        assert_eq!(rect.y, 2080);
        assert_eq!(rect.width, 192);
        assert_eq!(rect.height, 208);
    }

    // -- Manifest parsing tests --

    #[test]
    fn test_parse_valid_manifest() {
        let json = r#"{
            "id": "test-cat",
            "displayName": "Test Cat",
            "description": "A test pet",
            "spriteVersionNumber": 2,
            "spritesheetPath": "spritesheet.webp"
        }"#;
        let manifest: PetManifest = serde_json::from_str(json).unwrap();
        assert_eq!(manifest.id, Some("test-cat".to_string()));
        assert_eq!(manifest.display_name, Some("Test Cat".to_string()));
        assert_eq!(manifest.description, Some("A test pet".to_string()));
        assert_eq!(manifest.sprite_version_number, 2);
        assert_eq!(manifest.spritesheet_path, Some("spritesheet.webp".to_string()));
    }

    #[test]
    fn test_parse_manifest_missing_fields() {
        let json = r#"{}"#;
        let manifest: PetManifest = serde_json::from_str(json).unwrap();
        assert_eq!(manifest.name, None);
        assert_eq!(manifest.description, None);
        assert_eq!(manifest.sprite_version_number, 1);
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
        let anim = build_animation(PetAnimation::Idle, 2).unwrap();
        assert_eq!(anim.frames.len(), 6);
        assert!(anim.looping);
        assert_eq!(anim.frame_durations_ms, vec![280, 110, 110, 140, 140, 320]);
        // First frame should be row 0, col 0
        assert_eq!(anim.frames[0].x, 0);
        assert_eq!(anim.frames[0].y, 0);
    }

    #[test]
    fn test_build_waving_animation() {
        let anim = build_animation(PetAnimation::Waving, 2).unwrap();
        assert_eq!(anim.frames.len(), 4);
        assert!(!anim.looping);
        // Row 3
        assert_eq!(anim.frames[0].y, 3 * 208);
    }

    #[test]
    fn test_build_v2_look_directions() {
        let up = build_animation(PetAnimation::Look(0), 2).unwrap();
        assert_eq!(up.frames[0].x, 0);
        assert_eq!(up.frames[0].y, 9 * 208);

        let down = build_animation(PetAnimation::Look(8), 2).unwrap();
        assert_eq!(down.frames[0].x, 0);
        assert_eq!(down.frames[0].y, 10 * 208);

        let up_left = build_animation(PetAnimation::Look(15), 2).unwrap();
        assert_eq!(up_left.frames[0].x, 7 * 192);
        assert_eq!(up_left.frames[0].y, 10 * 208);
    }

    #[test]
    fn test_v1_has_no_look_directions() {
        assert!(build_animation(PetAnimation::Look(0), 1).is_none());
    }

    #[test]
    fn test_loads_complete_v2_pet_package() {
        let pet_dir = unique_test_dir("v2-load");
        std::fs::create_dir_all(&pet_dir).unwrap();
        let sheet_path = pet_dir.join("spritesheet.png");
        image::RgbaImage::new(CODEX_SHEET_WIDTH, CODEX_V2_SHEET_HEIGHT)
            .save(&sheet_path)
            .unwrap();
        let manifest_path = pet_dir.join("pet.json");
        std::fs::write(
            &manifest_path,
            r#"{
                "id": "test-v2",
                "displayName": "Test V2",
                "spriteVersionNumber": 2,
                "spritesheetPath": "spritesheet.png"
            }"#,
        )
        .unwrap();

        let provider = CodexPetProvider::load(&pet_dir).unwrap();
        assert_eq!(provider.metadata().id, "test-v2");
        assert_eq!(provider.metadata().display_name, "Test V2");
        assert!(provider.animation(PetAnimation::Look(0)).is_some());
        assert!(provider.animation(PetAnimation::Look(15)).is_some());

        std::fs::remove_file(manifest_path).unwrap();
        std::fs::remove_file(sheet_path).unwrap();
        std::fs::remove_dir(pet_dir).unwrap();
    }
}
