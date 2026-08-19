// Native Windows GDI renderer.
// All unsafe Win32 GDI code is isolated here.
// Renders a premultiplied-BGRA pixel buffer to a WS_EX_LAYERED window
// via a DIB section and UpdateLayeredWindow.

use super::PetWindow;
use crate::pet::model::SpriteRect;
use windows_sys::Win32::Foundation::{GetLastError, POINT, RECT, SIZE};
use windows_sys::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, SelectObject, AC_SRC_ALPHA,
    AC_SRC_OVER, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION, DIB_RGB_COLORS, HBITMAP, HDC,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{GetWindowRect, UpdateLayeredWindow, ULW_ALPHA};

pub struct Renderer {
    hwnd: isize,
    width: u32,
    height: u32,
    mem_dc: HDC,
    dib_bitmap: HBITMAP,
    old_bitmap: HBITMAP,
    pixels: *mut u8,
}

impl Renderer {
    /// Create a new renderer backed by a DIB section for the given HWND.
    pub fn new(
        window: &PetWindow,
        width: u32,
        height: u32,
    ) -> Result<Self, crate::error::AppError> {
        let hwnd = window.hwnd;
        unsafe {
            // Create a memory device context (screen DC = 0 / null)
            let mem_dc = CreateCompatibleDC(std::ptr::null_mut());
            if mem_dc.is_null() {
                return Err(crate::error::AppError::Window(
                    "CreateCompatibleDC failed".into(),
                ));
            }

            // Describe a 32-bit top-down DIB
            let bih = BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width as i32,
                biHeight: -(height as i32), // negative ⇒ top-down
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                biSizeImage: 0,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            };

            let mut pixel_ptr: *mut std::ffi::c_void = std::ptr::null_mut();
            let dib_bitmap = CreateDIBSection(
                mem_dc,
                &bih as *const BITMAPINFOHEADER as *const BITMAPINFO,
                DIB_RGB_COLORS,
                &mut pixel_ptr,
                std::ptr::null_mut(),
                0,
            );

            if dib_bitmap.is_null() || pixel_ptr.is_null() {
                DeleteDC(mem_dc);
                return Err(crate::error::AppError::Window(
                    "CreateDIBSection failed".into(),
                ));
            }

            // Select the DIB into the memory DC, saving the old bitmap
            let old_bitmap = SelectObject(mem_dc, dib_bitmap);

            Ok(Renderer {
                hwnd,
                width,
                height,
                mem_dc,
                dib_bitmap,
                old_bitmap,
                pixels: pixel_ptr as *mut u8,
            })
        }
    }

    /// Blit a sprite region from a premultiplied-BGRA spritesheet into the DIB
    /// and push the result to the layered window via UpdateLayeredWindow.
    pub fn draw_frame(&mut self, spritesheet: &[u8], stride: usize, rect: SpriteRect) {
        let w = self.width as usize;
        let h = self.height as usize;
        let dib_stride = w * 4;
        let dib_size = dib_stride * h;

        unsafe {
            // 1. Clear the entire DIB buffer to zero (transparent)
            std::ptr::write_bytes(self.pixels, 0u8, dib_size);

            // 2. Copy sprite region row-by-row into the DIB
            let src_w = rect.width as usize;
            let src_h = rect.height as usize;
            if src_w == w && src_h == h {
                let copy_w = src_w.min(w);
                let copy_h = src_h.min(h);
                for row in 0..copy_h {
                    let src_offset = ((rect.y as usize) + row) * stride + (rect.x as usize) * 4;
                    let dst_offset = row * dib_stride;
                    let bytes = copy_w * 4;
                    if src_offset + bytes <= spritesheet.len() {
                        std::ptr::copy_nonoverlapping(
                            spritesheet.as_ptr().add(src_offset),
                            self.pixels.add(dst_offset),
                            bytes,
                        );
                    }
                }
            } else {
                for dy in 0..h {
                    let sy = dy * src_h / h.max(1);
                    let src_row = ((rect.y as usize) + sy) * stride + (rect.x as usize) * 4;
                    let dst_row = dy * dib_stride;
                    for dx in 0..w {
                        let sx = dx * src_w / w.max(1);
                        let src = src_row + sx * 4;
                        let dst = dst_row + dx * 4;
                        if src + 4 <= spritesheet.len() {
                            std::ptr::copy_nonoverlapping(
                                spritesheet.as_ptr().add(src),
                                self.pixels.add(dst),
                                4,
                            );
                        }
                    }
                }
            }

            // 3. Get current window position on screen
            let mut win_rect = RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            };
            GetWindowRect(self.hwnd as _, &mut win_rect);

            // 4. Call UpdateLayeredWindow to composite the DIB onto the screen
            let dst_pt = POINT {
                x: win_rect.left,
                y: win_rect.top,
            };
            let sz = SIZE {
                cx: self.width as i32,
                cy: self.height as i32,
            };
            let src_pt = POINT { x: 0, y: 0 };

            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };

            let result = UpdateLayeredWindow(
                self.hwnd as _,       // hwnd
                std::ptr::null_mut(), // hdcDst (screen)
                &dst_pt,              // pptDst
                &sz,                  // psize
                self.mem_dc,          // hdcSrc
                &src_pt,              // pptSrc
                0,                    // crKey (no color key)
                &blend,               // pblend
                ULW_ALPHA,            // dwFlags
            );
            if result == 0 {
                let err = GetLastError();
                tracing::error!("UpdateLayeredWindow failed: GetLastError = {err}");
            }
        }
    }

    /// Recreate the DIB when settings change the on-screen pet size.
    pub fn set_output_size(&mut self, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);
        if width == self.width && height == self.height {
            return;
        }
        unsafe {
            SelectObject(self.mem_dc, self.old_bitmap);
            DeleteObject(self.dib_bitmap);
            let bih = BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width as i32,
                biHeight: -(height as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                biSizeImage: 0,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            };
            let mut pixel_ptr: *mut std::ffi::c_void = std::ptr::null_mut();
            let dib_bitmap = CreateDIBSection(
                self.mem_dc,
                &bih as *const BITMAPINFOHEADER as *const BITMAPINFO,
                DIB_RGB_COLORS,
                &mut pixel_ptr,
                std::ptr::null_mut(),
                0,
            );
            if dib_bitmap.is_null() || pixel_ptr.is_null() {
                tracing::error!("CreateDIBSection failed while resizing pet");
                return;
            }
            self.old_bitmap = SelectObject(self.mem_dc, dib_bitmap);
            self.dib_bitmap = dib_bitmap;
            self.pixels = pixel_ptr as *mut u8;
            self.width = width;
            self.height = height;
        }
    }

    /// The layered GDI surface size is owned by `set_output_size`.
    pub fn resize_surface(&mut self, _width: u32, _height: u32) {}
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            // Restore the old bitmap before deleting, per Win32 rules
            SelectObject(self.mem_dc, self.old_bitmap);
            DeleteObject(self.dib_bitmap);
            DeleteDC(self.mem_dc);
        }
    }
}
