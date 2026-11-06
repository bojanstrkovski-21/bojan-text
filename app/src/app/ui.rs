//! Window parts: menu bar, status bar, side bar, tabs, find bar, command
//! palette, settings windows and the editor area.

use super::*;

/// Position of split line `i` of `n` equal parts. The original's 3-way layouts use
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

/// Status-bar text, following sub_14036805e:
/// - empty selection: "Line L, Column C" where C is a *virtual* column
///   (selection_description_column_type = "virtual"): a tab advances to the
///   next tab stop, col += tab - col % tab; any other char adds 1.
/// - selection: "N lines, M characters selected" when it spans lines
///   (N = line breaks + 1), "1 character selected" for one char.
pub fn selection_description(text: &str, a: usize, b: usize, tab: usize) -> String {
    if a == b {
        let before: Vec<char> = text.chars().take(a).collect();
        let line = before.iter().filter(|c| **c == '\n').count() + 1;
        let ls = before.iter().rposition(|c| *c == '\n').map(|i| i + 1).unwrap_or(0);
        let mut col = 0usize;
        for ch in &before[ls..] {
            if *ch == '\t' {
                col += tab - col % tab;
            } else {
                col += 1;
            }
        }
        return format!("Line {line}, Column {}", col + 1);
    }
    let (lo, hi) = (a.min(b), a.max(b));
    let n = hi - lo;
    let breaks = text.chars().skip(lo).take(n).filter(|c| *c == '\n').count();
    let chars = if n == 1 { "1 character selected".to_string() } else { format!("{n} characters selected") };
    if breaks > 0 {
        format!("{} lines, {chars}", breaks + 1)
    } else {
        chars
    }
}

#[cfg(test)]
mod tests {
    use super::selection_description;
    #[test]
    fn status_text() {
        // a tab counts to the next tab stop ("virtual" columns)
        assert_eq!(selection_description("\tab", 2, 2, 4), "Line 1, Column 6");
        assert_eq!(selection_description("x\ny", 2, 2, 4), "Line 2, Column 1");
        assert_eq!(selection_description("abc", 0, 1, 4), "1 character selected");
        assert_eq!(selection_description("ab\ncd", 0, 5, 4), "2 lines, 5 characters selected");
    }
}

impl App {
    pub(super) fn status_bar(&mut self, ui: &mut egui::Ui) {
        let p = self.pal;
        let mut open_syntax = false;
        let mut anchor = self.syntax_anchor;
        let mut open_lsp = false;
        let mut open_colors = false;
        let fsz = self.s.theme.ui_font_size * 0.92;
        let small = |t: String| RichText::new(t).size(fsz).color(mix(p.fg, p.status_bg, 0.25));
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 16.0;
            let Some(d) = self.docs.get(self.active) else { return };
            let (a, b) = d.view.cursor.unwrap_or((0, 0));
            let desc = selection_description(&d.text, a, b, self.s.tab_size.max(1));
            // when the find bar is in use, the original shows "N of M matches"
            let find_desc = if self.find.open && !self.fif.open && !self.find.query.is_empty() {
                let m = self.find_all(&d.text);
                let (lo, hi) = (char_to_byte(&d.text, a.min(b)), char_to_byte(&d.text, a.max(b)));
                match m.len() {
                    0 => None,
                    1 => Some("1 match".to_string()),
                    n => Some(match m.iter().position(|x| *x == (lo, hi)) {
                        Some(i) => format!("{} of {n} matches", i + 1),
                        None => format!("{n} matches"),
                    }),
                }
            } else {
                None
            };
            ui.label(small(find_desc.unwrap_or(desc)));
            if let Some((m, t)) = &self.status {
                if t.elapsed().as_secs_f32() < 6.0 {
                    ui.label(RichText::new(m).size(fsz).color(p.accent));
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let syn = self.syntax_name_for(d);
                let r = ui.add(egui::Label::new(small(syn)).sense(egui::Sense::click()));
                if r.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                anchor = r.rect;
                if r.on_hover_text("Set syntax").clicked() {
                    open_syntax = true;
                }
                // original: "Spaces: %d" / "Tab Size: %d" (sub_1400eb0e2)
                let ind = if self.s.translate_tabs { format!("Spaces: {}", self.s.tab_size) } else { format!("Tab Size: {}", self.s.tab_size) };
                ui.label(small(ind));
                if self.s.colorizer {
                    let n = d.view.colors.list.len();
                    if n > 0 {
                        let r = ui.add(egui::Label::new(small(format!("● {n} colors"))).sense(egui::Sense::click()));
                        if r.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if r.on_hover_text("List color codes").clicked() {
                            open_colors = true;
                        }
                    }
                }
                if d.path.is_some() {
                    if let Some(spec) = self.spec_for(&d.ext()) {
                        let st = self
                            .clients
                            .get(&spec.name)
                            .map(|c| c.status.clone())
                            .unwrap_or_else(|| self.lsp_errors.get(&spec.name).cloned().unwrap_or("off".into()));
                        let key = d.path.as_ref().map(|p| lsp::norm_key(p)).unwrap_or_default();
                        let errs = self.diags.get(&key).map(|v| v.iter().filter(|x| x.severity == 1).count()).unwrap_or(0);
                        let warns = self.diags.get(&key).map(|v| v.iter().filter(|x| x.severity == 2).count()).unwrap_or(0);
                        let r = ui.add(egui::Label::new(small(format!("{}: {}  ✖ {errs}  ⚠ {warns}", spec.name, st))).sense(egui::Sense::click()));
                        if r.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if r.clicked() {
                            open_lsp = true;
                        }
                    }
                }
                if d.is_markdown() || d.md_mode != MdMode::Off {
                    let t = match d.md_mode {
                        MdMode::Off => "Preview off",
                        MdMode::Split => "Preview: split",
                        MdMode::Preview => "Preview only",
                    };
                    ui.label(small(t.into()));
                }
                if self.scan_rx.is_some() {
                    ui.add(egui::Spinner::new().size(11.0));
                    ui.label(small("Loading Sublime packages…".into()));
                }
            });
        });
        self.syntax_anchor = anchor;
        if open_syntax {
            self.syntax_popup = !self.syntax_popup;
        }
        if open_lsp {
            self.run("toggle_problems");
        }
        if open_colors {
            self.win_colors = true;
        }
    }

    /// The syntax chooser: a narrow list that opens just above the syntax name in the
    /// status bar (like the original's), with a check mark on the current syntax.
    pub(super) fn syntax_chooser(&mut self, ctx: &egui::Context) {
        if !self.syntax_popup {
            return;
        }
        let p = self.pal;
        let Some(d) = self.docs.get(self.active) else {
            self.syntax_popup = false;
            return;
        };
        let current = self.syntax_name_for(d);
        let mut names: Vec<String> = self
            .syntax_set
            .syntaxes()
            .iter()
            .filter(|s| !s.hidden && !s.name.contains("(for ") && !s.name.contains("Embedded"))
            .map(|s| s.name.clone())
            .collect();
        names.sort_by_key(|n| n.to_lowercase());
        names.dedup();
        let anchor = self.syntax_anchor;
        let width = 210.0f32;
        let max_h = (anchor.top() - 56.0).clamp(160.0, 640.0);
        let mut chosen: Option<String> = None;
        let radius = self.s.theme.radius;
        let resp = egui::Area::new(egui::Id::new("syntax_chooser"))
            .order(egui::Order::Foreground)
            .pivot(egui::Align2::RIGHT_BOTTOM)
            .fixed_pos(egui::pos2(anchor.right(), anchor.top() - 4.0))
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).fill(mix(p.sidebar_bg, p.bg, 0.3)).corner_radius(radius).inner_margin(egui::Margin::symmetric(0, 4)).show(ui, |ui| {
                    ui.set_width(width);
                    ui.spacing_mut().item_spacing.y = 0.0;
                    egui::ScrollArea::vertical().max_height(max_h).auto_shrink([false, true]).show(ui, |ui| {
                        for n in &names {
                            let (rect, r) = ui.allocate_exact_size(egui::vec2(width, 24.0), egui::Sense::click());
                            if r.hovered() {
                                ui.painter().rect_filled(rect, 0.0, mix(p.sidebar_bg, p.accent, 0.3));
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                            let is_cur = *n == current;
                            if is_cur {
                                ui.painter().text(egui::pos2(rect.left() + 10.0, rect.center().y), egui::Align2::LEFT_CENTER, "✔", FontId::proportional(13.0), p.fg);
                                if !self.syntax_scrolled {
                                    r.scroll_to_me(Some(egui::Align::Center));
                                    self.syntax_scrolled = true;
                                }
                            }
                            ui.painter().text(egui::pos2(rect.left() + 32.0, rect.center().y), egui::Align2::LEFT_CENTER, n, FontId::proportional(self.s.theme.ui_font_size), p.fg);
                            if r.clicked() {
                                chosen = Some(n.clone());
                            }
                        }
                    });
                });
            });
        let outside = ctx.input(|i| i.pointer.any_pressed()) && ctx.input(|i| i.pointer.interact_pos()).map(|pp| !resp.response.rect.contains(pp) && !self.syntax_anchor.contains(pp)).unwrap_or(false);
        if let Some(n) = chosen {
            if let Some(d) = self.docs.get_mut(self.active) {
                d.syntax_override = Some(n);
                d.view.hl.invalidate();
            }
            self.syntax_popup = false;
        } else if outside || ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.syntax_popup = false;
        }
        if !self.syntax_popup {
            self.syntax_scrolled = false;
        }
    }

    pub(super) fn sidebar(&mut self, ui: &mut egui::Ui) {
        let p = self.pal;
        let hs = self.s.theme.ui_font_size * 0.8;
        let head = |t: &str| RichText::new(t).size(hs).strong().color(mix(p.fg, p.sidebar_bg, 0.45));
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.add_space(10.0);
            let show_open = self.s.show_open_files;
            if show_open {
                ui.label(head("OPEN FILES"));
                ui.add_space(2.0);
            }
            let mut activate = None;
            let mut close = None;
            for (i, d) in self.docs.iter().enumerate().filter(|_| show_open) {
                let sel = i == self.active;
                let label = format!("{}{}", if d.dirty() { "● " } else { "" }, d.title);
                let col = if sel { p.fg } else { mix(p.fg, p.sidebar_bg, 0.25) };
                let resp = ui.add(egui::Button::selectable(sel, RichText::new(label).color(col)).frame_when_inactive(false));
                if resp.clicked() {
                    activate = Some(i);
                }
                if resp.middle_clicked() {
                    close = Some(i);
                }
            }
            if let Some(i) = activate {
                self.active = i;
            }
            if let Some(i) = close {
                self.close_doc(i);
            }
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.label(head("FOLDERS"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("+").on_hover_text("Add Folder to Project…").clicked() {
                        self.run("add_folder");
                    }
                });
            });
            let roots = self.all_folders();
            if !roots.is_empty() {
                let mut open = None;
                for root in roots {
                    let name = root.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or(root.display().to_string());
                    self.tree(ui, &root, &name, 0, &mut open);
                }
                if let Some(f) = open {
                    self.open_path(&f);
                }
            } else {
                ui.add_space(4.0);
                ui.label(RichText::new("No folder open.\nFile ▸ Open Folder… or drop a folder here.").color(p.muted));
            }
        });
    }

    fn tree(&mut self, ui: &mut egui::Ui, dir: &Path, name: &str, depth: usize, open: &mut Option<PathBuf>) {
        let p = self.pal;
        let expanded = depth == 0 || self.expanded.contains(dir);
        let arrow = if expanded { "▾" } else { "▸" };
        let resp = ui.add(egui::Button::new(RichText::new(format!("{}{} {}", "   ".repeat(depth), arrow, name)).color(p.fg)).frame(false));
        if resp.clicked() && depth > 0 {
            if expanded {
                self.expanded.remove(dir);
            } else {
                self.expanded.insert(dir.to_path_buf());
            }
        }
        if !expanded {
            return;
        }
        let entries = self.dir_cache.entry(dir.to_path_buf()).or_insert_with(|| read_dir_sorted(dir)).clone();
        for (path, is_dir) in entries {
            let n = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            if is_dir {
                self.tree(ui, &path, &n, depth + 1, open);
            } else {
                let active = self.docs.get(self.active).and_then(|d| d.path.as_ref()) == Some(&path);
                let col = if active { p.accent } else { mix(p.fg, p.sidebar_bg, 0.2) };
                let r = ui.add(egui::Button::new(RichText::new(format!("{}    {}", "   ".repeat(depth), n)).color(col)).frame(false));
                if r.clicked() {
                    *open = Some(path.clone());
                }
            }
        }
    }

    pub(super) fn tab_bar(&mut self, ui: &mut egui::Ui, group: usize) {
        let p = self.pal;
        let mut activate = None;
        let mut close = None;
        let radius = self.s.theme.radius;
        let style = self.s.theme.tab_style;
        let ufs = self.s.theme.ui_font_size;
        let group_sel = self.group_active.get(group).copied().flatten();
        egui::ScrollArea::horizontal()
            .id_salt(("tabs", group))
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    ui.add_space(6.0);
                    for (i, d) in self.docs.iter().enumerate().filter(|(_, d)| d.group == group) {
                        let sel = group_sel == Some(d.id);
                        let font = FontId::proportional(ufs);
                        let galley = ui.painter().layout_no_wrap(d.title.clone(), font.clone(), p.fg);
                        let w = (galley.size().x + 52.0).clamp(48.0, 180.0);
                        let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 35.0), egui::Sense::click());
                        let hovered = resp.hovered();
                        let fill = if sel { p.tab_active } else if hovered { p.tab_hover } else { p.tab_inactive };
                        let r = rect.with_min_y(rect.top() + 5.0);
                        let rr = (radius + 2.0).clamp(0.0, 30.0) as u8;
                        let cr = match style {
                            TabStyle::Rounded => egui::CornerRadius { nw: rr, ne: rr, sw: 0, se: 0 },
                            _ => egui::CornerRadius::ZERO,
                        };
                        if style != TabStyle::Underline {
                            ui.painter().rect_filled(r, cr, fill);
                        } else if sel || hovered {
                            ui.painter().rect_filled(r, cr, p.tab_hover);
                        }
                        if sel {
                            let bar = if style == TabStyle::Underline {
                                egui::Rect::from_min_max(egui::pos2(r.left(), r.bottom() - 2.0), r.right_bottom())
                            } else {
                                egui::Rect::from_min_max(r.left_top(), egui::pos2(r.right(), r.top() + 2.0))
                            };
                            ui.painter().rect_filled(bar, cr, p.accent);
                        }
                        let txt_col = if sel { p.fg } else { mix(p.fg, p.tab_inactive, 0.35) };
                        ui.painter().with_clip_rect(egui::Rect::from_min_max(r.left_top(), egui::pos2(r.right() - 30.0, r.bottom()))).text(egui::pos2(r.left() + 14.0, r.center().y), egui::Align2::LEFT_CENTER, &d.title, font, txt_col);
                        let cb = egui::Rect::from_center_size(egui::pos2(r.right() - 16.0, r.center().y), egui::vec2(16.0, 16.0));
                        let cresp = ui.interact(cb, egui::Id::new(("tabclose", d.id)), egui::Sense::click());
                        if cresp.hovered() {
                            ui.painter().rect_filled(cb, 4.0, mix(fill, p.fg, 0.15));
                        }
                        if d.dirty() && !cresp.hovered() {
                            ui.painter().circle_filled(cb.center(), 4.0, mix(p.fg, fill, 0.2));
                        } else if sel || hovered || cresp.hovered() {
                            ui.painter().text(cb.center(), egui::Align2::CENTER_CENTER, "×", FontId::proportional(15.0), txt_col);
                        }
                        if cresp.clicked() || resp.middle_clicked() {
                            close = Some(i);
                        } else if resp.clicked() {
                            activate = Some(i);
                        }
                        resp.on_hover_text(d.path.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "Not saved yet".into()));
                    }
                    if ui.add(egui::Button::new(RichText::new("+").size(16.0).color(p.muted)).frame(false)).on_hover_text("New File").clicked() {
                        activate = Some(usize::MAX);
                    }
                });
            });
        if let Some(i) = close {
            self.close_doc(i);
        } else if activate == Some(usize::MAX) {
            self.focus_group = group;
            self.run("new");
        } else if let Some(i) = activate {
            self.active = i;
            if let Some(d) = self.docs.get_mut(i) {
                d.view.focus = true;
            }
        }
    }

    // ------------------------------------------------------------------
    // split panes: View ▸ Layout / Groups

    /// Keep groups, their active files and the focused group consistent.
    pub(super) fn sync_groups(&mut self) {
        self.groups = self.groups.clamp(1, 16);
        self.cols = self.cols.clamp(1, self.groups);
        self.group_active.resize(self.groups, None);
        for d in self.docs.iter_mut() {
            if d.group >= self.groups {
                d.group = self.groups - 1;
            }
        }
        for g in 0..self.groups {
            let ok = self.group_active[g].map(|id| self.docs.iter().any(|d| d.id == id && d.group == g)).unwrap_or(false);
            if !ok {
                self.group_active[g] = self.docs.iter().rev().find(|d| d.group == g).map(|d| d.id);
            }
        }
        let cur = self.docs.get(self.active).map(|d| d.id);
        if cur != self.last_active_id {
            // the active file changed (tab click, Goto Anything, new file…): follow it
            if let Some(d) = self.docs.get(self.active) {
                self.focus_group = d.group.min(self.groups - 1);
                self.group_active[self.focus_group] = Some(d.id);
            }
            self.last_active_id = cur;
        }
        self.focus_group = self.focus_group.min(self.groups - 1);
    }

    pub(super) fn set_layout(&mut self, groups: usize, cols: usize) {
        self.groups = groups.max(1);
        self.cols = cols.clamp(1, self.groups);
        self.sync_groups();
    }

    pub(super) fn focus_group_to(&mut self, g: usize) {
        if g >= self.groups {
            return;
        }
        self.focus_group = g;
        if let Some(id) = self.group_active[g] {
            if let Some(i) = self.docs.iter().position(|d| d.id == id) {
                self.active = i;
                self.last_active_id = Some(id);
                self.docs[i].view.focus = true;
            }
        }
    }

    pub(super) fn move_file_to_group(&mut self, g: usize) {
        if g >= self.groups {
            return;
        }
        if let Some(d) = self.docs.get_mut(self.active) {
            d.group = g;
            let id = d.id;
            self.group_active[g] = Some(id);
            self.focus_group = g;
            self.last_active_id = Some(id);
            d.view.focus = true;
        }
        self.sync_groups();
    }

    pub(super) fn new_group(&mut self) {
        // a single row grows by one column (up to "Max Columns"); otherwise a new row starts
        let single_row = self.cols == self.groups;
        self.groups += 1;
        if single_row && self.groups <= self.max_columns {
            self.cols = self.groups;
        }
        self.sync_groups();
    }

    pub(super) fn close_group(&mut self) {
        if self.groups <= 1 {
            return;
        }
        let g = self.focus_group;
        // files of the closed group move to the neighbouring group
        let to = if g == 0 { 1 } else { g - 1 };
        for d in self.docs.iter_mut() {
            if d.group == g {
                d.group = to;
            } else if d.group > g {
                d.group -= 1;
            }
        }
        self.group_active.remove(g);
        self.groups -= 1;
        self.cols = self.cols.min(self.groups);
        self.focus_group = to.min(self.groups - 1);
        self.sync_groups();
        let fg = self.focus_group;
        self.focus_group_to(fg);
    }

    /// Keep split copies of a file (File ▸ Split View) showing the same text.
    pub(super) fn sync_clones(&mut self) {
        let mut ids: Vec<u64> = self.docs.iter().filter(|d| d.clone_id != 0).map(|d| d.clone_id).collect();
        ids.sort_unstable();
        ids.dedup();
        for cid in ids {
            let members: Vec<usize> = (0..self.docs.len()).filter(|&i| self.docs[i].clone_id == cid).collect();
            if members.len() < 2 {
                for &i in &members {
                    self.docs[i].clone_id = 0;
                }
                continue;
            }
            let last = self.clone_sync.get(&cid).copied();
            let hashes: Vec<u64> = members.iter().map(|&i| hash_text(&self.docs[i].text)).collect();
            let changed = members.iter().zip(hashes.iter()).find(|(_, h)| Some(**h) != last).map(|(i, _)| *i);
            if let Some(src) = changed {
                let text = self.docs[src].text.clone();
                let h = hash_text(&text);
                for &i in &members {
                    if i != src && self.docs[i].text != text {
                        self.docs[i].text = text.clone();
                    }
                }
                self.clone_sync.insert(cid, h);
            }
        }
    }

    pub(super) fn split_view(&mut self) {
        let Some(d) = self.docs.get(self.active) else { return };
        let (title, text, path, enc, le, syn) = (d.title.clone(), d.text.clone(), d.path.clone(), d.encoding, d.line_ending, d.syntax_override.clone());
        let cid = if d.clone_id != 0 { d.clone_id } else {
            let c = self.next_clone;
            self.next_clone += 1;
            c
        };
        let saved = (d.saved_hash, d.saved_text.clone());
        self.docs[self.active].clone_id = cid;
        if self.groups < 2 {
            self.set_layout(2, 2);
        }
        let target = (self.focus_group + 1) % self.groups;
        let id = self.new_doc(&title, text.clone(), path);
        if let Some(n) = self.docs.iter_mut().find(|x| x.id == id) {
            n.clone_id = cid;
            n.group = target;
            n.encoding = enc;
            n.line_ending = le;
            n.syntax_override = syn;
            n.saved_hash = saved.0;
            n.saved_text = saved.1;
        }
        self.clone_sync.insert(cid, hash_text(&text));
        self.focus_group = target;
        self.sync_groups();
        self.group_active[target] = Some(id);
    }

    pub(super) fn command_palette(&mut self, ctx: &egui::Context) {
        let Some(mode) = self.palette else { return };
        let p = self.pal;
        let mut chosen: Option<String> = None;
        let screen = ctx.content_rect();
        let w = 560.0f32.min(screen.width() - 40.0);
        let area_id = egui::Id::new("palette");
        let ufs = self.s.theme.ui_font_size;
        let radius = self.s.theme.radius;
        let area = egui::Area::new(area_id).order(egui::Order::Foreground).fixed_pos(egui::pos2(screen.center().x - w / 2.0, screen.top() + 70.0)).show(ctx, |ui| {
            egui::Frame::popup(ui.style()).fill(mix(p.sidebar_bg, p.bg, 0.4)).corner_radius(radius * 1.5).inner_margin(egui::Margin::same(10)).show(ui, |ui| {
                ui.set_width(w);
                let hint = match mode {
                    PaletteMode::Commands => "Type a command…",
                    PaletteMode::Files => "Goto anything: type a file name…",
                    PaletteMode::GotoLine => "Line number…",
                    PaletteMode::Syntax => "Set syntax…",
                    PaletteMode::Scheme => "Color scheme…",
                    PaletteMode::PasteHistory => "Paste from history…",
                    PaletteMode::Complete => "Complete…",
                    PaletteMode::Symbols => "Goto symbol…",
                    PaletteMode::ProjectSymbols => "Goto symbol in project…",
                    PaletteMode::Snippets => "Insert snippet…",
                    PaletteMode::BuildWith => "Build with…",
                    PaletteMode::PackageFiles => "View package file…",
                    PaletteMode::Projects => "Switch project…",
                    PaletteMode::Fonts => "Choose the editor font…",
                };
                if mode == PaletteMode::ProjectSymbols {
                    if let Some(rx) = &self.project_symbols_rx {
                        if let Ok(v) = rx.try_recv() {
                            self.pal_items = v;
                            self.project_symbols_rx = None;
                        } else {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label(RichText::new("Indexing symbols in your folders…").color(p.muted));
                            });
                            ui.ctx().request_repaint_after(std::time::Duration::from_millis(150));
                        }
                    }
                }
                let r = ui.add(egui::TextEdit::singleline(&mut self.pal_query).hint_text(hint).desired_width(f32::INFINITY).font(FontId::proportional(ufs + 2.0)));
                if self.pal_focus {
                    r.request_focus();
                    self.pal_focus = false;
                }
                if r.changed() {
                    self.pal_sel = 0;
                }
                let items: Vec<(String, String, String)> = match mode {
                    PaletteMode::Commands => self.commands().into_iter().map(|(l, id, sc)| (l.to_string(), id.to_string(), sc.to_string())).collect(),
                    PaletteMode::Files => {
                        let mut v: Vec<(String, String, String)> = self.docs.iter().map(|d| (d.title.clone(), format!("doc:{}", d.id), "open".into())).collect();
                        if let Some((root, files)) = &self.file_index {
                            for f in files.iter() {
                                let rel = f.strip_prefix(root).unwrap_or(f).display().to_string();
                                v.push((rel, format!("file:{}", f.display()), String::new()));
                            }
                        }
                        v
                    }
                    PaletteMode::GotoLine => vec![(format!("Go to line {}", self.pal_query.trim()), "line".into(), String::new())],
                    PaletteMode::Syntax => {
                        let mut v: Vec<(String, String, String)> =
                            self.syntax_set.syntaxes().iter().filter(|s| !s.hidden).map(|s| (s.name.clone(), format!("syn:{}", s.name), String::new())).collect();
                        v.sort();
                        v.dedup_by(|a, b| a.0 == b.0);
                        v
                    }
                    PaletteMode::Scheme => self.schemes.iter().enumerate().map(|(i, s)| (s.name.clone(), format!("scheme:{i}"), s.source.clone())).collect(),
                    _ => self.pal_items.clone(),
                };
                let q = self.pal_query.to_lowercase();
                let mut filtered: Vec<(i32, (String, String, String))> = items
                    .into_iter()
                    .filter_map(|it| {
                        if mode == PaletteMode::GotoLine {
                            return Some((0, it));
                        }
                        fuzzy(&it.0.to_lowercase(), &q).map(|s| (s, it))
                    })
                    .collect();
                if !q.is_empty() {
                    filtered.sort_by_key(|(s, _)| -*s);
                }
                filtered.truncate(200);
                if ui.input(|i| i.key_pressed(Key::ArrowDown)) {
                    self.pal_sel = (self.pal_sel + 1).min(filtered.len().saturating_sub(1));
                }
                if ui.input(|i| i.key_pressed(Key::ArrowUp)) {
                    self.pal_sel = self.pal_sel.saturating_sub(1);
                }
                if ui.input(|i| i.key_pressed(Key::Enter)) {
                    if let Some((_, it)) = filtered.get(self.pal_sel) {
                        chosen = Some(it.1.clone());
                    }
                }
                ui.add_space(6.0);
                egui::ScrollArea::vertical().max_height(380.0).show(ui, |ui| {
                    for (k, (_, (label, id, extra))) in filtered.iter().enumerate() {
                        let sel = k == self.pal_sel;
                        let resp = ui
                            .horizontal(|ui| {
                                let r = ui.add(egui::Button::selectable(sel, RichText::new(label).color(p.fg)).frame_when_inactive(false).min_size(egui::vec2(w - 140.0, 24.0)));
                                if !extra.is_empty() {
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        ui.label(RichText::new(extra).small().color(p.muted));
                                    });
                                }
                                r
                            })
                            .inner;
                        if sel {
                            resp.scroll_to_me(None);
                        }
                        if resp.clicked() {
                            chosen = Some(id.clone());
                        }
                    }
                    if filtered.is_empty() {
                        ui.weak("No matches");
                    }
                });
            });
        });
        let clicked_outside = ctx.input(|i| i.pointer.any_pressed()) && ctx.input(|i| i.pointer.interact_pos()).map(|pp| !area.response.rect.contains(pp)).unwrap_or(false);
        if let Some(id) = chosen {
            self.palette = None;
            if let Some(d) = id.strip_prefix("doc:") {
                let n: u64 = d.parse().unwrap_or(0);
                if let Some(i) = self.docs.iter().position(|x| x.id == n) {
                    self.active = i;
                }
            } else if let Some(f) = id.strip_prefix("file:") {
                self.open_path(Path::new(f));
            } else if let Some(s) = id.strip_prefix("syn:") {
                if let Some(d) = self.docs.get_mut(self.active) {
                    d.syntax_override = Some(s.to_string());
                    d.view.hl.invalidate();
                }
            } else if let Some(s) = id.strip_prefix("scheme:") {
                self.set_scheme(s.parse().unwrap_or(0));
            } else if id == "line" {
                if let Ok(n) = self.pal_query.trim().trim_start_matches(':').parse::<usize>() {
                    self.push_jump();
                    if let Some(d) = self.docs.get_mut(self.active) {
                        let ch: usize = d.text.split('\n').take(n.saturating_sub(1)).map(|l| l.chars().count() + 1).sum();
                        d.view.select = Some((ch, ch));
                    }
                }
            } else if self.palette_choose(&id) {
            } else {
                self.dispatch(&id, ctx);
            }
            if let Some(d) = self.docs.get_mut(self.active) {
                d.view.focus = true;
            }
        } else if clicked_outside {
            self.palette = None;
        }
    }

    pub(super) fn windows(&mut self, ctx: &egui::Context) {
        let p = self.pal;
        // Theme & appearance
        let mut open = self.win_theme;
        let mut pick = None;
        egui::Window::new("Theme & Appearance").open(&mut open).resizable(false).default_width(400.0).show(ctx, |ui| {
            ui.label(RichText::new("The whole window follows the color scheme. Changes apply instantly.").color(p.muted));
            ui.add_space(6.0);
            egui::Grid::new("theme_grid").num_columns(2).spacing([14.0, 8.0]).show(ui, |ui| {
                ui.label("Color scheme");
                egui::ComboBox::from_id_salt("scheme_combo").selected_text(self.schemes[self.scheme_idx].name.clone()).width(220.0).show_ui(ui, |ui| {
                    for (i, s) in self.schemes.iter().enumerate() {
                        if ui.selectable_label(i == self.scheme_idx, &s.name).on_hover_text(&s.source).clicked() {
                            pick = Some(i);
                        }
                    }
                });
                ui.end_row();
                ui.label("Accent color");
                ui.horizontal(|ui| {
                    let mut use_custom = self.s.theme.accent_override.is_some();
                    if ui.checkbox(&mut use_custom, "custom").changed() {
                        self.s.theme.accent_override = if use_custom { Some([p.accent.r(), p.accent.g(), p.accent.b()]) } else { None };
                    }
                    if let Some(rgb) = self.s.theme.accent_override.as_mut() {
                        ui.color_edit_button_srgb(rgb);
                    }
                    for (n, c) in [("Sky", [0x5a, 0xb0, 0xf6]), ("Violet", [0xa7, 0x8b, 0xfa]), ("Mint", [0x34, 0xd3, 0x99]), ("Amber", [0xff, 0xb4, 0x54]), ("Rose", [0xf4, 0x72, 0xb6])] {
                        let (r, resp) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::click());
                        ui.painter().circle_filled(r.center(), 7.0, Color32::from_rgb(c[0], c[1], c[2]));
                        if resp.on_hover_text(n).clicked() {
                            self.s.theme.accent_override = Some(c);
                        }
                    }
                });
                ui.end_row();
                ui.label("Tabs");
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.s.theme.tab_style, TabStyle::Rounded, "Rounded");
                    ui.selectable_value(&mut self.s.theme.tab_style, TabStyle::Square, "Square");
                    ui.selectable_value(&mut self.s.theme.tab_style, TabStyle::Underline, "Underline");
                });
                ui.end_row();
                ui.label("Corner roundness");
                ui.add(egui::Slider::new(&mut self.s.theme.radius, 0.0..=14.0));
                ui.end_row();
                ui.label("Spacing");
                ui.add(egui::Slider::new(&mut self.s.theme.density, 0.7..=1.4).text("compact ↔ roomy"));
                ui.end_row();
                ui.label("Side bar contrast");
                ui.add(egui::Slider::new(&mut self.s.theme.sidebar_contrast, 0.0..=1.0));
                ui.end_row();
                ui.label("Tint panels from scheme");
                ui.checkbox(&mut self.s.theme.tinted_surfaces, "");
                ui.end_row();
                ui.label("Interface font size");
                ui.add(egui::Slider::new(&mut self.s.theme.ui_font_size, 10.0..=20.0));
                ui.end_row();
                ui.label("Editor font size");
                ui.add(egui::Slider::new(&mut self.s.theme.editor_font_size, 8.0..=32.0));
                ui.end_row();
                ui.label("Line spacing");
                ui.add(egui::Slider::new(&mut self.s.theme.line_padding, 0.0..=10.0));
                ui.end_row();
                ui.label("Tab size");
                ui.add(egui::Slider::new(&mut self.s.tab_size, 1..=8));
                ui.end_row();
                ui.label("Colorizer style");
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.s.color_style, ColorStyle::Fill, "Fill");
                    ui.selectable_value(&mut self.s.color_style, ColorStyle::Underline, "Underline");
                    ui.selectable_value(&mut self.s.color_style, ColorStyle::Outline, "Outline");
                });
                ui.end_row();
                ui.label("Named colors (red, teal…)");
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.s.named_colors, NamedMode::Off, "Off");
                    ui.selectable_value(&mut self.s.named_colors, NamedMode::StyleFiles, "Style files");
                    ui.selectable_value(&mut self.s.named_colors, NamedMode::Everywhere, "Everywhere");
                });
                ui.end_row();
                ui.label("Gutter color swatches");
                ui.checkbox(&mut self.s.gutter_swatches, "");
                ui.end_row();
            });
            ui.add_space(6.0);
            if ui.button("Reset appearance").clicked() {
                self.s.theme = ThemeOptions::default();
            }
        });
        self.win_theme = open;
        if let Some(i) = pick {
            self.set_scheme(i);
        }

        // Sublime packages
        let mut open = self.win_packages;
        let mut rescan = false;
        egui::Window::new("Sublime Text Packages").open(&mut open).default_size([640.0, 480.0]).show(ctx, |ui| {
            ui.label(RichText::new("Languages and color schemes are read straight from Sublime Text's package files. Python plugins (LSP, Emmet…) need Sublime's own plugin host, so they are listed but not run; language servers are handled by this editor's built-in client instead.").color(p.muted));
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.s.use_sublime_packages, "Use Sublime packages");
                if ui.button("Rescan").clicked() {
                    rescan = true;
                }
                if ui.button("Add package folder…").clicked() {
                    if let Some(f) = rfd::FileDialog::new().pick_folder() {
                        self.s.extra_package_dirs.push(f.display().to_string());
                        rescan = true;
                    }
                }
            });
            if self.scan_rx.is_some() {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Scanning…");
                });
            }
            if let Some(r) = &self.pkg_report {
                ui.label(format!(
                    "{} languages loaded, {} could not be read, {} color schemes, {} Python plugin files (not run)",
                    r.total_loaded(),
                    r.total_failed(),
                    r.packages.iter().map(|p| p.schemes.len()).sum::<usize>(),
                    r.total_python()
                ));
                if let Some(e) = &r.error {
                    ui.colored_label(Color32::from_rgb(240, 120, 100), e);
                }
                for (f, ok) in &r.folders {
                    ui.small(format!("{} {}", if *ok { "✔" } else { "–" }, f));
                }
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for pk in &r.packages {
                        let title = format!(
                            "{}   ·  {} syntax{}{}{}",
                            pk.name,
                            pk.syntaxes_loaded.len(),
                            if pk.syntaxes_failed.is_empty() { String::new() } else { format!(" (+{} unsupported)", pk.syntaxes_failed.len()) },
                            if pk.schemes.is_empty() { String::new() } else { format!("  · {} scheme(s)", pk.schemes.len()) },
                            if pk.python_files > 0 { format!("  · {} .py", pk.python_files) } else { String::new() },
                        );
                        egui::CollapsingHeader::new(title).id_salt(&pk.location).show(ui, |ui| {
                            ui.small(&pk.location);
                            for s in &pk.syntaxes_loaded {
                                ui.label(format!("✔ {s}"));
                            }
                            for (s, e) in &pk.syntaxes_failed {
                                ui.label(RichText::new(format!("✖ {s}: {e}")).color(p.muted));
                            }
                            for s in &pk.schemes {
                                ui.label(format!("● {s}"));
                            }
                            if pk.tm_languages > 0 {
                                ui.label(RichText::new(format!("{} old-style .tmLanguage file(s) skipped", pk.tm_languages)).color(p.muted));
                            }
                            if pk.snippets + pk.completions > 0 {
                                ui.label(RichText::new(format!("{} snippet / completion file(s) (not used yet)", pk.snippets + pk.completions)).color(p.muted));
                            }
                            if pk.lsp_settings {
                                ui.label("LSP settings found — used by Tools ▸ Language Servers");
                            }
                        });
                    }
                });
            } else if self.scan_rx.is_none() {
                ui.label("Not scanned yet.");
            }
        });
        self.win_packages = open;
        if rescan {
            self.start_scan();
        }

        // LSP
        let mut open = self.win_lsp;
        let mut restart = false;
        egui::Window::new("Language Servers (LSP)").open(&mut open).default_size([580.0, 400.0]).show(ctx, |ui| {
            ui.label(RichText::new("Install a server (for example rust-analyzer, pyright, typescript-language-server, clangd) and open a saved file of that language. Clients you configured for Sublime's LSP package are picked up too. Errors show as squiggles and in the Problems panel; rest the mouse on code for info; F12 goes to a definition.").color(p.muted));
            ui.checkbox(&mut self.s.lsp_enabled, "Enable language servers");
            ui.add_space(4.0);
            egui::ScrollArea::vertical().show(ui, |ui| {
                egui::Grid::new("lsp_grid").striped(true).num_columns(4).spacing([12.0, 6.0]).show(ui, |ui| {
                    ui.strong("Server");
                    ui.strong("Files");
                    ui.strong("Status");
                    ui.strong("From");
                    ui.end_row();
                    for s in &self.servers {
                        ui.label(&s.name);
                        ui.label(s.exts.join(" "));
                        let st = if let Some(c) = self.clients.get(&s.name) {
                            RichText::new(format!("running · {}", c.status)).color(Color32::from_rgb(110, 200, 120))
                        } else if let Some(e) = self.lsp_errors.get(&s.name) {
                            RichText::new(e).color(Color32::from_rgb(240, 120, 100))
                        } else if s.found.is_some() {
                            RichText::new("installed").color(p.fg)
                        } else {
                            RichText::new("not installed").color(p.muted)
                        };
                        ui.label(st);
                        ui.label(RichText::new(&s.source).small());
                        ui.end_row();
                    }
                });
            });
            ui.add_space(6.0);
            if ui.button("Search again & restart").clicked() {
                restart = true;
            }
        });
        self.win_lsp = open;
        if restart {
            self.run("lsp_restart");
        }

        // About
        let mut open = self.win_about;
        egui::Window::new(format!("About {APP_NAME}")).open(&mut open).collapsible(false).resizable(false).show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                let (r, _) = ui.allocate_exact_size(egui::vec2(64.0, 64.0), egui::Sense::hover());
                logo(ui.painter(), r, p.accent, p.bg);
                ui.heading(APP_NAME);
                ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
                ui.label("A fast text editor made for Bojan.");
                ui.add_space(4.0);
                ui.label(RichText::new("Colorizer · live Markdown preview · Sublime packages · language servers").color(p.muted));
            });
        });
        self.win_about = open;

        // Colors list
        let mut open = self.win_colors;
        let mut jump: Option<(usize, usize)> = None;
        let mut edit: Option<ColorMatch> = None;
        if let Some(d) = self.docs.get(self.active) {
            let doc_text = &d.text;
            egui::Window::new("Color codes in this file").open(&mut open).default_size([380.0, 420.0]).show(ctx, |ui| {
                ui.label(RichText::new(format!("{} color codes in {} — click a swatch to edit, click the code to jump", d.view.colors.list.len(), d.title)).color(p.muted));
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for c in &d.view.colors.list {
                        let Some(code) = doc_text.get(c.start..c.end) else { continue };
                        ui.horizontal(|ui| {
                            let (r, resp) = ui.allocate_exact_size(egui::vec2(22.0, 22.0), egui::Sense::click());
                            editor::checker(ui.painter(), r);
                            ui.painter().rect_filled(r, 5.0, c.color.to_color32());
                            if resp.on_hover_text("Edit color").clicked() {
                                edit = Some(c.clone());
                            }
                            let line = doc_text[..c.start].matches('\n').count() + 1;
                            if ui.add(egui::Button::new(RichText::new(code).monospace()).frame(false)).clicked() {
                                let a = byte_to_char(doc_text, c.start);
                                jump = Some((a, a + code.chars().count()));
                            }
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(RichText::new(format!("line {line}")).small().color(p.muted));
                                ui.label(RichText::new(c.color.to_hex()).small().monospace().color(p.muted));
                            });
                        });
                    }
                });
            });
        }
        self.win_colors = open;
        if let Some(j) = jump {
            if let Some(d) = self.docs.get_mut(self.active) {
                d.view.select = Some(j);
            }
        }
        if let Some(m) = edit {
            let id = self.docs[self.active].id;
            self.begin_color_edit(id, m);
        }

        // Color editor
        let mut keep = true;
        if let Some(ce) = self.color_edit.as_mut() {
            let mut changed = false;
            let mut apply_fmt: Option<String> = None;
            egui::Window::new("Edit color")
                .open(&mut keep)
                .collapsible(false)
                .resizable(false)
                .default_pos(ctx.content_rect().center() - egui::vec2(150.0, 150.0))
                .show(ctx, |ui| {
                    ui.label(RichText::new(&ce.original).monospace().color(p.muted));
                    if egui::color_picker::color_picker_color32(ui, &mut ce.color, egui::color_picker::Alpha::OnlyBlend) {
                        changed = true;
                    }
                    let [r, g, b, a] = ce.color.to_srgba_unmultiplied();
                    let rgba = Rgba::from_u8(r, g, b, a);
                    ui.separator();
                    ui.label("Write it as:");
                    ui.horizontal_wrapped(|ui| {
                        for f in [rgba.to_hex(), rgba.to_rgb_css(), rgba.to_hsl_css()] {
                            if ui.button(RichText::new(&f).monospace()).clicked() {
                                apply_fmt = Some(f);
                            }
                        }
                    });
                });
            if changed || apply_fmt.is_some() {
                let [r, g, b, a] = ce.color.to_srgba_unmultiplied();
                let rgba = Rgba::from_u8(r, g, b, a);
                let new_text = apply_fmt.unwrap_or_else(|| ce.m.format_like(rgba, &ce.original));
                if let Some(d) = self.docs.iter_mut().find(|d| d.id == ce.doc_id) {
                    if ce.end <= d.text.len() && d.text.is_char_boundary(ce.start) && d.text.is_char_boundary(ce.end) && d.text[ce.start..ce.end] == ce.original {
                        d.text.replace_range(ce.start..ce.end, &new_text);
                        ce.end = ce.start + new_text.len();
                        ce.original = new_text;
                    }
                }
            }
        }
        if !keep {
            self.color_edit = None;
        }

        // LSP hover popup
        if let Some((t, pos)) = &self.hover_popup {
            let efs = self.s.theme.editor_font_size;
            egui::Area::new(egui::Id::new("lsp_hover")).order(egui::Order::Tooltip).fixed_pos(*pos + egui::vec2(12.0, 18.0)).show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_max_width(520.0);
                    egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                        ui.label(RichText::new(t.trim()).monospace().size(efs * 0.9));
                    });
                });
            });
            let moved = ctx.input(|i| i.pointer.delta().length() > 6.0 || i.events.iter().any(|e| matches!(e, egui::Event::Text(_) | egui::Event::Key { .. })));
            if moved {
                self.hover_popup = None;
            }
        }
    }

    pub(super) fn problems_panel(&mut self, ui: &mut egui::Ui) {
        let p = self.pal;
        let mut jump: Option<(PathBuf, usize, usize)> = None;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let mut any = false;
            for d in &self.docs {
                let Some(path) = &d.path else { continue };
                let Some(list) = self.diags.get(&lsp::norm_key(path)) else { continue };
                for di in list {
                    any = true;
                    let (icon, col) = match di.severity {
                        1 => ("✖", Color32::from_rgb(0xf0, 0x5d, 0x5d)),
                        2 => ("⚠", Color32::from_rgb(0xf5, 0xb9, 0x42)),
                        _ => ("ℹ", p.accent),
                    };
                    let text = format!("{icon}  {}:{}:{}  {}  {}", d.title, di.line + 1, di.col + 1, di.message.lines().next().unwrap_or(""), di.source);
                    if ui.add(egui::Button::new(RichText::new(text).color(col)).frame(false)).clicked() {
                        jump = Some((path.clone(), di.line, di.col));
                    }
                }
            }
            if !any {
                ui.label(RichText::new("No problems reported. Language servers report errors and warnings here.").color(p.muted));
            }
        });
        if let Some((path, line, col)) = jump {
            self.open_path(&path);
            if let Some(d) = self.docs.get_mut(self.active) {
                let c = line_col_to_char(&d.text, line, col);
                d.view.select = Some((c, c));
            }
        }
    }

    /// Draw all split panes: each has its own tab bar and shows its own file.
    pub(super) fn panes(&mut self, ui: &mut egui::Ui) {
        let p = self.pal;
        let full = ui.available_rect_before_wrap();
        let rows = (self.groups + self.cols - 1) / self.cols;
        let gap = 1.0;
        let mut press_group: Option<usize> = None;
        let pressed_at = ui.input(|i| if i.pointer.any_pressed() { i.pointer.interact_pos() } else { None });
        for g in 0..self.groups {
            let (r, c) = (g / self.cols, g % self.cols);
            let in_row = (self.groups - r * self.cols).min(self.cols);
            let (x0, x1) = (edge(c, in_row), edge(c + 1, in_row));
            let (y0, y1) = (edge(r, rows), edge(r + 1, rows));
            let rect = egui::Rect::from_min_max(
                egui::pos2(full.left() + x0 * full.width() + if c > 0 { gap / 2.0 } else { 0.0 }, full.top() + y0 * full.height() + if r > 0 { gap / 2.0 } else { 0.0 }),
                egui::pos2(full.left() + x1 * full.width() - if c + 1 < in_row { gap / 2.0 } else { 0.0 }, full.top() + y1 * full.height() - if r + 1 < rows { gap / 2.0 } else { 0.0 }),
            );
            if pressed_at.map(|pp| rect.contains(pp)).unwrap_or(false) {
                press_group = Some(g);
            }
            let focused = g == self.focus_group && self.groups > 1;
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::top_down(egui::Align::Min)));
            child.set_clip_rect(rect);
            if self.s.show_tabs {
                egui::Panel::top(("pane_tabs", g)).exact_size(35.0).frame(egui::Frame::new().fill(p.tabbar_bg)).show(&mut child, |ui| {
                    ui.horizontal(|ui| {
                        if g == 0 && !self.s.show_menu && ui.add(egui::Button::new(RichText::new("☰").size(16.0).color(p.muted)).frame(false)).on_hover_text("Show the menu again").clicked() {
                            self.s.show_menu = true;
                        }
                        self.tab_bar(ui, g);
                    });
                });
            }
            let id = self.group_active.get(g).copied().flatten();
            egui::CentralPanel::no_frame().show(&mut child, |ui| match id.and_then(|id| self.docs.iter().position(|d| d.id == id)) {
                Some(idx) => self.editor_area(ui, idx, g == self.focus_group),
                None => {
                    ui.painter().rect_filled(ui.available_rect_before_wrap(), 0.0, p.bg);
                    ui.centered_and_justified(|ui| {
                        ui.label(RichText::new("Ctrl+N for a new file · Ctrl+O to open · Ctrl+Shift+P for commands").color(p.muted));
                    });
                }
            });
            if focused {
                // a thin accent line marks the focused pane
                ui.painter().hline(rect.x_range(), rect.top() + 0.5, egui::Stroke::new(2.0, p.accent));
            }
            if self.groups > 1 && c + 1 < in_row {
                ui.painter().vline(full.left() + x1 * full.width(), rect.y_range(), egui::Stroke::new(1.0, p.border));
            }
        }
        if self.groups > 1 {
            for r in 1..rows {
                let y = full.top() + edge(r, rows) * full.height();
                ui.painter().hline(full.x_range(), y, egui::Stroke::new(1.0, p.border));
            }
        }
        if let Some(g) = press_group {
            if g != self.focus_group {
                self.focus_group_to(g);
                // an empty pane can be focused too
                self.focus_group = g;
            }
        }
        ui.allocate_rect(full, egui::Sense::hover());
    }

    pub(super) fn editor_area(&mut self, ui: &mut egui::Ui, idx: usize, _focused_group: bool) {
        let p = self.pal;
        let syntax_name = self.syntax_name_for(&self.docs[idx]);
        let named = self.named_colors_for(&self.docs[idx]);
        let md_mode = self.docs[idx].md_mode;
        let scheme_name = self.scheme().name.clone();
        let font = FontId::monospace(self.s.theme.editor_font_size);
        let row_h = ui.ctx().fonts_mut(|f| f.row_height(&font));
        let line_height = row_h + self.s.theme.line_padding;
        let diag_key = self.docs[idx].path.as_ref().map(|p| lsp::norm_key(p));

        // markdown preview pane
        // in a narrow pane the split preview would squeeze the text: show the text only
        if md_mode != MdMode::Off && !(md_mode == MdMode::Split && ui.available_width() < 520.0) {
            let full_w = ui.available_width();
            let mut want_browser = false;
            let mut toggle_full = false;
            let panel = egui::Panel::right(egui::Id::new(("mdprev", self.docs[idx].id)))
                .resizable(md_mode == MdMode::Split)
                .default_size(full_w * 0.5)
                .min_size(120.0_f32.min(full_w * 0.3))
                .max_size(full_w * 0.6)
                .frame(egui::Frame::new().fill(if p.dark { mix(p.bg, Color32::BLACK, 0.12) } else { Color32::WHITE }).inner_margin(egui::Margin { left: 22, right: 18, top: 10, bottom: 8 }));
            let panel = if md_mode == MdMode::Preview { panel.exact_size(full_w) } else { panel };
            let ss = self.syntax_set.clone();
            let theme_ref = self.schemes[self.scheme_idx].theme.clone();
            let code_cache = &mut self.code_cache;
            let efs = self.s.theme.editor_font_size;
            let fs = self.s.theme.ui_font_size + 2.0;
            let sync = self.s.md_sync_scroll && md_mode == MdMode::Split;
            let doc = &mut self.docs[idx];
            let target = if sync && doc.view.content_h > doc.view.view_h {
                let frac = (doc.view.scroll_y / (doc.view.content_h - doc.view.view_h)).clamp(0.0, 1.0);
                Some(frac * (doc.preview_content_h - doc.preview_view_h).max(0.0))
            } else {
                None
            };
            panel.show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("PREVIEW").small().strong().color(p.muted));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Open in browser ↗").clicked() {
                            want_browser = true;
                        }
                        if ui.small_button(if md_mode == MdMode::Preview { "Split" } else { "Full" }).clicked() {
                            toggle_full = true;
                        }
                    });
                });
                ui.add_space(4.0);
                let mut sa = egui::ScrollArea::vertical().id_salt(("mdscroll", doc.id)).auto_shrink([false, false]);
                if let Some(t) = target {
                    if (t - doc.preview_scroll).abs() > 1.0 {
                        sa = sa.vertical_scroll_offset(t);
                    }
                }
                let st = MdStyle { fg: p.fg, muted: p.muted, accent: p.accent, code_bg: p.code_bg, border: p.border, font_size: fs };
                let out = sa.show(ui, |ui| {
                    ui.set_max_width(ui.available_width().min(860.0));
                    let mut code_hl = |ui: &mut egui::Ui, lang: &str, code: &str| {
                        let key = (hash_text(code), lang.to_string());
                        let job = code_cache
                            .entry(key)
                            .or_insert_with(|| editor::highlight_snippet(code, lang, &ss, &theme_ref, FontId::monospace(efs * 0.95), p.fg))
                            .clone();
                        ui.label(job);
                    };
                    doc.md.show(ui, &doc.text, &st, &mut code_hl);
                });
                doc.preview_scroll = out.state.clamped_offset().y;
                doc.preview_content_h = out.content_size.y;
                doc.preview_view_h = out.inner_rect.height();
            });
            if want_browser {
                self.open_browser_preview();
            }
            if toggle_full {
                let d = &mut self.docs[idx];
                d.md_mode = if d.md_mode == MdMode::Preview { MdMode::Split } else { MdMode::Preview };
            }
            if md_mode == MdMode::Preview {
                return;
            }
        }

        self.modified_range(idx);
        let syntax = highlight::syntax_by_name(&self.syntax_set, &syntax_name).unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());
        let spec = self.find_spec();
        let find_q = if self.find.open && self.find.highlight && !self.find.query.is_empty() && !self.fif.open { Some((spec.0.as_str(), spec.1, spec.2)) } else { None };
        let bookmarks = self.docs[idx].bookmarks.clone();
        let rulers = self.s.rulers.clone();
        let cfg = ViewCfg {
            rulers: &rulers,
            wrap_column: self.s.wrap_column,
            bookmarks: &bookmarks,
            modified: self.docs[idx].mod_cache.1,
            ss: &self.syntax_set,
            syntax,
            theme: &self.schemes[self.scheme_idx].theme,
            hl_key: format!("{}|{}|{}", syntax_name, scheme_name, self.set_gen),
            pal: &p,
            font,
            line_height,
            wrap: self.s.word_wrap,
            colorizer: self.s.colorizer,
            color_style: self.s.color_style,
            named_colors: named,
            gutter_swatches: self.s.gutter_swatches && self.s.colorizer,
            find: find_q,
            highlight_line: self.s.highlight_line,
            line_numbers: self.s.line_numbers,
            minimap: self.s.show_minimap,
            tab_size: self.s.tab_size.max(1),
            diags: diag_key.as_ref().and_then(|k| self.diags.get(k)),
        };
        let doc = &mut self.docs[idx];
        let id = egui::Id::new(("doc", doc.id));
        let out = editor::editor_view(ui, id, &mut doc.text, &mut doc.view, &cfg);
        let doc_id = doc.id;
        if let Some(m) = out.color_edit {
            self.begin_color_edit(doc_id, m);
        }
        // LSP hover after the pointer rests on code
        if let Some(ch) = out.hover_char {
            let now = ui.input(|i| i.time);
            match self.hover_since {
                Some((d, c, t)) if d == doc_id && c == ch => {
                    if now - t > 0.7 && self.hover_sent != Some((doc_id, ch)) {
                        self.hover_sent = Some((doc_id, ch));
                        let d = &self.docs[idx];
                        if let (Some(path), Some(spec)) = (d.path.clone(), self.spec_for(&d.ext())) {
                            let (line, col) = Self::lsp_position(d, ch);
                            if let Some(c) = self.clients.get_mut(&spec.name) {
                                c.hover(&path, line, col);
                                let pos = ui.input(|i| i.pointer.hover_pos()).unwrap_or_default();
                                ui.ctx().data_mut(|dd| dd.insert_temp(egui::Id::new("hover_pos"), pos));
                            }
                        }
                    }
                }
                _ => self.hover_since = Some((doc_id, ch, now)),
            }
        }
        if let Some((t, pos)) = self.hover_popup.take() {
            let pos = if pos == egui::Pos2::ZERO { ui.ctx().data(|d| d.get_temp::<egui::Pos2>(egui::Id::new("hover_pos"))).unwrap_or_default() } else { pos };
            self.hover_popup = Some((t, pos));
        }
    }
}
