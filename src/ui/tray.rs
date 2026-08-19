// System tray + context menu — Phase 6
// Provides a system tray icon with a menu for controlling the desktop pet.

use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

use crate::locale::{ui_strings, Locale};
use crate::platform::NativeWindowHandle;

/// Manages the system tray icon and its associated menus.
#[allow(dead_code)]
pub struct PetTray {
    /// Keep the tray icon alive for the lifetime of this struct.
    _tray: TrayIcon,
    /// "Show/Hide Pet" menu item in the tray menu.
    pub menu_show_hide: MenuItem,
    /// "Exit" menu item.
    pub menu_exit: MenuItem,
    /// Per-pet selection menu items: (menu_item, pet_id).
    pub pet_menu_items: Vec<(MenuItem, String)>,
    // Context menu items for right-click on the pet window.
    pub menu_idle: MenuItem,
    pub menu_wave: MenuItem,
    pub menu_jump: MenuItem,
    pub menu_always_on_top: MenuItem,
    pub menu_open_folder: MenuItem,
    /// The context menu (built separately so it can be shown on right-click).
    context_menu: Menu,
    /// Whether always-on-top is currently active.
    pub always_on_top: bool,
    /// Current locale (used when updating dynamic text).
    locale: Locale,
}

impl PetTray {
    /// Create a new system tray with the given pet names.
    ///
    /// `pet_names` is a slice of `(display_name, pet_id)` tuples used to populate
    /// the "Select Pet" submenu.
    pub fn new(
        pet_names: &[(String, String)],
        locale: Locale,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let s = ui_strings(locale);

        // -- Build tray menu --
        let menu = Menu::new();

        let menu_show_hide = MenuItem::with_id("show_hide", s.show_hide_pet, true, None);
        menu.append(&menu_show_hide)?;

        menu.append(&PredefinedMenuItem::separator())?;

        // "Select Pet" submenu
        let pet_submenu = Submenu::with_id("select_pet", s.select_pet, true);
        let mut pet_menu_items = Vec::new();
        for (display_name, pet_id) in pet_names {
            let item = MenuItem::with_id(format!("pet_{}", pet_id), display_name, true, None);
            pet_submenu.append(&item)?;
            pet_menu_items.push((item, pet_id.clone()));
        }
        menu.append(&pet_submenu)?;

        menu.append(&PredefinedMenuItem::separator())?;

        let menu_open_folder_tray =
            MenuItem::with_id("open_folder_tray", s.open_pet_folder, true, None);
        menu.append(&menu_open_folder_tray)?;

        let menu_exit = MenuItem::with_id("exit", s.exit, true, None);
        menu.append(&menu_exit)?;

        // -- Build context menu (right-click on pet window) --
        let context_menu = Menu::new();

        let menu_idle = MenuItem::with_id("anim_idle", s.anim_idle, true, None);
        let menu_wave = MenuItem::with_id("anim_wave", s.anim_wave, true, None);
        let menu_jump = MenuItem::with_id("anim_jump", s.anim_jump, true, None);
        let menu_always_on_top =
            MenuItem::with_id("always_on_top", s.always_on_top_check, true, None);
        let menu_open_folder = MenuItem::with_id("open_folder", s.open_pet_folder, true, None);
        let menu_exit_ctx = MenuItem::with_id("exit_ctx", s.exit, true, None);

        context_menu.append(&menu_idle)?;
        context_menu.append(&menu_wave)?;
        context_menu.append(&menu_jump)?;
        context_menu.append(&PredefinedMenuItem::separator())?;

        // "Select Pet" submenu (mirrors the tray menu)
        let pet_submenu_ctx = Submenu::with_id("select_pet_ctx", s.select_pet, true);
        for (display_name, pet_id) in pet_names {
            let item = MenuItem::with_id(format!("pet_{}", pet_id), display_name, true, None);
            pet_submenu_ctx.append(&item)?;
        }
        context_menu.append(&pet_submenu_ctx)?;

        context_menu.append(&PredefinedMenuItem::separator())?;
        context_menu.append(&menu_always_on_top)?;
        context_menu.append(&menu_open_folder)?;
        context_menu.append(&PredefinedMenuItem::separator())?;
        context_menu.append(&menu_exit_ctx)?;

        // -- Create tray icon --
        let icon = load_app_icon()?;
        let _tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_icon(icon)
            .with_tooltip(s.tray_tooltip)
            .build()?;

        Ok(PetTray {
            _tray,
            menu_show_hide,
            menu_exit,
            pet_menu_items,
            menu_idle,
            menu_wave,
            menu_jump,
            menu_always_on_top,
            menu_open_folder,
            context_menu,
            always_on_top: true,
            locale,
        })
    }

    /// Show the context menu as a popup near the given screen coordinates.
    /// This uses `muda`'s `show_context_menu_for_hwnd` to display the menu
    /// anchored to the pet window.
    ///
    /// # Safety
    /// The native handle must remain valid while the menu is shown.
    pub fn show_context_menu(&self, handle: NativeWindowHandle, x: i32, y: i32) {
        use tray_icon::menu::ContextMenu;
        let position = tray_icon::menu::dpi::PhysicalPosition {
            x: x as f64,
            y: y as f64,
        };
        #[cfg(target_os = "windows")]
        unsafe {
            let NativeWindowHandle::Win32(hwnd) = handle;
            self.context_menu
                .show_context_menu_for_hwnd(hwnd, Some(position.into()));
        }
        #[cfg(target_os = "macos")]
        unsafe {
            let NativeWindowHandle::AppKit(view) = handle;
            self.context_menu
                .show_context_menu_for_nsview(view, Some(position.into()));
        }
    }

    /// Update the "Always On Top" menu item text to reflect current state.
    pub fn set_always_on_top_text(&mut self, on: bool) {
        self.always_on_top = on;
        let s = ui_strings(self.locale);
        let text = if on {
            s.always_on_top_check
        } else {
            s.always_on_top
        };
        self.menu_always_on_top.set_text(text);
    }
}

/// Poll the global menu event channel for a pending menu event.
pub fn check_menu_event() -> Option<MenuEvent> {
    MenuEvent::receiver().try_recv().ok()
}

/// Load the project icon embedded in the executable and resize it for the tray.
fn load_app_icon() -> Result<Icon, Box<dyn std::error::Error>> {
    const TRAY_ICON_SIZE: u32 = 32;
    let source = image::load_from_memory(include_bytes!("../../icon.png"))?;
    let rgba = source
        .resize_exact(
            TRAY_ICON_SIZE,
            TRAY_ICON_SIZE,
            image::imageops::FilterType::Lanczos3,
        )
        .into_rgba8();

    Ok(Icon::from_rgba(
        rgba.into_raw(),
        TRAY_ICON_SIZE,
        TRAY_ICON_SIZE,
    )?)
}
