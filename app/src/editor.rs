//! The text view: syntax-colored TextEdit with gutter, current line, indent
//! guides, color-code decorations, find highlights, diagnostics and minimap.

use crate::colors::{find_colors, ColorMatch};
use crate::highlight::HlCache;
use crate::lsp::Diag;
use crate::theme::{mix, Palette};
use eframe::egui::{self, text::LayoutJob, Color32, FontId, Pos2, Rect, Sense, Shape, Stroke, TextFormat};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use syntect::highlighting::{FontStyle, Theme};
use syntect::parsing::{SyntaxReference, SyntaxSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColorStyle {
    Fill,
    Underline,
    Outline,
}

#[derive(Default)]
pub struct ColorCache {
    pub hash: u64,
    pub named: bool,
    pub list: Vec<ColorMatch>,
}

impl ColorCache {
    pub fn update(&mut self, text: &str, hash: u64, named: bool) {
        if self.hash != hash || self.named != named || (hash == 0 && !text.is_empty()) {
            self.list = find_colors(text, named);
            self.hash = hash;
            self.named = named;
        }
    }
    pub fn at(&self, byte: usize) -> Option<&ColorMatch> {
        let i = self.list.partition_point(|c| c.start <= byte);
        if i == 0 {
            return None;
        }
        let c = &self.list[i - 1];
        (byte < c.end).then_some(c)
    }
}

pub fn hash_text(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

pub struct ViewCfg<'a> {
    pub ss: &'a SyntaxSet,
    pub syntax: &'a SyntaxReference,
    pub theme: &'a Theme,
    pub hl_key: String,
    pub pal: &'a Palette,
    pub font: FontId,
    pub line_height: f32,
    pub wrap: bool,
    pub colorizer: bool,
    pub color_style: ColorStyle,
    pub named_colors: bool,
    pub gutter_swatches: bool,
    /// (query, match case, regex)
    pub find: Option<(&'a str, bool, bool)>,
    pub highlight_line: bool,
    pub line_numbers: bool,
    pub minimap: bool,
    pub tab_size: usize,
    pub diags: Option<&'a Vec<Diag>>,
    /// vertical guide columns (View ▸ Ruler)
    pub rulers: &'a [usize],
    /// wrap at this column when word wrap is on (0 = window width)
    pub wrap_column: usize,
    /// bookmarked line numbers (0-based)
    pub bookmarks: &'a [usize],
    /// lines changed since the last save: [start, end) (start == end marks deleted lines)
    pub modified: Option<(usize, usize)>,
}

#[derive(Default)]
pub struct ViewState {
    pub hl: HlCache,
    pub colors: ColorCache,
    pub cursor: Option<(usize, usize)>, // char range (primary, secondary)
    pub select: Option<(usize, usize)>, // request: select char range and scroll to it
    pub scroll_y: f32,
    pub content_h: f32,
    pub view_h: f32,
    pub set_scroll: Option<f32>,
    pub focus: bool,
    /// offset of the pointer inside the minimap viewport box while dragging it
    pub mm_grab: Option<f32>,
}

#[derive(Default)]
pub struct ViewOutput {
    pub changed: bool,
    pub color_edit: Option<ColorMatch>,
    pub hover_char: Option<usize>,
    pub galley_rows: usize,
}

fn build_job(text: &str, cfg: &ViewCfg, hl: &HlCache, colors: &[ColorMatch], finds: &[(usize, usize)], wrap_width: f32) -> LayoutJob {
    let mut job = LayoutJob { text: text.to_string(), ..Default::default() };
    job.wrap.max_width = if cfg.wrap { wrap_width } else { f32::INFINITY };
    let base = TextFormat {
        font_id: cfg.font.clone(),
        color: cfg.pal.fg,
        line_height: Some(cfg.line_height),
        ..Default::default()
    };
    // boundaries of overlays
    let mut cuts: Vec<usize> = Vec::new();
    if cfg.colorizer {
        for c in colors {
            cuts.push(c.start);
            cuts.push(c.end);
        }
    }
    for f in finds {
        cuts.push(f.0);
        cuts.push(f.1);
    }
    cuts.sort_unstable();
    cuts.dedup();

    let fmt_for = |a: usize, mut f: TextFormat| -> TextFormat {
        if cfg.colorizer {
            let i = colors.partition_point(|c| c.start <= a);
            if i > 0 && a < colors[i - 1].end {
                let col = colors[i - 1].color;
                match cfg.color_style {
                    ColorStyle::Fill => {
                        // paint over the editor background so translucent colors still read well
                        let solid = crate::theme::mix(cfg.pal.bg, Color32::from_rgb((col.r * 255.0) as u8, (col.g * 255.0) as u8, (col.b * 255.0) as u8), col.a);
                        f.background = solid;
                        f.color = if crate::theme::luminance(solid) > 0.55 { Color32::from_rgb(16, 18, 24) } else { Color32::WHITE };
                    }
                    ColorStyle::Underline => {
                        f.underline = Stroke::new(3.0, col.to_color32());
                    }
                    ColorStyle::Outline => {}
                }
            }
        }
        let j = finds.partition_point(|m| m.0 <= a);
        if j > 0 && a < finds[j - 1].1 {
            f.background = cfg.pal.find_hl;
        }
        f
    };

    let push = |job: &mut LayoutJob, a: usize, b: usize, f: TextFormat| {
        if a >= b {
            return;
        }
        // split at overlay boundaries
        let mut s = a;
        let mut k = cuts.partition_point(|&c| c <= a);
        while s < b {
            let e = if k < cuts.len() && cuts[k] < b { cuts[k] } else { b };
            job.sections.push(egui::text::LayoutSection { leading_space: 0.0, byte_range: egui::text::ByteIndex(s)..egui::text::ByteIndex(e), format: fmt_for(s, f.clone()) });
            s = e;
            k += 1;
        }
    };

    let in_sync = hl.spans.len() == hl.line_starts.len() && !hl.spans.is_empty();
    if !in_sync || text.is_empty() {
        push(&mut job, 0, text.len(), base.clone());
        return job;
    }
    let mut covered = 0;
    for (i, spans) in hl.spans.iter().enumerate() {
        let ls = hl.line_starts[i];
        if spans.is_empty() {
            let end = hl.line_starts.get(i + 1).copied().unwrap_or(text.len());
            push(&mut job, ls, end.min(text.len()), base.clone());
            covered = end.min(text.len());
            continue;
        }
        for (s, e, st) in spans {
            let a = (ls + s).min(text.len());
            let b = (ls + e).min(text.len());
            let mut f = base.clone();
            f.color = Color32::from_rgba_unmultiplied(st.foreground.r, st.foreground.g, st.foreground.b, st.foreground.a);
            f.italics = st.font_style.contains(FontStyle::ITALIC);
            if st.font_style.contains(FontStyle::UNDERLINE) {
                f.underline = Stroke::new(1.0, f.color);
            }
            push(&mut job, a, b, f);
            covered = b;
        }
    }
    if covered < text.len() {
        push(&mut job, covered, text.len(), base);
    }
    job
}

pub fn find_matches(text: &str, q: &str, case: bool, regex: bool) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    if q.is_empty() {
        return out;
    }
    if regex {
        let pat = if case { q.to_string() } else { format!("(?i){q}") };
        if let Ok(re) = fancy_regex::Regex::new(&pat) {
            for m in re.find_iter(text).flatten() {
                if m.end() > m.start() {
                    out.push((m.start(), m.end()));
                }
                if out.len() > 20000 {
                    break;
                }
            }
        }
        return out;
    }
    if case {
        let mut i = 0;
        while let Some(p) = text[i..].find(q) {
            out.push((i + p, i + p + q.len()));
            i += p + q.len().max(1);
            if out.len() > 20000 {
                break;
            }
        }
    } else {
        let t = text.to_ascii_lowercase();
        let ql = q.to_ascii_lowercase();
        let mut i = 0;
        while let Some(p) = t[i..].find(&ql) {
            out.push((i + p, i + p + ql.len()));
            i += p + ql.len().max(1);
            if out.len() > 20000 {
                break;
            }
        }
    }
    out
}

pub fn char_to_byte(text: &str, ch: usize) -> usize {
    text.char_indices().nth(ch).map(|(b, _)| b).unwrap_or(text.len())
}
pub fn byte_to_char(text: &str, b: usize) -> usize {
    text[..b.min(text.len())].chars().count()
}

/// Show the editor for one document inside `rect`.
pub fn editor_view(ui: &mut egui::Ui, id: egui::Id, text: &mut String, vs: &mut ViewState, cfg: &ViewCfg) -> ViewOutput {
    let mut out = ViewOutput::default();
    let full = ui.available_rect_before_wrap();
    ui.painter().rect_filled(full, 0.0, cfg.pal.bg);
    let char_w = ui.ctx().fonts_mut(|f| f.glyph_width(&cfg.font, '0'));
    let line_count = text.bytes().filter(|&b| b == b'\n').count() + 1;
    let digits = (line_count.max(1) as f32).log10().floor() as usize + 1;
    let gutter_w = if cfg.line_numbers { (digits.max(2) as f32) * char_w + 36.0 } else { 18.0 };
    let mm_w = if cfg.minimap { 110.0 } else { 0.0 };
    let gutter_rect = Rect::from_min_max(full.min, Pos2::new(full.left() + gutter_w, full.bottom()));
    let mm_rect = Rect::from_min_max(Pos2::new(full.right() - mm_w, full.top()), full.max);
    let text_rect = Rect::from_min_max(Pos2::new(gutter_rect.right(), full.top()), Pos2::new(mm_rect.left(), full.bottom()));

    // apply a requested selection before the widget runs
    let te_id = id.with("te");
    if let Some((a, b)) = vs.select.take() {
        let mut st = egui::TextEdit::load_state(ui.ctx(), te_id).unwrap_or_default();
        st.cursor.set_char_range(Some(egui::text::CCursorRange::two(egui::text::CCursor::new(a), egui::text::CCursor::new(b))));
        egui::TextEdit::store_state(ui.ctx(), te_id, st);
        ui.memory_mut(|m| m.request_focus(te_id));
        vs.focus = true;
        vs.cursor = Some((b, a));
        vs.select = Some((a, b)); // keep for scroll after layout
    }
    if vs.focus {
        ui.memory_mut(|m| m.request_focus(te_id));
        vs.focus = false;
    }

    let finds_q = cfg.find.map(|(q, c, r)| (q.to_string(), c, r));
    let col_wrap = if cfg.wrap && cfg.wrap_column > 0 { Some(cfg.wrap_column as f32 * char_w + 12.0) } else { None };
    let mut layouter = |ui: &egui::Ui, buf: &dyn egui::TextBuffer, wrap_width: f32| -> Arc<egui::Galley> {
        let t = buf.as_str();
        vs.hl.update(t, cfg.syntax, cfg.ss, cfg.theme, &cfg.hl_key);
        if cfg.colorizer || cfg.gutter_swatches {
            vs.colors.update(t, hash_text(t), cfg.named_colors);
        }
        let finds = finds_q.as_ref().map(|(q, c, r)| find_matches(t, q, *c, *r)).unwrap_or_default();
        let ww = col_wrap.map(|w| w.min(wrap_width)).unwrap_or(wrap_width);
        let job = build_job(t, cfg, &vs.hl, &vs.colors.list, &finds, ww);
        ui.ctx().fonts_mut(|f| f.layout_job(job))
    };

    let mut sa = egui::ScrollArea::both().id_salt(id.with("scroll")).auto_shrink([false, false]);
    if cfg.wrap {
        sa = egui::ScrollArea::vertical().id_salt(id.with("scroll")).auto_shrink([false, false]);
    }
    if let Some(y) = vs.set_scroll.take() {
        sa = sa.vertical_scroll_offset(y.max(0.0));
    }
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(text_rect));
    let view_rows = (text_rect.height() / cfg.line_height).ceil() as usize;
    let sel_take = vs.select.take();
    let scroll_out = sa.show(&mut child, |ui| {
        let bg_idx = ui.painter().add(Shape::Noop);
        let te = egui::TextEdit::multiline(text)
            .id(te_id)
            .font(cfg.font.clone())
            .frame(egui::Frame::NONE)
            .margin(egui::Margin { left: 4, right: 8, top: 0, bottom: 0 })
            .desired_width(if cfg.wrap { text_rect.width() - 16.0 } else { f32::INFINITY })
            .desired_rows(view_rows.max(1))
            .lock_focus(true)
            .text_color(cfg.pal.fg)
            .layouter(&mut layouter);
        let o = te.show(ui);
        // scroll past end (original: scroll_past_end = true)
        ui.allocate_space(egui::vec2(1.0, (text_rect.height() - cfg.line_height * 3.0).max(0.0)));
        if let Some((a, b)) = sel_take {
            let r = o.galley.pos_from_cursor(egui::text::CCursor::new(b.max(a))).translate(o.galley_pos.to_vec2());
            ui.scroll_to_rect(r.expand(cfg.line_height * 2.0), Some(egui::Align::Center));
        }
        (o, bg_idx)
    });
    let (o, bg_idx) = scroll_out.inner;
    vs.scroll_y = scroll_out.state.clamped_offset().y;
    vs.content_h = scroll_out.content_size.y;
    vs.view_h = scroll_out.inner_rect.height();
    out.changed = o.response.changed();
    out.galley_rows = o.galley.rows.len();
    if let Some(r) = o.cursor_range {
        vs.cursor = Some((r.primary.index.0, r.secondary.index.0));
    }
    let galley = &o.galley;
    let gpos = o.galley_pos;
    let painter = ui.painter().with_clip_rect(full);
    let text_painter = ui.painter().with_clip_rect(text_rect);
    let clip = text_rect;

    // current line highlight + gutter
    let cursor_char = vs.cursor.map(|c| c.0).unwrap_or(0);
    let cur_rect = galley.pos_from_cursor(egui::text::CCursor::new(cursor_char)).translate(gpos.to_vec2());
    if cfg.highlight_line || o.response.has_focus() {
        let r = Rect::from_min_max(Pos2::new(text_rect.left(), cur_rect.top()), Pos2::new(text_rect.right(), cur_rect.bottom()));
        if cfg.highlight_line {
            ui.painter().set(bg_idx, Shape::rect_filled(r, 0.0, cfg.pal.line_hl));
        }
    }
    painter.rect_filled(gutter_rect, 0.0, cfg.pal.bg);

    // walk galley rows: line numbers, indent guides, swatches
    let mut line_no = 0usize;
    let mut at_line_start = true;
    let mut char_idx = 0usize;
    let text_now: &str = text.as_str();
    let gutter_font = FontId::new(cfg.font.size * 0.92, cfg.font.family.clone());
    let cur_line_top = cur_rect.top();
    let colors = &vs.colors.list;
    let mut color_i = 0usize;
    let mut byte_idx = 0usize;
    for prow in galley.rows.iter() {
        let row_top = gpos.y + prow.pos.y;
        let row_h = prow.rect().height();
        let row_chars = prow.char_count_excluding_newline().0;
        let row_char_start = char_idx;
        let visible = row_top + row_h >= clip.top() && row_top <= clip.bottom();
        let row_text_len_bytes: usize = text_now[byte_idx..].chars().take(row_chars).map(|c| c.len_utf8()).sum();
        let row_byte_start = byte_idx;
        if at_line_start {
            line_no += 1;
            if visible {
                let l0 = line_no - 1;
                // changed since save: thin accent bar at the gutter's edge
                if let Some((ma, mb)) = cfg.modified {
                    let x = gutter_rect.right() - 2.5;
                    if l0 >= ma && l0 < mb {
                        painter.vline(x, row_top..=row_top + row_h, Stroke::new(2.5, mix(cfg.pal.accent, cfg.pal.bg, 0.15)));
                    } else if ma == mb && l0 == ma {
                        painter.add(Shape::convex_polygon(
                            vec![Pos2::new(x - 3.0, row_top - 3.0), Pos2::new(x + 1.5, row_top), Pos2::new(x - 3.0, row_top + 3.0)],
                            Color32::from_rgb(0xf0, 0x5d, 0x5d),
                            Stroke::NONE,
                        ));
                    }
                }
                if cfg.bookmarks.contains(&l0) {
                    let cy = row_top + row_h / 2.0;
                    let x0 = gutter_rect.left() + 4.0;
                    painter.add(Shape::convex_polygon(
                        vec![Pos2::new(x0, cy - 6.0), Pos2::new(x0 + 8.0, cy - 6.0), Pos2::new(x0 + 8.0, cy + 6.0), Pos2::new(x0 + 4.0, cy + 2.5), Pos2::new(x0, cy + 6.0)],
                        cfg.pal.accent,
                        Stroke::NONE,
                    ));
                }
                if cfg.line_numbers {
                    let is_cur = (row_top - cur_line_top).abs() < 1.0;
                    let col = if is_cur { cfg.pal.fg } else { cfg.pal.gutter_fg };
                    painter.text(
                        Pos2::new(gutter_rect.right() - 17.0, row_top + row_h / 2.0),
                        egui::Align2::RIGHT_CENTER,
                        line_no.to_string(),
                        gutter_font.clone(),
                        col,
                    );
                }
                // indent guides (original: indent_guide_options = draw_normal)
                let line_text = &text_now[row_byte_start..row_byte_start + row_text_len_bytes];
                let mut cols = 0usize;
                for ch in line_text.chars() {
                    match ch {
                        ' ' => cols += 1,
                        '\t' => cols += cfg.tab_size - cols % cfg.tab_size,
                        _ => break,
                    }
                }
                if !line_text.trim().is_empty() {
                    let mut g = cfg.tab_size;
                    while g <= cols && g > 0 {
                        let x = gpos.x + 4.0 + (g - cfg.tab_size) as f32 * char_w + 1.0;
                        text_painter.vline(x, row_top..=row_top + row_h, Stroke::new(1.0, mix(cfg.pal.bg, cfg.pal.fg, 0.08)));
                        g += cfg.tab_size;
                    }
                }
            }
        }
        // colors on this row: gutter swatches + outline style
        let row_byte_end = row_byte_start + row_text_len_bytes;
        let mut dots = 0;
        while color_i < colors.len() && colors[color_i].start < row_byte_end + 1 {
            let c = &colors[color_i];
            if c.start >= row_byte_start && visible {
                if cfg.gutter_swatches && dots < 3 {
                    let x = gutter_rect.right() - 7.0 - dots as f32 * 3.0;
                    let rr = Rect::from_center_size(Pos2::new(x, row_top + row_h / 2.0), egui::vec2(6.0, 6.0));
                    painter.rect_filled(rr, 2.0, c.color.to_color32());
                    painter.rect_stroke(rr, 2.0, Stroke::new(1.0, mix(cfg.pal.bg, cfg.pal.fg, 0.3)), egui::StrokeKind::Outside);
                    dots += 1;
                }
                if cfg.colorizer && cfg.color_style == ColorStyle::Outline {
                    let a = row_char_start + text_now[row_byte_start..c.start].chars().count();
                    let b = a + text_now[c.start..c.end.min(text_now.len())].chars().count();
                    let ra = galley.pos_from_cursor(egui::text::CCursor::new(a)).translate(gpos.to_vec2());
                    let rb = galley.pos_from_cursor(egui::text::CCursor::new(b)).translate(gpos.to_vec2());
                    let r = Rect::from_min_max(ra.min, Pos2::new(rb.max.x, ra.max.y)).expand2(egui::vec2(2.0, 0.0));
                    text_painter.rect_stroke(r, 3.0, Stroke::new(2.0, c.color.to_color32()), egui::StrokeKind::Outside);
                }
            }
            color_i += 1;
        }
        // advance (galley rows: newline counts as one char at end of row)
        char_idx += row_chars;
        byte_idx += row_text_len_bytes;
        if prow.ends_with_newline {
            char_idx += 1;
            byte_idx += 1;
        }
        at_line_start = prow.ends_with_newline;
    }

    // diagnostics squiggles and gutter markers
    if let Some(diags) = cfg.diags {
        let line_starts: Vec<usize> = std::iter::once(0).chain(text_now.char_indices().filter(|(_, c)| *c == '\n').map(|(i, _)| i + 1)).collect();
        for d in diags {
            let Some(&ls) = line_starts.get(d.line) else { continue };
            let line_end = line_starts.get(d.line + 1).map(|e| e - 1).unwrap_or(text_now.len());
            let line = &text_now[ls..line_end.max(ls)];
            let c0 = byte_to_char(text_now, ls) + crate::lsp::char_col_from_utf16(line, d.col);
            let c1 = if d.end_line == d.line {
                byte_to_char(text_now, ls) + crate::lsp::char_col_from_utf16(line, d.end_col).max(crate::lsp::char_col_from_utf16(line, d.col) + 1)
            } else {
                byte_to_char(text_now, line_end)
            };
            let ra = galley.pos_from_cursor(egui::text::CCursor::new(c0)).translate(gpos.to_vec2());
            let rb = galley.pos_from_cursor(egui::text::CCursor::new(c1)).translate(gpos.to_vec2());
            let col = match d.severity {
                1 => Color32::from_rgb(0xf0, 0x5d, 0x5d),
                2 => Color32::from_rgb(0xf5, 0xb9, 0x42),
                _ => Color32::from_rgb(0x5a, 0xb0, 0xf6),
            };
            let y = ra.bottom() - 1.0;
            let x1 = if (rb.top() - ra.top()).abs() < 1.0 { rb.left().max(ra.left() + char_w) } else { ra.left() + char_w * 4.0 };
            let mut pts = Vec::new();
            let mut x = ra.left();
            let mut up = true;
            while x <= x1 {
                pts.push(Pos2::new(x, if up { y - 1.5 } else { y + 1.0 }));
                x += 3.0;
                up = !up;
            }
            if pts.len() >= 2 {
                text_painter.add(Shape::line(pts, Stroke::new(1.2, col)));
            }
            if ra.top() >= clip.top() && ra.top() <= clip.bottom() {
                painter.circle_filled(Pos2::new(gutter_rect.left() + 6.0, ra.center().y), 3.5, col);
            }
        }
    }

    // rulers (View ▸ Ruler)
    for &col in cfg.rulers {
        let x = gpos.x + 4.0 + col as f32 * char_w;
        if x > text_rect.left() && x < text_rect.right() {
            text_painter.vline(x, text_rect.y_range(), Stroke::new(1.0, mix(cfg.pal.bg, cfg.pal.fg, 0.12)));
        }
    }

    // gutter separator
    painter.vline(gutter_rect.right() - 1.0, gutter_rect.y_range(), Stroke::new(1.0, mix(cfg.pal.bg, cfg.pal.fg, 0.04)));

    // hover & alt-click on colors
    if let Some(pp) = ui.input(|i| i.pointer.hover_pos()) {
        if text_rect.contains(pp) {
            let cc = galley.cursor_from_pos(pp - gpos);
            let ch = cc.index.0;
            out.hover_char = Some(ch);
            let b = char_to_byte(text_now, ch);
            let hit = vs.colors.at(b).or_else(|| if b > 0 { vs.colors.at(b - 1) } else { None }).cloned();
            if let Some(cm) = hit {
                if cfg.colorizer {
                    let alt = ui.input(|i| i.modifiers.alt);
                    if alt && ui.input(|i| i.pointer.primary_clicked()) {
                        out.color_edit = Some(cm.clone());
                    }
                    egui::Tooltip::always_open(ui.ctx().clone(), ui.layer_id(), id.with("color_tip"), egui::PopupAnchor::Pointer).show(|ui| {
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(egui::vec2(44.0, 44.0), Sense::hover());
                            checker(ui.painter(), r);
                            ui.painter().rect_filled(r, 6.0, cm.color.to_color32());
                            ui.vertical(|ui| {
                                ui.monospace(cm.color.to_hex());
                                ui.monospace(cm.color.to_rgb_css());
                                ui.monospace(cm.color.to_hsl_css());
                            });
                        });
                        ui.small("Alt+Click to edit this color");
                    });
                }
            }
        }
    }
    // gutter swatch click opens the color editor for that line's first color
    let gresp = ui.interact(gutter_rect, id.with("gutter"), Sense::click());
    if gresp.clicked() {
        if let Some(pp) = gresp.interact_pointer_pos() {
            let cc = galley.cursor_from_pos(egui::vec2(0.0, pp.y - gpos.y));
            let b = char_to_byte(text_now, cc.index.0);
            if let Some(c) = vs.colors.list.iter().find(|c| c.start >= b && !text_now[b..c.start].contains('\n')) {
                out.color_edit = Some(c.clone());
            }
        }
    }

    // minimap
    if cfg.minimap {
        minimap(ui, id, mm_rect, vs, cfg, line_count, text_now);
    }
    out
}

pub fn checker(p: &egui::Painter, r: Rect) {
    let s = 6.0;
    let mut y = r.top();
    let mut row = 0;
    while y < r.bottom() {
        let mut x = r.left() + if row % 2 == 0 { 0.0 } else { s };
        while x < r.right() {
            let rr = Rect::from_min_size(Pos2::new(x, y), egui::vec2(s, s)).intersect(r);
            p.rect_filled(rr, 0.0, Color32::from_gray(200));
            x += s * 2.0;
        }
        y += s;
        row += 1;
    }
}

fn minimap(ui: &mut egui::Ui, id: egui::Id, rect: Rect, vs: &mut ViewState, cfg: &ViewCfg, line_count: usize, text: &str) {
    let p = ui.painter().with_clip_rect(rect);
    p.rect_filled(rect, 0.0, cfg.pal.bg);
    let lh = 2.6f32; // minimap line height in px
    let cw = 1.15f32;
    let total_h = line_count as f32 * lh;
    let frac = if vs.content_h > vs.view_h { (vs.scroll_y / (vs.content_h - vs.view_h)).clamp(0.0, 1.0) } else { 0.0 };
    let overflow = (total_h - rect.height()).max(0.0);
    let mm_off = frac * overflow;
    let first = (mm_off / lh) as usize;
    let last = ((mm_off + rect.height()) / lh) as usize + 1;
    let hl = &vs.hl;
    for li in first..last.min(hl.spans.len()) {
        let y = rect.top() + li as f32 * lh - mm_off;
        let ls = hl.line_starts[li];
        for (s, e, st) in &hl.spans[li] {
            let col = Color32::from_rgba_unmultiplied(st.foreground.r, st.foreground.g, st.foreground.b, 150);
            let a = (ls + s).min(text.len());
            let b = (ls + e).min(text.len());
            let Some(piece) = text.get(a..b) else { continue };
            // draw runs of non-space chars; columns approximated by bytes (tabs = tab_size)
            let mut col_x = 0usize;
            if let Some(prefix) = text.get(ls..a) {
                for ch in prefix.chars() {
                    col_x += if ch == '\t' { cfg.tab_size } else { 1 };
                }
            }
            let mut run_start: Option<usize> = None;
            for ch in piece.chars().chain(std::iter::once(' ')) {
                if ch.is_whitespace() {
                    if let Some(rs) = run_start.take() {
                        let x = rect.left() + 6.0 + rs as f32 * cw;
                        let w = ((col_x - rs) as f32 * cw).min(rect.right() - x);
                        if w > 0.0 {
                            p.rect_filled(Rect::from_min_size(Pos2::new(x, y), egui::vec2(w, lh - 0.8)), 0.0, col);
                        }
                    }
                } else if run_start.is_none() {
                    run_start = Some(col_x);
                }
                col_x += if ch == '\t' { cfg.tab_size } else { 1 };
            }
        }
    }
    // viewport box (original: always_show_minimap_viewport = false → shown on hover)
    let resp = ui.interact(rect, id.with("minimap"), Sense::click_and_drag());
    let view_frac = if vs.content_h > 0.0 { (vs.view_h / vs.content_h).min(1.0) } else { 1.0 };
    let vb_h = (total_h.max(rect.height().min(total_h)) * view_frac).max(12.0).min(rect.height());
    let vb_top = rect.top() + frac * (total_h.min(rect.height()) - vb_h).max(0.0);
    // viewport colour: the minimap control's built-in default in sub_140387134
    // (u32 0x60444444 at +0x120 → r = g = b = 0x44, alpha 0x60)
    if resp.hovered() || resp.dragged() {
        p.rect_filled(Rect::from_min_size(Pos2::new(rect.left(), vb_top), egui::vec2(rect.width(), vb_h)), 0.0, Color32::from_rgba_unmultiplied(0x44, 0x44, 0x44, 0x60));
    }
    let max_scroll = (vs.content_h - vs.view_h).max(0.0);
    let track = (total_h.min(rect.height()) - vb_h).max(1.0);
    if resp.drag_started() {
        if let Some(pp) = resp.interact_pointer_pos() {
            // pressing inside the viewport box grabs it (sub_1403876e2)
            vs.mm_grab = if pp.y >= vb_top && pp.y <= vb_top + vb_h { Some(pp.y - vb_top) } else { None };
        }
    }
    if resp.dragged() {
        if let (Some(grab), Some(pp)) = (vs.mm_grab, resp.interact_pointer_pos()) {
            let top = (pp.y - grab - rect.top()).clamp(0.0, track);
            vs.set_scroll = Some(top / track * max_scroll);
        }
    }
    if resp.drag_stopped() {
        vs.mm_grab = None;
    }
    let pressed_outside = resp.clicked() || (resp.dragged() && vs.mm_grab.is_none());
    if pressed_outside {
        if let Some(pp) = resp.interact_pointer_pos() {
            // minimap_scroll_to_clicked_text = true: centre the clicked line,
            // offset = half the visible height (0.5 at 0x1405aa1d8)
            let line = ((pp.y - rect.top() + mm_off) / lh).max(0.0);
            let y = line * cfg.line_height - vs.view_h * 0.5;
            vs.set_scroll = Some(y.clamp(0.0, max_scroll));
        }
    }
    p.vline(rect.left(), rect.y_range(), Stroke::new(1.0, mix(cfg.pal.bg, cfg.pal.fg, 0.05)));
}

/// Highlight a code snippet once (for markdown preview code blocks).
pub fn highlight_snippet(code: &str, lang: &str, ss: &SyntaxSet, theme: &Theme, font: FontId, fg: Color32) -> LayoutJob {
    use syntect::easy::HighlightLines;
    let syn = ss.find_syntax_by_token(lang).unwrap_or_else(|| ss.find_syntax_plain_text());
    let mut h = HighlightLines::new(syn, theme);
    let mut job = LayoutJob::default();
    for line in code.split_inclusive('\n') {
        match h.highlight_line(line, ss) {
            Ok(parts) => {
                for (st, piece) in parts {
                    job.append(piece, 0.0, TextFormat {
                        font_id: font.clone(),
                        color: Color32::from_rgb(st.foreground.r, st.foreground.g, st.foreground.b),
                        italics: st.font_style.contains(FontStyle::ITALIC),
                        ..Default::default()
                    });
                }
            }
            Err(_) => job.append(line, 0.0, TextFormat { font_id: font.clone(), color: fg, ..Default::default() }),
        }
    }
    job
}
