// System tray + context menu — Phase 6
// Provides a system tray icon with a menu for controlling the desktop pet.

use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

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
}

impl PetTray {
    /// Create a new system tray with the given pet names.
    ///
    /// `pet_names` is a slice of `(display_name, pet_id)` tuples used to populate
    /// the "Select Pet" submenu.
    pub fn new(
        pet_names: &[(String, String)],
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // -- Build tray menu --
        let menu = Menu::new();

        let menu_show_hide = MenuItem::with_id("show_hide", "显示/隐藏宠物", true, None);
        menu.append(&menu_show_hide)?;

        menu.append(&PredefinedMenuItem::separator())?;

        // "Select Pet" submenu
        let pet_submenu = Submenu::with_id("select_pet", "选择宠物", true);
        let mut pet_menu_items = Vec::new();
        for (display_name, pet_id) in pet_names {
            let item = MenuItem::with_id(
                format!("pet_{}", pet_id),
                display_name,
                true,
                None,
            );
            pet_submenu.append(&item)?;
            pet_menu_items.push((item, pet_id.clone()));
        }
        menu.append(&pet_submenu)?;

        menu.append(&PredefinedMenuItem::separator())?;

        let menu_open_folder_tray =
            MenuItem::with_id("open_folder_tray", "打开宠物文件夹", true, None);
        menu.append(&menu_open_folder_tray)?;

        let menu_exit = MenuItem::with_id("exit", "退出", true, None);
        menu.append(&menu_exit)?;

        // -- Build context menu (right-click on pet window) --
        let context_menu = Menu::new();

        let menu_idle = MenuItem::with_id("anim_idle", "待机", true, None);
        let menu_wave = MenuItem::with_id("anim_wave", "挥手", true, None);
        let menu_jump = MenuItem::with_id("anim_jump", "跳跃", true, None);
        let menu_always_on_top =
            MenuItem::with_id("always_on_top", "置顶 ✓", true, None);
        let menu_open_folder =
            MenuItem::with_id("open_folder", "打开宠物文件夹", true, None);
        let menu_exit_ctx = MenuItem::with_id("exit_ctx", "退出", true, None);

        context_menu.append(&menu_idle)?;
        context_menu.append(&menu_wave)?;
        context_menu.append(&menu_jump)?;
        context_menu.append(&PredefinedMenuItem::separator())?;

        // "Select Pet" submenu (mirrors the tray menu)
        let pet_submenu_ctx = Submenu::with_id("select_pet_ctx", "选择宠物", true);
        for (display_name, pet_id) in pet_names {
            let item = MenuItem::with_id(
                format!("pet_{}", pet_id),
                display_name,
                true,
                None,
            );
            pet_submenu_ctx.append(&item)?;
        }
        context_menu.append(&pet_submenu_ctx)?;

        context_menu.append(&PredefinedMenuItem::separator())?;
        context_menu.append(&menu_always_on_top)?;
        context_menu.append(&menu_open_folder)?;
        context_menu.append(&PredefinedMenuItem::separator())?;
        context_menu.append(&menu_exit_ctx)?;

        // -- Create tray icon --
        let icon = create_default_icon()?;
        let _tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_icon(icon)
            .with_tooltip("CodingPet - 桌面宠物")
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
        })
    }

    /// Show the context menu as a popup near the given screen coordinates.
    /// This uses `muda`'s `show_context_menu_for_hwnd` to display the menu
    /// anchored to the pet window.
    ///
    /// # Safety
    /// `hwnd` must be a valid window handle.
    pub fn show_context_menu(&self, hwnd: isize, x: i32, y: i32) {
        use tray_icon::menu::ContextMenu;
        let position = tray_icon::menu::dpi::PhysicalPosition {
            x: x as f64,
            y: y as f64,
        };
        unsafe {
            self.context_menu
                .show_context_menu_for_hwnd(hwnd, Some(position.into()));
        }
    }

    /// Update the "Always On Top" menu item text to reflect current state.
    pub fn set_always_on_top_text(&mut self, on: bool) {
        self.always_on_top = on;
        let text = if on {
            "置顶 ✓"
        } else {
            "置顶"
        };
        self.menu_always_on_top.set_text(text);
    }
}

/// Poll the global menu event channel for a pending menu event.
pub fn check_menu_event() -> Option<MenuEvent> {
    MenuEvent::receiver().try_recv().ok()
}

/// Create a simple 16×16 RGBA icon (a small colored paw-like square).
fn create_default_icon() -> Result<Icon, Box<dyn std::error::Error>> {
    const SIZE: u32 = 16;
    let mut rgba = vec![0u8; (SIZE * SIZE * 4) as usize];

    // Draw a small colored pattern: a warm orange circle on transparent background
    let cx = SIZE as f64 / 2.0;
    let cy = SIZE as f64 / 2.0;
    let radius = 6.0_f64;

    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = x as f64 - cx + 0.5;
            let dy = y as f64 - cy + 0.5;
            let dist = (dx * dx + dy * dy).sqrt();
            let idx = ((y * SIZE + x) * 4) as usize;

            if dist <= radius {
                let edge = ((radius - dist).min(1.0)).max(0.0);
                let alpha = (255.0 * edge) as u8;
                // Warm orange: R=240, G=160, B=80
                rgba[idx] = 240;
                rgba[idx + 1] = 160;
                rgba[idx + 2] = 80;
                rgba[idx + 3] = alpha;
            }
        }
    }

    Ok(Icon::from_rgba(rgba, SIZE, SIZE)?)
}
