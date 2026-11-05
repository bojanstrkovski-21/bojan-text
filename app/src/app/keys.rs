//! Keyboard shortcuts, following the original's Windows key map
//! (Default (Windows).sublime-keymap), including Ctrl+K chords.

use super::features::BottomPanel;
use super::*;

impl App {
    pub(super) fn shortcuts(&mut self, ctx: &egui::Context) {
        let sc = |m: Modifiers, k: Key| KeyboardShortcut::new(m, k);
        let n = Modifiers::NONE;
        let c = Modifiers::COMMAND;
        let s = Modifiers::SHIFT;
        let a = Modifiers::ALT;
        let cs = Modifiers::COMMAND | Modifiers::SHIFT;
        let ca = Modifiers::COMMAND | Modifiers::ALT;
        let as_ = Modifiers::ALT | Modifiers::SHIFT;
        let cas = Modifiers::COMMAND | Modifiers::ALT | Modifiers::SHIFT;
        let editor_focused = self.editor_focus_id().map(|id| ctx.memory(|m| m.has_focus(id))).unwrap_or(false);

        // second key of a Ctrl+K chord
        let mut hits: Vec<String> = Vec::new();
        if let Some(t) = self.chord_k {
            if t.elapsed().as_secs_f32() > 2.0 {
                self.chord_k = None;
            } else {
                let chord: &[(Key, &str)] = &[
                    (Key::B, "toggle_sidebar"),
                    (Key::U, "upper"),
                    (Key::L, "lower"),
                    (Key::K, "del_to_end"),
                    (Key::Backspace, "del_to_start"),
                    (Key::Space, "set_mark"),
                    (Key::A, "select_to_mark"),
                    (Key::W, "delete_to_mark"),
                    (Key::X, "swap_with_mark"),
                    (Key::G, "clear_mark"),
                    (Key::Y, "yank"),
                    (Key::C, "scroll_to_sel"),
                    (Key::Z, "revert_mod"),
                    (Key::V, "paste_history"),
                    (Key::D, "quick_add_next"),
                ];
                ctx.input_mut(|i| {
                    for (k, id) in chord {
                        if i.consume_shortcut(&sc(c, *k)) || i.consume_shortcut(&sc(n, *k)) {
                            hits.push(id.to_string());
                            self.chord_k = None;
                        }
                    }
                    // any other key press ends the chord
                    if self.chord_k.is_some() && i.events.iter().any(|e| matches!(e, egui::Event::Key { pressed: true, key, .. } if *key != Key::K)) {
                        self.chord_k = None;
                    }
                });
            }
        }

        let list: Vec<(KeyboardShortcut, &str)> = vec![
            // File
            (sc(cs, Key::P), "palette_cmd"),
            (sc(cs, Key::S), "save_as"),
            (sc(cs, Key::N), "new_window"),
            (sc(cs, Key::W), "close_window"),
            (sc(c, Key::N), "new"),
            (sc(c, Key::O), "open"),
            (sc(c, Key::S), "save"),
            (sc(c, Key::W), "close"),
            // Edit
            (sc(cs, Key::V), "paste_indent"),
            (sc(c, Key::CloseBracket), "indent"),
            (sc(c, Key::OpenBracket), "unindent"),
            (sc(cs, Key::ArrowUp), "swap_up"),
            (sc(cs, Key::ArrowDown), "swap_down"),
            (sc(cs, Key::D), "dup_line"),
            (sc(cs, Key::K), "del_line"),
            (sc(c, Key::J), "join_lines"),
            (sc(cs, Key::Slash), "block_comment"),
            (sc(cs, Key::Questionmark), "block_comment"),
            (sc(c, Key::Slash), "comment"),
            (sc(cs, Key::Enter), "line_before"),
            (sc(c, Key::Enter), "line_after"),
            (sc(c, Key::Delete), "del_word_fwd"),
            (sc(c, Key::T), "transpose"),
            (sc(a, Key::Period), "close_tag"),
            (sc(cs, Key::A), "expand_tag"),
            (sc(as_, Key::W), "wrap_tag"),
            (sc(a, Key::Q), "wrap_at:0"),
            (sc(c, Key::Space), "complete"),
            (sc(c, Key::F9), "sort_case"),
            (sc(n, Key::F9), "sort_lines"),
            // Selection
            (sc(cs, Key::Space), "expand_sel"),
            (sc(c, Key::L), "expand_line"),
            (sc(c, Key::D), "quick_add_next"),
            (sc(cs, Key::M), "expand_brackets"),
            (sc(cs, Key::J), "expand_indent"),
            (sc(a, Key::C), "color_at_cursor"),
            // Find
            (sc(cs, Key::F), "find_in_files"),
            (sc(cs, Key::H), "replace_next"),
            (sc(cs, Key::E), "use_sel_replace"),
            (sc(c, Key::F), "find"),
            (sc(c, Key::I), "find"),
            (sc(c, Key::H), "replace"),
            (sc(c, Key::E), "use_sel_find"),
            (sc(c, Key::F3), "quick_find"),
            (sc(s, Key::F3), "find_prev"),
            (sc(n, Key::F3), "find_next"),
            (sc(s, Key::F4), "prev_result"),
            (sc(n, Key::F4), "next_result"),
            // View
            (sc(c, Key::Backtick), "console"),
            (sc(s, Key::F11), "distraction_free"),
            (sc(n, Key::F11), "fullscreen"),
            (sc(a, Key::Z), "toggle_wrap"),
            (sc(a, Key::M), "md_split"),
            (sc(a, Key::B), "md_browser"),
            // Goto
            (sc(cs, Key::R), "goto_symbol_project"),
            (sc(c, Key::R), "goto_symbol"),
            (sc(c, Key::P), "goto_anything"),
            (sc(c, Key::G), "goto_line"),
            (sc(s, Key::F12), "goto_reference"),
            (sc(n, Key::F12), "goto_def"),
            (sc(as_, Key::Minus), "jump_forward"),
            (sc(a, Key::Minus), "jump_back"),
            (sc(c, Key::PageDown), "next_tab"),
            (sc(c, Key::PageUp), "prev_tab"),
            (sc(cs, Key::Tab), "prev_in_stack"),
            (sc(c, Key::Tab), "next_in_stack"),
            (sc(a, Key::O), "switch_header"),
            (sc(c, Key::ArrowUp), "scroll_up"),
            (sc(c, Key::ArrowDown), "scroll_down"),
            (sc(cs, Key::F2), "clear_bookmarks"),
            (sc(c, Key::F2), "toggle_bookmark"),
            (sc(s, Key::F2), "prev_bookmark"),
            (sc(n, Key::F2), "next_bookmark"),
            (sc(c, Key::M), "matching_bracket"),
            // Tools
            (sc(cs, Key::B), "build_with"),
            (sc(c, Key::B), "build"),
            (sc(c, Key::Pause), "cancel_build"),
            (sc(cs, Key::Q), "play_macro"),
            (sc(c, Key::Q), "record_macro"),
            (sc(cas, Key::P), "scope_name"),
            (sc(ca, Key::P), "switch_project"),
            // Preferences
            (sc(c, Key::Equals), "zoom_in"),
            (sc(c, Key::Plus), "zoom_in"),
            (sc(c, Key::Minus), "zoom_out"),
            (sc(c, Key::Num0), "font_reset"),
        ];
        let tabs_keys = [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7, Key::Num8, Key::Num9, Key::Num0];
        let mut tab_hit = None;
        let mut start_chord = false;
        let mut escape = false;
        let mut tab_key: Option<bool> = None; // Some(shift)
        let mut del_word_back = false;
        ctx.input_mut(|i| {
            if i.consume_shortcut(&sc(c, Key::K)) {
                start_chord = true;
            }
            for (k, key) in tabs_keys.iter().enumerate() {
                if i.consume_shortcut(&sc(a, *key)) {
                    tab_hit = Some(k + 1);
                }
            }
            for (sh, id) in &list {
                if i.consume_shortcut(sh) {
                    hits.push(id.to_string());
                }
            }
            if editor_focused {
                if i.consume_shortcut(&sc(c, Key::Backspace)) {
                    del_word_back = true;
                }
                if i.consume_shortcut(&sc(s, Key::Tab)) {
                    tab_key = Some(true);
                } else if i.consume_shortcut(&sc(n, Key::Tab)) {
                    tab_key = Some(false);
                }
            }
            if i.consume_key(n, Key::Escape) {
                escape = true;
            }
        });
        if start_chord {
            self.chord_k = Some(Instant::now());
            self.log("Ctrl+K was pressed, waiting for the second key…");
        }
        if let Some(t) = tab_hit {
            hits.push(format!("tab:{t}"));
        }
        if del_word_back {
            hits.push("del_word_back".into());
        }
        if let Some(shift) = tab_key {
            self.tab_key(shift);
        }
        if escape {
            if self.palette.is_some() {
                self.palette = None;
            } else if self.color_edit.is_some() {
                self.color_edit = None;
            } else if self.scope_popup.is_some() {
                self.scope_popup = None;
            } else if self.find.open {
                self.find.open = false;
                self.fif.open = false;
            } else if self.panel.is_some() {
                self.panel = None;
            } else {
                self.run("single_selection");
            }
            self.hover_popup = None;
            if let Some(d) = self.docs.get_mut(self.active) {
                d.view.focus = true;
            }
        }
        for h in hits {
            self.dispatch(&h, ctx);
        }

        // Copy / Cut with nothing selected act on the whole line (like the original),
        // and everything copied goes into Paste from History.
        if editor_focused {
            let (lo, hi) = self.sel();
            let wants: Vec<bool> = ctx.input(|i| i.events.iter().filter_map(|e| match e {
                egui::Event::Copy => Some(false),
                egui::Event::Cut => Some(true),
                _ => None,
            }).collect());
            if let Some(&cut) = wants.first() {
                if lo == hi {
                    ctx.input_mut(|i| i.events.retain(|e| !matches!(e, egui::Event::Copy | egui::Event::Cut)));
                    self.copy_or_cut(cut);
                } else {
                    let t = self.selected_text();
                    self.paste_history.retain(|x| x != &t);
                    self.paste_history.insert(0, t);
                    self.paste_history.truncate(15);
                }
            }
        }
    }

    /// Tab / Shift+Tab: indent a multi-line selection, else insert a tab (or spaces).
    fn tab_key(&mut self, shift: bool) {
        let tab = self.s.tab_size.max(1);
        let (lo, hi) = self.sel();
        let multi = self.docs.get(self.active).map(|d| d.text.chars().skip(lo).take(hi - lo).any(|c| c == '\n')).unwrap_or(false);
        if shift {
            self.run("unindent");
        } else if multi {
            self.run("indent");
        } else {
            let col = self.docs.get(self.active).map(|d| {
                let before: Vec<char> = d.text.chars().take(lo).collect();
                let ls = crate::textops::line_start(&before, before.len());
                before[ls..].iter().map(|c| if *c == '\t' { tab } else { 1 }).sum::<usize>()
            }).unwrap_or(0);
            let ins = if self.s.translate_tabs { " ".repeat(tab - col % tab) } else { "\t".to_string() };
            self.edit_active(|t, lo, hi| {
                let c: Vec<char> = t.chars().collect();
                let mut out: String = c[..lo].iter().collect();
                out.push_str(&ins);
                out.extend(c[hi..].iter());
                let p = lo + ins.chars().count();
                Some(crate::textops::Out { text: out, sel: (p, p) })
            });
        }
    }

    /// Find / Replace / Find in Files bar.
    pub(super) fn find_bar(&mut self, ui: &mut egui::Ui) {
        let mut next = false;
        let mut prev = false;
        let mut rep_one = false;
        let mut rep_all = false;
        let mut in_files = false;
        let p = self.pal;
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.toggle_value(&mut self.find.regex, ".*").on_hover_text("Regular expression");
            ui.toggle_value(&mut self.find.case, "Aa").on_hover_text("Match case");
            ui.label(RichText::new(if self.fif.open { "Find in files:" } else { "Find:" }).color(p.muted));
            let r = ui.add(egui::TextEdit::singleline(&mut self.find.query).hint_text("Find").desired_width(240.0));
            if self.find.focus {
                r.request_focus();
                self.find.focus = false;
            }
            if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                if self.fif.open {
                    in_files = true;
                } else if ui.input(|i| i.modifiers.shift) {
                    prev = true
                } else {
                    next = true
                }
                r.request_focus();
            }
            if self.fif.open {
                ui.label(RichText::new("Where:").color(p.muted));
                ui.add(egui::TextEdit::singleline(&mut self.fif.where_).hint_text("<open folders>, or folders separated by commas").desired_width(220.0));
                if ui.small_button("…").on_hover_text("Pick a folder").clicked() {
                    if let Some(f) = rfd::FileDialog::new().pick_folder() {
                        self.fif.where_ = f.display().to_string();
                    }
                }
                if ui.button("Find").clicked() {
                    in_files = true;
                }
                if ui.small_button("This file only").clicked() {
                    self.fif.open = false;
                }
            } else {
                if ui.button("Find").clicked() {
                    next = true;
                }
                if ui.button("Find Prev").clicked() {
                    prev = true;
                }
                if self.find.replace_open {
                    ui.add(egui::TextEdit::singleline(&mut self.find.replace).hint_text("Replace").desired_width(180.0));
                    if ui.button("Replace").clicked() {
                        rep_one = true;
                    }
                    if ui.button("Replace All").clicked() {
                        rep_all = true;
                    }
                } else if ui.small_button("⇄").on_hover_text("Show replace").clicked() {
                    self.find.replace_open = true;
                }
                if ui.small_button("In files…").on_hover_text("Find in Files (Ctrl+Shift+F)").clicked() {
                    self.fif.open = true;
                    if self.fif.where_.is_empty() {
                        self.fif.where_ = "<open folders>".into();
                    }
                }
                if let Some(d) = self.docs.get(self.active) {
                    if !self.find.query.is_empty() {
                        let n = editor::find_matches(&d.text, &self.find.query, self.find.case, self.find.regex).len();
                        ui.weak(if n == 0 { "No results".to_string() } else { format!("{n} matches") });
                    }
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("✕").clicked() {
                    self.find.open = false;
                    self.fif.open = false;
                }
            });
        });
        if in_files {
            self.start_find_in_files();
        }
        if rep_all {
            self.replace_all();
        } else if rep_one {
            self.replace_step();
        } else if next {
            self.find_step(true);
        } else if prev {
            self.find_step(false);
        }
    }

    /// The bottom panel: Problems, Find Results, Build Results, Console.
    pub(super) fn bottom_panel(&mut self, ui: &mut egui::Ui) {
        let p = self.pal;
        let mut close = false;
        ui.horizontal(|ui| {
            for (k, label) in [(BottomPanel::Problems, "Problems"), (BottomPanel::FindResults, "Find Results"), (BottomPanel::Build, "Build Results"), (BottomPanel::Console, "Console")] {
                if ui.selectable_label(self.panel == Some(k), label).clicked() {
                    self.panel = Some(k);
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("✕").clicked() {
                    close = true;
                }
                if self.panel == Some(BottomPanel::Build) && self.build.running.load(std::sync::atomic::Ordering::Relaxed) {
                    if ui.small_button("Cancel Build").clicked() {
                        self.run("cancel_build");
                    }
                    ui.spinner();
                }
                if self.panel == Some(BottomPanel::FindResults) && self.fif.rx.is_some() {
                    if ui.small_button("Cancel").clicked() {
                        self.run("cancel_fif");
                    }
                    ui.spinner();
                }
            });
        });
        ui.separator();
        let mono = FontId::monospace(self.s.theme.editor_font_size * 0.9);
        match self.panel {
            Some(BottomPanel::Problems) => self.problems_panel(ui),
            Some(BottomPanel::FindResults) => {
                ui.label(RichText::new(&self.fif.title).color(p.muted));
                let mut open: Option<usize> = None;
                egui::ScrollArea::vertical().auto_shrink([false, false]).show_rows(ui, 18.0, self.fif.hits.len(), |ui, range| {
                    for i in range {
                        let h = &self.fif.hits[i];
                        let show_file = i == 0 || self.fif.hits[i - 1].path != h.path;
                        let sel = self.fif.cur == Some(i);
                        let file = if show_file { h.path.display().to_string() } else { String::new() };
                        ui.horizontal(|ui| {
                            ui.add_sized([260.0, 16.0], egui::Label::new(RichText::new(file).color(p.accent).size(12.0)).truncate());
                            let t = format!("{:>5}:  {}", h.line + 1, h.text.trim_end());
                            if ui.add(egui::Button::selectable(sel, RichText::new(t).font(mono.clone())).frame_when_inactive(false)).clicked() {
                                open = Some(i);
                            }
                        });
                    }
                });
                if let Some(i) = open {
                    self.fif.cur = Some(i);
                    let h = self.fif.hits[i].clone();
                    self.open_hit(h);
                }
            }
            Some(BottomPanel::Build) => {
                let lines = self.build.output.lock().unwrap().clone();
                let results = self.build_results();
                let re = fancy_regex::Regex::new(&self.build.regex).ok().filter(|_| !self.build.regex.is_empty());
                let mut jump = None;
                egui::ScrollArea::vertical().auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
                    for l in &lines {
                        let is_hit = re.as_ref().map(|r| r.is_match(l).unwrap_or(false)).unwrap_or(false);
                        let col = if l.starts_with('[') { p.muted } else if is_hit { p.accent } else if l.to_lowercase().contains("error") { Color32::from_rgb(0xf0, 0x5d, 0x5d) } else { p.fg };
                        let r = ui.add(egui::Label::new(RichText::new(l).font(mono.clone()).color(col)).sense(egui::Sense::click()));
                        if is_hit && r.clicked() {
                            jump = results.iter().position(|x| re.as_ref().and_then(|re| re.captures(l).ok().flatten()).and_then(|c| c.get(1)).map(|m| x.0.ends_with(m.as_str()) || x.0 == PathBuf::from(m.as_str())).unwrap_or(false));
                        }
                    }
                });
                if let Some(i) = jump {
                    self.build.cur = Some(i);
                    let (p, line, col, _) = results[i].clone();
                    self.open_hit(features::Hit { path: p, line: line.saturating_sub(1), col: col.saturating_sub(1), len: 0, text: String::new() });
                }
            }
            Some(BottomPanel::Console) => {
                egui::ScrollArea::vertical().auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
                    for l in &self.console {
                        ui.label(RichText::new(l).font(mono.clone()).color(p.fg));
                    }
                    for c in self.clients.values() {
                        for l in &c.log {
                            ui.label(RichText::new(format!("[{}] {l}", c.name)).font(mono.clone()).color(p.muted));
                        }
                    }
                });
            }
            None => {}
        }
        if close {
            self.panel = None;
        }
    }
}
