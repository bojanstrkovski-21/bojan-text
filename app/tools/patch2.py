# One-off source patch: tab sizes, pane edges, expand-to-tag by scope, find options.
import io

def rd(p):
    return io.open(p, encoding='utf-8').read()

def wr(p, s):
    io.open(p, 'w', encoding='utf-8', newline='\n').write(s)

def rep(s, old, new, count=1):
    assert old in s, old[:60]
    return s.replace(old, new, count)

# ---------------- ui.rs
p = 'src/app/ui.rs'
s = rd(p)
# tab sizes from sub_1400e3d5a: tab_height 35, tab_min_width 48, tab_width 180
s = rep(s, "let w = (galley.size().x + 52.0).clamp(90.0, 260.0);", "let w = (galley.size().x + 52.0).clamp(48.0, 180.0);")
s = rep(s, "egui::Sense::click());\n                        let hovered = resp.hovered();", "egui::Sense::click());\n                        let hovered = resp.hovered();")
s = rep(s, "allocate_exact_size(egui::vec2(w, 36.0), egui::Sense::click())", "allocate_exact_size(egui::vec2(w, 35.0), egui::Sense::click())")
s = rep(s, "ui.painter().text(egui::pos2(r.left() + 14.0, r.center().y), egui::Align2::LEFT_CENTER, &d.title, font, txt_col);",
        "ui.painter().with_clip_rect(egui::Rect::from_min_max(r.left_top(), egui::pos2(r.right() - 30.0, r.bottom()))).text(egui::pos2(r.left() + 14.0, r.center().y), egui::Align2::LEFT_CENTER, &d.title, font, txt_col);")
s = rep(s, 'egui::Panel::top(("pane_tabs", g)).exact_size(36.0)', 'egui::Panel::top(("pane_tabs", g)).exact_size(35.0)')
# pane edges: the original's 3-way layouts use 0.33 / 0.66 (Default (Windows).sublime-keymap)
OLD = '''            let (r, c) = (g / self.cols, g % self.cols);
            let in_row = (self.groups - r * self.cols).min(self.cols);
            let w = (full.width() - gap * (in_row as f32 - 1.0)) / in_row as f32;
            let h = (full.height() - gap * (rows as f32 - 1.0)) / rows as f32;
            let min = egui::pos2(full.left() + c as f32 * (w + gap), full.top() + r as f32 * (h + gap));
            let rect = egui::Rect::from_min_size(min, egui::vec2(w, h));'''
NEW = '''            let (r, c) = (g / self.cols, g % self.cols);
            let in_row = (self.groups - r * self.cols).min(self.cols);
            let (x0, x1) = (edge(c, in_row), edge(c + 1, in_row));
            let (y0, y1) = (edge(r, rows), edge(r + 1, rows));
            let rect = egui::Rect::from_min_max(
                egui::pos2(full.left() + x0 * full.width() + if c > 0 { gap / 2.0 } else { 0.0 }, full.top() + y0 * full.height() + if r > 0 { gap / 2.0 } else { 0.0 }),
                egui::pos2(full.left() + x1 * full.width() - if c + 1 < in_row { gap / 2.0 } else { 0.0 }, full.top() + y1 * full.height() - if r + 1 < rows { gap / 2.0 } else { 0.0 }),
            );'''
s = rep(s, OLD, NEW)
OLD2 = '''        if self.groups > 1 {
            for r in 1..rows {
                let y = full.top() + r as f32 * ((full.height() - gap * (rows as f32 - 1.0)) / rows as f32 + gap) - gap / 2.0;
                ui.painter().hline(full.x_range(), y, egui::Stroke::new(1.0, p.border));
            }
        }'''
NEW2 = '''        if self.groups > 1 {
            for r in 1..rows {
                let y = full.top() + edge(r, rows) * full.height();
                ui.painter().hline(full.x_range(), y, egui::Stroke::new(1.0, p.border));
            }
        }'''
s = rep(s, OLD2, NEW2)
s = rep(s, "            if self.groups > 1 && c + 1 < in_row {\n                ui.painter().vline(rect.right() + gap / 2.0, rect.y_range(), egui::Stroke::new(1.0, p.border));",
        "            if self.groups > 1 && c + 1 < in_row {\n                ui.painter().vline(full.left() + x1 * full.width(), rect.y_range(), egui::Stroke::new(1.0, p.border));")
# helper at top of file
s = rep(s, "/// Status-bar text, following sub_14036805e:", '''/// Position of split line `i` of `n` equal parts. The original's 3-way layouts use
/// 0.33 / 0.66 (not exact thirds) in its set_layout key bindings.
fn edge(i: usize, n: usize) -> f32 {
    if i == 0 {
        0.0
    } else if i >= n {
        1.0
    } else if n == 3 {
        [0.0, 0.33, 0.66, 1.0][i]
    } else {
        i as f32 / n as f32
    }
}

/// Status-bar text, following sub_14036805e:''')
wr(p, s)

# ---------------- features.rs : expand_tag by scope
p = 'src/app/features.rs'
s = rd(p)
OLD = '''                let (lo, hi) = self.sel();
                if let Some(d) = self.docs.get(self.active) {
                    let c: Vec<char> = d.text.chars().collect();
                    let r = textops::expand_to_tag(&d.text, lo, hi).or_else(|| Some(textops::expand_smart(&c, lo, hi)));'''
NEW = '''                let (lo, hi) = self.sel();
                let scope = self.doc_scope();
                // keymap: tag expansion only where selector is "(text.html, text.xml) - source"
                let markup = (scope.starts_with("text.html") || scope.starts_with("text.xml")) && !scope.starts_with("source");
                if let Some(d) = self.docs.get(self.active) {
                    let c: Vec<char> = d.text.chars().collect();
                    let r = if markup { textops::expand_to_tag(&d.text, lo, hi) } else { None }.or_else(|| Some(textops::expand_smart(&c, lo, hi)));'''
s = rep(s, OLD, NEW)
wr(p, s)
print('patched2')
