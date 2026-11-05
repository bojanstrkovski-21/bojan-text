#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod colors;
mod editor;
mod highlight;
mod lsp;
mod markdown;
mod textops;
mod theme;

use std::path::PathBuf;

fn main() -> eframe::Result<()> {
    let mut screenshot: Option<PathBuf> = None;
    let mut files = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if a == "--screenshot" {
            screenshot = args.next().map(PathBuf::from);
        } else if !a.starts_with("--") {
            files.push(PathBuf::from(a));
        }
    }
    let shot_mode = screenshot.is_some();
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title(app::APP_NAME)
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([520.0, 320.0])
            .with_icon(std::sync::Arc::new(app_icon())),
        persist_window: !shot_mode,
        ..Default::default()
    };
    eframe::run_native(
        app::APP_NAME,
        options,
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, screenshot, files)))),
    )
}

/// A simple generated icon: rounded accent square with a "B"-like mark.
fn app_icon() -> eframe::egui::IconData {
    let n = 64usize;
    let mut rgba = vec![0u8; n * n * 4];
    for y in 0..n {
        for x in 0..n {
            let i = (y * n + x) * 4;
            let (fx, fy) = (x as f32, y as f32);
            let r = 12.0;
            let dx = (r - fx).max(fx - (n as f32 - 1.0 - r)).max(0.0);
            let dy = (r - fy).max(fy - (n as f32 - 1.0 - r)).max(0.0);
            if dx * dx + dy * dy > r * r {
                continue;
            }
            let t = fy / n as f32;
            let (mut cr, mut cg, mut cb) = ((90.0 + 60.0 * t) as u8, (176.0 - 50.0 * t) as u8, 246u8);
            let in_b = (18..=24).contains(&x) && (14..=50).contains(&y)
                || (18..=40).contains(&x) && ((14..=19).contains(&y) || (29..=34).contains(&y) || (45..=50).contains(&y))
                || (38..=44).contains(&x) && (18..=30).contains(&y)
                || (40..=46).contains(&x) && (33..=46).contains(&y);
            if in_b {
                cr = 27;
                cg = 34;
                cb = 48;
            }
            rgba[i] = cr;
            rgba[i + 1] = cg;
            rgba[i + 2] = cb;
            rgba[i + 3] = 255;
        }
    }
    eframe::egui::IconData { rgba, width: n as u32, height: n as u32 }
}
