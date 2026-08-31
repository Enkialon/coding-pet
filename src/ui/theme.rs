use std::sync::Arc;

use egui::{Color32, FontData, FontDefinitions, FontFamily, FontId, TextStyle};
use egui_components::theme::Theme;

pub const PAGE_BACKGROUND: Color32 = Color32::WHITE;
pub const CARD_BACKGROUND: Color32 = Color32::WHITE;
pub const ACCENT: Color32 = Color32::from_rgb(139, 92, 246);
pub const ACCENT_SOFT: Color32 = Color32::from_rgb(245, 237, 255);
pub const TEXT: Color32 = Color32::from_rgb(49, 42, 58);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(124, 113, 134);
pub const BORDER: Color32 = Color32::from_rgb(232, 232, 236);

pub fn install(ctx: &egui::Context) {
    install_cjk_fonts(ctx);
    ctx.set_theme(egui::Theme::Light);

    let mut theme = Theme::light();
    theme.colors.background = PAGE_BACKGROUND;
    theme.colors.foreground = TEXT;
    theme.colors.border = BORDER;
    theme.colors.ring = ACCENT;
    theme.colors.primary_background = ACCENT;
    theme.colors.primary_foreground = Color32::WHITE;
    theme.colors.primary_hover_background = Color32::from_rgb(124, 72, 232);
    theme.colors.primary_active_background = Color32::from_rgb(109, 54, 214);
    theme.colors.secondary_background = ACCENT_SOFT;
    theme.colors.secondary_foreground = Color32::from_rgb(91, 58, 119);
    theme.colors.secondary_hover_background = Color32::from_rgb(238, 224, 252);
    theme.colors.secondary_active_background = Color32::from_rgb(229, 208, 248);
    theme.colors.accent_background = ACCENT_SOFT;
    theme.colors.accent_foreground = Color32::from_rgb(91, 58, 119);
    theme.colors.muted_background = CARD_BACKGROUND;
    theme.colors.muted_foreground = TEXT_MUTED;
    theme.colors.popover_background = CARD_BACKGROUND;
    theme.colors.popover_foreground = TEXT;
    theme.colors.slider_bar_background = ACCENT;
    theme.colors.slider_thumb_background = Color32::WHITE;
    theme.colors.switch_background = Color32::from_rgb(214, 202, 214);
    theme.colors.selection_background = Color32::from_rgb(210, 187, 251);
    theme.colors.input_border = BORDER;
    theme.metrics.radius = 10.0;
    theme.metrics.radius_sm = 8.0;
    theme.metrics.radius_lg = 16.0;
    theme.install(ctx);

    ctx.style_mut_of(egui::Theme::Light, |style| {
        style.spacing.item_spacing = egui::vec2(10.0, 8.0);
        style.spacing.window_margin = egui::Margin::same(0);
        style.text_styles.insert(
            TextStyle::Heading,
            FontId::new(24.0, FontFamily::Proportional),
        );
        style
            .text_styles
            .insert(TextStyle::Body, FontId::new(14.0, FontFamily::Proportional));
        style.text_styles.insert(
            TextStyle::Small,
            FontId::new(12.0, FontFamily::Proportional),
        );
    });
}

fn install_cjk_fonts(ctx: &egui::Context) {
    let Some(bytes) = [
        r"C:\Windows\Fonts\msyh.ttc",
        r"C:\Windows\Fonts\simhei.ttf",
        "/System/Library/Fonts/PingFang.ttc",
        "/Library/Fonts/Arial Unicode.ttf",
    ]
    .into_iter()
    .find_map(|path| std::fs::read(path).ok()) else {
        return;
    };
    let mut fonts = FontDefinitions::default();
    fonts
        .font_data
        .insert("cjk".into(), Arc::new(FontData::from_owned(bytes)));
    if let Some(family) = fonts.families.get_mut(&FontFamily::Proportional) {
        family.insert(0, "cjk".into());
    }
    ctx.set_fonts(fonts);
}
