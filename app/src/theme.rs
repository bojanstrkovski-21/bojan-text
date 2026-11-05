//! UI theming: the whole window (sidebar, tabs, status bar, menus) adapts to the
//! active color scheme, with user controls for accent, roundness, density and contrast.

use eframe::egui::{self, Color32, CornerRadius, Stroke};
use serde::{Deserialize, Serialize};
use syntect::highlighting::Theme;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TabStyle {
    Rounded,
    Square,
    Underline,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ThemeOptions {
    pub accent_override: Option<[u8; 3]>,
    pub radius: f32,
    pub tab_style: TabStyle,
    pub density: f32,
    pub sidebar_contrast: f32,
    pub ui_font_size: f32,
    pub editor_font_size: f32,
    pub line_padding: f32,
    pub tinted_surfaces: bool,
}

impl Default for ThemeOptions {
    fn default() -> Self {
        Self {
            accent_override: None,
            radius: 6.0,
            tab_style: TabStyle::Rounded,
            density: 1.0,
            sidebar_contrast: 0.5,
            ui_font_size: 13.0,
            // Original default: Consolas 11pt on Windows (Preferences (Windows).sublime-settings)
            // = 11 * 96 / 72 = 14.67 px
            editor_font_size: 14.67,
            line_padding: 2.0,
            tinted_surfaces: true,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub dark: bool,
    pub bg: Color32,
    pub fg: Color32,
    pub muted: Color32,
    pub gutter_fg: Color32,
    pub sidebar_bg: Color32,
    pub tabbar_bg: Color32,
    pub tab_active: Color32,
    pub tab_inactive: Color32,
    pub tab_hover: Color32,
    pub status_bg: Color32,
    pub border: Color32,
    pub accent: Color32,
    pub on_accent: Color32,
    pub selection: Color32,
    pub line_hl: Color32,
    pub caret: Color32,
    pub find_hl: Color32,
    pub widget_bg: Color32,
    pub code_bg: Color32,
}

fn c(o: Option<syntect::highlighting::Color>, d: Color32) -> Color32 {
    o.map(crate::highlight::color_from_syn).unwrap_or(d)
}

pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 * (1.0 - t) + y as f32 * t).round() as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

fn opaque_over(fg: Color32, bg: Color32) -> Color32 {
    let a = fg.a() as f32 / 255.0;
    // Color32 is premultiplied; unmultiply then blend
    let un = |v: u8| if a > 0.0 { (v as f32 / a).min(255.0) } else { 0.0 };
    let l = |x: f32, y: u8| (x * a + y as f32 * (1.0 - a)).round() as u8;
    Color32::from_rgb(l(un(fg.r()), bg.r()), l(un(fg.g()), bg.g()), l(un(fg.b()), bg.b()))
}

pub fn luminance(c: Color32) -> f32 {
    (0.2126 * c.r() as f32 + 0.7152 * c.g() as f32 + 0.0722 * c.b() as f32) / 255.0
}

pub fn palette_from(theme: &Theme, o: &ThemeOptions) -> Palette {
    let s = &theme.settings;
    let bg = c(s.background, Color32::from_rgb(0x1b, 0x22, 0x30));
    let dark = luminance(bg) < 0.45;
    let fg = c(s.foreground, if dark { Color32::from_rgb(0xd6, 0xde, 0xeb) } else { Color32::from_rgb(0x2e, 0x34, 0x40) });
    let accent = match o.accent_override {
        Some([r, g, b]) => Color32::from_rgb(r, g, b),
        None => c(s.accent.or(s.caret), Color32::from_rgb(0x5a, 0xb0, 0xf6)),
    };
    let base = if o.tinted_surfaces { bg } else if dark { Color32::from_gray(36) } else { Color32::from_gray(240) };
    let k = o.sidebar_contrast.clamp(0.0, 1.0);
    let (sidebar_bg, status_bg) = if dark {
        (mix(base, Color32::BLACK, 0.12 + 0.28 * k), mix(base, Color32::BLACK, 0.2 + 0.3 * k))
    } else {
        (mix(base, Color32::from_rgb(20, 30, 50), 0.03 + 0.07 * k), mix(base, Color32::from_rgb(20, 30, 50), 0.05 + 0.09 * k))
    };
    let tab_inactive = mix(sidebar_bg, bg, 0.35);
    let tab_hover = mix(sidebar_bg, bg, 0.7);
    let border = if dark { mix(sidebar_bg, Color32::WHITE, 0.07) } else { mix(sidebar_bg, Color32::BLACK, 0.1) };
    let on_accent = if luminance(accent) > 0.6 { Color32::from_rgb(20, 22, 28) } else { Color32::WHITE };
    let selection = opaque_over(c(s.selection, mix(bg, accent, 0.3)), bg);
    let line_hl = opaque_over(c(s.line_highlight, mix(bg, fg, 0.05)), bg);
    let gutter_fg = opaque_over(c(s.gutter_foreground, mix(bg, fg, 0.35)), bg);
    Palette {
        dark,
        bg,
        fg,
        muted: mix(fg, bg, 0.4),
        gutter_fg,
        sidebar_bg,
        tabbar_bg: sidebar_bg,
        tab_active: bg,
        tab_inactive,
        tab_hover,
        status_bg,
        border,
        accent,
        on_accent,
        selection,
        line_hl,
        caret: c(s.caret, accent),
        find_hl: opaque_over(c(s.find_highlight, Color32::from_rgba_unmultiplied(255, 200, 80, 90)), bg),
        widget_bg: if dark { mix(sidebar_bg, Color32::WHITE, 0.06) } else { mix(sidebar_bg, Color32::WHITE, 0.6) },
        code_bg: if dark { mix(bg, Color32::BLACK, 0.25) } else { mix(bg, Color32::from_rgb(20, 30, 50), 0.05) },
    }
}

pub fn apply(ctx: &egui::Context, p: &Palette, o: &ThemeOptions) {
    let theme = if p.dark { egui::Theme::Dark } else { egui::Theme::Light };
    let mut style = (*ctx.style_of(theme)).clone();
    let r = CornerRadius::same(o.radius.round().clamp(0.0, 20.0) as u8);
    let v = &mut style.visuals;
    v.dark_mode = p.dark;
    v.override_text_color = Some(p.fg);
    v.panel_fill = p.sidebar_bg;
    v.window_fill = mix(p.sidebar_bg, p.bg, 0.5);
    v.window_stroke = Stroke::new(1.0, p.border);
    v.window_corner_radius = CornerRadius::same((o.radius * 1.6).round().clamp(0.0, 24.0) as u8);
    v.menu_corner_radius = r;
    v.extreme_bg_color = p.bg;
    v.text_edit_bg_color = Some(if p.dark { mix(p.bg, Color32::BLACK, 0.15) } else { Color32::WHITE });
    v.faint_bg_color = mix(p.sidebar_bg, p.fg, 0.03);
    v.code_bg_color = p.code_bg;
    v.hyperlink_color = p.accent;
    v.selection.bg_fill = mix(p.bg, p.accent, 0.38);
    v.selection.stroke = Stroke::new(1.0, p.fg);
    v.text_cursor.stroke = Stroke::new(2.0, p.caret);
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = p.sidebar_bg;
    w.noninteractive.weak_bg_fill = p.sidebar_bg;
    w.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
    w.noninteractive.fg_stroke = Stroke::new(1.0, p.fg);
    w.noninteractive.corner_radius = r;
    w.inactive.bg_fill = p.widget_bg;
    w.inactive.weak_bg_fill = p.widget_bg;
    w.inactive.fg_stroke = Stroke::new(1.0, p.fg);
    w.inactive.bg_stroke = Stroke::NONE;
    w.inactive.corner_radius = r;
    w.hovered.bg_fill = mix(p.widget_bg, p.accent, 0.25);
    w.hovered.weak_bg_fill = mix(p.widget_bg, p.accent, 0.18);
    w.hovered.bg_stroke = Stroke::new(1.0, mix(p.border, p.accent, 0.5));
    w.hovered.fg_stroke = Stroke::new(1.5, p.fg);
    w.hovered.corner_radius = r;
    w.active.bg_fill = p.accent;
    w.active.weak_bg_fill = p.accent;
    w.active.fg_stroke = Stroke::new(1.5, p.on_accent);
    w.active.corner_radius = r;
    w.open.bg_fill = mix(p.widget_bg, p.accent, 0.2);
    w.open.weak_bg_fill = mix(p.widget_bg, p.accent, 0.2);
    w.open.corner_radius = r;
    let d = o.density.clamp(0.6, 1.6);
    style.spacing.item_spacing = egui::vec2(8.0 * d, 5.0 * d);
    style.spacing.button_padding = egui::vec2(8.0 * d, 3.0 * d);
    style.spacing.interact_size.y = 20.0 * d;
    use egui::{FontFamily, FontId, TextStyle};
    let f = o.ui_font_size;
    style.text_styles = [
        (TextStyle::Small, FontId::new(f * 0.85, FontFamily::Proportional)),
        (TextStyle::Body, FontId::new(f, FontFamily::Proportional)),
        (TextStyle::Button, FontId::new(f, FontFamily::Proportional)),
        (TextStyle::Heading, FontId::new(f * 1.45, FontFamily::Proportional)),
        (TextStyle::Monospace, FontId::new(o.editor_font_size, FontFamily::Monospace)),
    ]
    .into();
    ctx.set_theme(theme);
    ctx.set_style_of(theme, style);
}

/// Load nicer system fonts when present (Consolas for code — the original's
/// Windows default — and Segoe UI for the interface).
pub fn install_fonts(ctx: &egui::Context) {
    install_fonts_with(ctx, None);
}

/// Same, with an optional font file chosen in Preferences ▸ Font ▸ Choose… for code.
pub fn install_fonts_with(ctx: &egui::Context, code_font: Option<&str>) {
    let mut fonts = egui::FontDefinitions::default();
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
    let fdir = std::path::Path::new(&windir).join("Fonts");
    let mut add = |key: &str, file: &str, family: egui::FontFamily| {
        if let Ok(bytes) = std::fs::read(fdir.join(file)) {
            fonts.font_data.insert(key.into(), std::sync::Arc::new(egui::FontData::from_owned(bytes)));
            fonts.families.entry(family).or_default().insert(0, key.into());
        }
    };
    add("consolas", "consola.ttf", egui::FontFamily::Monospace);
    if let Some(f) = code_font {
        // absolute path: `join` keeps it as is
        add("chosen_code", f, egui::FontFamily::Monospace);
    }
    add("segoe", "segoeui.ttf", egui::FontFamily::Proportional);
    // symbols fallback for icons used in the UI
    if let Ok(bytes) = std::fs::read(fdir.join("seguisym.ttf")) {
        fonts.font_data.insert("symbols".into(), std::sync::Arc::new(egui::FontData::from_owned(bytes)));
        for fam in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            fonts.families.entry(fam).or_default().push("symbols".into());
        }
    }
    ctx.set_fonts(fonts);
}
