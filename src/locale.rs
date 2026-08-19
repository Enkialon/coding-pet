// Locale / internationalisation — supports Chinese (zh) and English (en).
// Detects the system language on Windows via GetUserDefaultUILanguage().

/// Supported languages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Locale {
    Zh,
    En,
}

impl Locale {
    /// Detect the current locale.
    /// If `config_lang` is "zh" or "en", use that directly.
    /// Otherwise ("auto" or empty), detect from the Windows system language.
    pub fn detect(config_lang: &str) -> Self {
        match config_lang {
            "zh" => return Locale::Zh,
            "en" => return Locale::En,
            _ => {}
        }
        // Auto-detect from system
        Self::system_locale()
    }

    /// Detect the operating system UI language.
    fn system_locale() -> Self {
        #[cfg(windows)]
        {
            unsafe {
                let user_lang = windows_sys::Win32::Globalization::GetUserDefaultUILanguage();
                let sys_lang = windows_sys::Win32::Globalization::GetSystemDefaultUILanguage();
                let user_primary = user_lang & 0x3FF;
                let sys_primary = sys_lang & 0x3FF;

                tracing::info!(
                    "Locale detection: user_lang=0x{:04X} (primary=0x{:02X}), \
                     sys_lang=0x{:04X} (primary=0x{:02X})",
                    user_lang,
                    user_primary,
                    sys_lang,
                    sys_primary,
                );

                // 0x04 = Chinese
                if user_primary == 0x04 || sys_primary == 0x04 {
                    return Locale::Zh;
                }
            }
        }
        #[cfg(target_os = "macos")]
        {
            if let Some(locale) = sys_locale::get_locale() {
                tracing::info!("Locale detection: {locale}");
                if locale.to_ascii_lowercase().starts_with("zh") {
                    return Locale::Zh;
                }
            }
        }
        // Default to English for non-Chinese systems
        Locale::En
    }
}

/// All translatable UI strings.
pub struct UiStrings {
    pub show_hide_pet: &'static str,
    pub select_pet: &'static str,
    pub open_pet_folder: &'static str,
    pub exit: &'static str,
    pub actions: &'static str,
    pub anim_idle: &'static str,
    pub anim_running: &'static str,
    pub anim_running_right: &'static str,
    pub anim_running_left: &'static str,
    pub anim_wave: &'static str,
    pub anim_jump: &'static str,
    pub anim_failed: &'static str,
    pub anim_waiting: &'static str,
    pub anim_review: &'static str,
    pub always_on_top: &'static str,
    pub always_on_top_check: &'static str,
    pub tray_tooltip: &'static str,
}

/// Return the set of UI strings for the given locale.
pub fn ui_strings(locale: Locale) -> UiStrings {
    match locale {
        Locale::Zh => UiStrings {
            show_hide_pet: "显示/隐藏宠物",
            select_pet: "选择宠物",
            open_pet_folder: "打开宠物文件夹",
            exit: "退出",
            actions: "动作",
            anim_idle: "待机",
            anim_running: "工作",
            anim_running_right: "向右跑",
            anim_running_left: "向左跑",
            anim_wave: "挥手",
            anim_jump: "跳跃",
            anim_failed: "失败",
            anim_waiting: "等待",
            anim_review: "检查",
            always_on_top: "置顶",
            always_on_top_check: "置顶 ✓",
            tray_tooltip: "CodingPet - 桌面宠物",
        },
        Locale::En => UiStrings {
            show_hide_pet: "Show/Hide Pet",
            select_pet: "Select Pet",
            open_pet_folder: "Open Pet Folder",
            exit: "Exit",
            actions: "Actions",
            anim_idle: "Idle",
            anim_running: "Working",
            anim_running_right: "Run Right",
            anim_running_left: "Run Left",
            anim_wave: "Wave",
            anim_jump: "Jump",
            anim_failed: "Failed",
            anim_waiting: "Waiting",
            anim_review: "Review",
            always_on_top: "Always on Top",
            always_on_top_check: "Always on Top ✓",
            tray_tooltip: "CodingPet - Desktop Pet",
        },
    }
}
