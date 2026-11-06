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
        } else if a == "--layout" {
            args.next();
        } else if !a.starts_with("--") {
            files.push(PathBuf::from(a));
        }
    }
    let shot_mode = screenshot.is_some();
    if !shot_mode {
        drop_bad_window_state();
    }
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title(app::APP_NAME)
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([520.0, 320.0])
            .with_icon(std::sync::Arc::new(app_icon())),
        persist_window: !shot_mode,
        // screenshot runs never read or write the person's saved window / settings
        persistence_path: if shot_mode { Some(std::env::temp_dir().join("bojan_text_screenshot_state.ron")) } else { None },
        ..Default::default()
    };
    eframe::run_native(
        app::APP_NAME,
        options,
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, screenshot, files)))),
    )
}

/// The window position and size are saved when the program closes. If it was closed
/// while minimised, Windows reports position -32000 and size 0, and the next start
/// would open a tiny window off-screen. Drop such a saved window before starting.
fn drop_bad_window_state() {
    let Some(dir) = eframe::storage_dir(app::APP_NAME) else { return };
    let file = dir.join("app.ron");
    let Ok(text) = std::fs::read_to_string(&file) else { return };
    let number_after = |line: &str, key: &str| -> Option<f32> {
        let i = line.find(key)? + key.len();
        let rest = &line[i..];
        let end = rest.find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-')).unwrap_or(rest.len());
        rest[..end].parse().ok()
    };
    let mut changed = false;
    let kept: Vec<&str> = text
        .lines()
        .filter(|l| {
            if !l.trim_start().starts_with("\"window\":") {
                return true;
            }
            let w = number_after(l, "inner_size_points:(x:").unwrap_or(0.0);
            let h = number_after(l, "inner_size_points:(x:").and_then(|_| l.split("inner_size_points:(x:").nth(1)).and_then(|r| number_after(r, "y:")).unwrap_or(0.0);
            let px = number_after(l, "outer_position_pixels:Some((x:").unwrap_or(0.0);
            let py = l.split("outer_position_pixels:Some((x:").nth(1).and_then(|r| number_after(r, "y:")).unwrap_or(0.0);
            let ok = w >= 480.0 && h >= 300.0 && px > -10000.0 && py > -10000.0 && px < 20000.0 && py < 20000.0;
            changed |= !ok;
            ok
        })
        .collect();
    if changed {
        let _ = std::fs::write(&file, kept.join("\n") + "\n");
    }
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
