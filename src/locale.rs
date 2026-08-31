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
    pub settings: &'static str,
}

/// Strings used by the on-demand settings window.
pub struct SettingsStrings {
    pub window_title: &'static str,
    pub page_title: &'static str,
    pub page_description: &'static str,
    pub appearance: &'static str,
    pub pet_size: &'static str,
    pub pet_size_description: &'static str,
    pub awareness_range: &'static str,
    pub awareness_range_description: &'static str,
    pub poll_interval: &'static str,
    pub poll_interval_description: &'static str,
    pub behavior: &'static str,
    pub loop_idle: &'static str,
    pub loop_idle_description: &'static str,
    pub idle_animation: &'static str,
    pub idle_animation_description: &'static str,
    pub always_on_top: &'static str,
    pub always_on_top_description: &'static str,
    pub auto_save: &'static str,
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
            settings: "设置",
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
            settings: "Settings",
        },
    }
}

pub fn settings_strings(locale: Locale) -> SettingsStrings {
    match locale {
        Locale::Zh => SettingsStrings {
            window_title: "CodingPet 设置",
            page_title: "照顾好你的桌面伙伴",
            page_description: "按你喜欢的方式调整外观和互动，改动会立即生效。",
            appearance: "外观与感应",
            pet_size: "宠物大小",
            pet_size_description: "只改变屏幕上的显示尺寸",
            awareness_range: "感应范围",
            awareness_range_description: "鼠标靠近时，宠物会看向你",
            poll_interval: "感应刷新间隔",
            poll_interval_description: "越低越跟手，越高轮询越少",
            behavior: "日常行为",
            loop_idle: "循环待机动画",
            loop_idle_description: "空闲时持续播放待机动作",
            idle_animation: "随机闲置动作",
            idle_animation_description: "偶尔做一个小动作陪伴你",
            always_on_top: "保持在最前面",
            always_on_top_description: "让宠物不会被其他窗口挡住",
            auto_save: "所有改动都会自动保存",
        },
        Locale::En => SettingsStrings {
            window_title: "CodingPet Settings",
            page_title: "Care for your desktop buddy",
            page_description: "Tune its look and behavior. Changes take effect instantly.",
            appearance: "Appearance & awareness",
            pet_size: "Pet size",
            pet_size_description: "Changes its on-screen size only",
            awareness_range: "Awareness range",
            awareness_range_description: "The pet looks over when your pointer is nearby",
            poll_interval: "Awareness refresh",
            poll_interval_description: "Lower feels quicker; higher polls less often",
            behavior: "Everyday behavior",
            loop_idle: "Loop idle animation",
            loop_idle_description: "Keep the idle animation moving",
            idle_animation: "Random idle actions",
            idle_animation_description: "Occasionally perform a small companion action",
            always_on_top: "Keep in front",
            always_on_top_description: "Prevent other windows from covering the pet",
            auto_save: "Every change is saved automatically",
        },
    }
}
