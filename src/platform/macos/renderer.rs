use pixels::wgpu::{BlendState, Color, CompositeAlphaMode};
use pixels::{Pixels, PixelsBuilder, SurfaceTexture};

use crate::pet::model::SpriteRect;

use super::PetWindow;

/// Metal-backed transparent surface used only on macOS.
///
/// Rendering remains event-driven: a frame is submitted only when the animator
/// changes frames or the window asks for a redraw.
pub struct Renderer {
    pixels: Pixels<'static>,
    width: u32,
    height: u32,
}

impl Renderer {
    pub fn new(
        window: &PetWindow,
        width: u32,
        height: u32,
    ) -> Result<Self, crate::error::AppError> {
        let surface_size = window.window.inner_size();
        let surface = SurfaceTexture::new(
            surface_size.width.max(1),
            surface_size.height.max(1),
            window.window.clone(),
        );
        let pixels = PixelsBuilder::new(width, height, surface)
            .clear_color(Color::TRANSPARENT)
            .blend_state(BlendState::REPLACE)
            .alpha_mode(CompositeAlphaMode::PreMultiplied)
            .build()
            .map_err(|error| crate::error::AppError::Window(error.to_string()))?;

        Ok(Self {
            pixels,
            width,
            height,
        })
    }

    pub fn resize_surface(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        if let Err(error) = self.pixels.resize_surface(width, height) {
            tracing::error!("Could not resize macOS render surface: {error}");
        }
    }

    pub fn draw_frame(&mut self, spritesheet: &[u8], stride: usize, rect: SpriteRect) {
        let width = self.width as usize;
        let height = self.height as usize;
        let frame = self.pixels.frame_mut();
        frame.fill(0);

        let copy_width = (rect.width as usize).min(width);
        let copy_height = (rect.height as usize).min(height);
        for row in 0..copy_height {
            let source_offset = ((rect.y as usize) + row) * stride + (rect.x as usize) * 4;
            let target_offset = row * width * 4;
            for column in 0..copy_width {
                let source = source_offset + column * 4;
                let target = target_offset + column * 4;
                if source + 4 <= spritesheet.len() {
                    // The shared asset buffer is premultiplied BGRA for GDI.
                    // Metal's texture is RGBA, so swap red and blue while
                    // preserving the premultiplied alpha representation.
                    frame[target] = spritesheet[source + 2];
                    frame[target + 1] = spritesheet[source + 1];
                    frame[target + 2] = spritesheet[source];
                    frame[target + 3] = spritesheet[source + 3];
                }
            }
        }

        if let Err(error) = self.pixels.render() {
            tracing::error!("macOS frame presentation failed: {error}");
        }
    }
}
