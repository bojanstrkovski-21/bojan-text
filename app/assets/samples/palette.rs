// Colors written in code are recognized too.
use egui::Color32;

pub const BACKGROUND: Color32 = Color32::from_rgb(27, 34, 48);
pub const ACCENT: Color32 = Color32::from_rgb(90, 176, 246);
pub const WARNING: u32 = 0xFFB454;
pub const DANGER: u32 = 0xFFF05D5D; // AARRGGBB

fn tint() -> [f32; 4] {
    // float tuples after rgb/rgba names
    rgba(0.65, 0.85, 0.56, 1.0)
}
