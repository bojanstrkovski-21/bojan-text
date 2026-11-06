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
        let as_ = Modifiers::ALT | Modifiers::SHIFT;
        let cas = Modifiers::COMMAND | Modifiers::ALT | Modifiers::SHIFT;
        let editor_focused = self.editor_focus_id().map(|id| ctx.memory(|m| m.has_focus(id))).unwrap_or(false);

        // second key of a Ctrl+K chord (most specific first; Ctrl may stay held or not)
        let mut hits: Vec<String> = Vec::new();
        if let Some(t) = self.chord_k {
            if t.elapsed().as_secs_f32() > 2.0 {
                self.chord_k = None;
            } else {
                let chord: &[(Modifiers, Key, &str)] = &[
                    (cs, Key::ArrowUp, "new_group"),
                    (cs, Key::ArrowRight, "move_group_next"),
                    (cs, Key::ArrowLeft, "move_group_prev"),
                    (cs, Key::Z, "revert_mod"),
                    (c, Key::ArrowUp, "move_new_group"),
                    (c, Key::ArrowDown, "close_group"),
                    (c, Key::ArrowRight, "focus_group_next"),
                    (c, Key::ArrowLeft, "focus_group_prev"),
                    (c, Key::CloseBracket, "reindent"),
                    (c, Key::Slash, "noop"),
                    (c, Key::Backspace, "del_to_start"),
                    (c, Key::B, "toggle_sidebar"),
                    (c, Key::U, "upper"),
                    (c, Key::L, "lower"),
                    (c, Key::K, "del_to_end"),
                    (c, Key::Space, "set_mark"),
                    (c, Key::A, "select_to_mark"),
                    (c, Key::W, "delete_to_mark"),
                    (c, Key::X, "swap_with_mark"),
                    (c, Key::G, "clear_mark"),
                    (c, Key::Y, "yank"),
                    (c, Key::C, "scroll_to_sel"),
                    (c, Key::Z, "revert_mod"),
                    (c, Key::V, "paste_history"),
                    (c, Key::D, "quick_add_next"),
                ];
                ctx.input_mut(|i| {
                    for (m, k, id) in chord {
                        let plain = Modifiers { command: false, ctrl: false, mac_cmd: false, ..*m };
                        if i.consume_shortcut(&sc(*m, *k)) || i.consume_shortcut(&sc(plain, *k)) {
                            hits.push(id.to_string());
                            self.chord_k = None;
                            break;
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
            (sc(cs, Key::J), "join_lines"),
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
            (sc(cs, Key::Space), "expand_scope"),
            (sc(a, Key::L), "expand_line_up"),
            (sc(c, Key::L), "expand_line"),
            (sc(c, Key::D), "quick_add_next"),
            (sc(cs, Key::M), "expand_brackets"),
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
            (sc(as_, Key::P), "switch_project"),
            (sc(s, Key::Escape), "show_build"),
            (sc(c, Key::Period), "next_mod"),
            (sc(c, Key::Comma), "prev_mod"),
            // split panes
            (sc(as_, Key::Num1), "layout:1:1"),
            (sc(as_, Key::Num2), "layout:2:2"),
            (sc(as_, Key::Num3), "layout:3:3"),
            (sc(as_, Key::Num4), "layout:4:4"),
            (sc(as_, Key::Num5), "layout:4:2"),
            (sc(as_, Key::Num8), "layout:2:1"),
            (sc(as_, Key::Num9), "layout:3:1"),
            (sc(cs, Key::Num1), "move_group:1"),
            (sc(cs, Key::Num2), "move_group:2"),
            (sc(cs, Key::Num3), "move_group:3"),
            (sc(cs, Key::Num4), "move_group:4"),
            (sc(c, Key::Num1), "focus_group:1"),
            (sc(c, Key::Num2), "focus_group:2"),
            (sc(c, Key::Num3), "focus_group:3"),
            (sc(c, Key::Num4), "focus_group:4"),
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
        let mut enter_key = false;
        let mut del_word_back = false;
        ctx.input_mut(|i| {
            if i.consume_shortcut(&sc(c, Key::K)) {
                start_chord = true;
            }
            for (sh, id) in &list {
                if i.consume_shortcut(sh) {
                    hits.push(id.to_string());
                }
            }
            // Alt+1…Alt+0 (after the Alt+Shift layout keys, which must win)
            for (k, key) in tabs_keys.iter().enumerate() {
                if i.consume_shortcut(&sc(a, *key)) {
                    tab_hit = Some(k + 1);
                }
            }
            if editor_focused {
                if i.consume_shortcut(&sc(c, Key::Backspace)) {
                    del_word_back = true;
                }
                if i.consume_shortcut(&sc(n, Key::Enter)) || i.consume_shortcut(&sc(s, Key::Enter)) {
                    enter_key = true;
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
        if enter_key {
            let unit = if self.s.translate_tabs { " ".repeat(self.s.tab_size.max(1)) } else { "\t".to_string() };
            self.edit_active(|t, lo, hi| Some(crate::textops::newline(t, lo, hi, &unit, true)));
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

    /// Tab / Shift+Tab, following the original's key map (later rules win):
    /// 1. a multi-line selection is indented (Shift+Tab: unindented);
    /// 2. on an empty line with `auto_indent`, the line is reindented;
    /// 3. a snippet trigger before the cursor expands (`tab_completion`);
    /// 4. a word before the cursor is completed when there is exactly one candidate
    ///    (`commit_single`), else the completion list opens;
    /// 5. otherwise a tab (or spaces) is inserted. Shift+Tab unindents when only
    ///    whitespace precedes the cursor, else inserts a tab.
    fn tab_key(&mut self, shift: bool) {
        let tab = self.s.tab_size.max(1);
        let unit = if self.s.translate_tabs { " ".repeat(tab) } else { "\t".to_string() };
        let (lo, hi) = self.sel();
        let Some(d) = self.docs.get(self.active) else { return };
        let chars: Vec<char> = d.text.chars().collect();
        let multi = chars[lo..hi].iter().any(|c| *c == '\n');
        let ls = crate::textops::line_start(&chars, lo);
        let le = crate::textops::line_end(&chars, lo);
        let before_ws = chars[ls..lo].iter().all(|c| *c == ' ' || *c == '\t');
        if shift {
            if multi || before_ws {
                self.run("unindent");
                return;
            }
        } else if multi {
            self.run("indent");
            return;
        } else if lo == hi && ls == le {
            // empty line: reindent it from the lines above
            self.edit_active(|t, lo, _| {
                let o = crate::textops::reindent(t, lo, lo, &unit);
                let c: Vec<char> = o.text.chars().collect();
                let ls = crate::textops::line_start(&c, lo.min(c.len()));
                let p = crate::textops::line_end(&c, ls);
                Some(crate::textops::Out { text: o.text, sel: (p, p) })
            });
            return;
        } else if lo == hi && lo > 0 && (chars[lo - 1].is_alphanumeric() || chars[lo - 1] == '_') {
            let mut a = lo;
            while a > 0 && (chars[a - 1].is_alphanumeric() || chars[a - 1] == '_') {
                a -= 1;
            }
            let word: String = chars[a..lo].iter().collect();
            let name = self.syntax_name_for(d);
            let scope = highlight::syntax_by_name(&self.syntax_set, &name).map(|s| s.scope.build_string()).unwrap_or_default();
            let back = lo - a;
            if let Some(sn) = self.extras.snippets.iter().find(|s| s.trigger == word && super::palette::scope_matches(&s.scope, &scope)).cloned() {
                let (body, at) = crate::textops::expand_snippet(&sn.content);
                self.edit_active(|t, lo, hi| {
                    let c: Vec<char> = t.chars().collect();
                    let a = lo - back;
                    let mut out: String = c[..a].iter().collect();
                    out.push_str(&body);
                    out.extend(c[hi..].iter());
                    let p = a + at;
                    Some(crate::textops::Out { text: out, sel: (p, p) })
                });
                return;
            }
            // words already in the file that continue what was typed
            let pl = word.to_lowercase();
            let mut seen = HashSet::new();
            let cands: Vec<String> = d
                .text
                .split(|ch: char| !(ch.is_alphanumeric() || ch == '_'))
                .filter(|w| w.chars().count() > word.chars().count() && w.to_lowercase().starts_with(&pl) && seen.insert(w.to_string()))
                .map(String::from)
                .collect();
            if cands.len() == 1 {
                let w = cands[0].clone();
                self.edit_active(|t, lo, hi| {
                    let c: Vec<char> = t.chars().collect();
                    let a = lo - back;
                    let mut out: String = c[..a].iter().collect();
                    out.push_str(&w);
                    out.extend(c[hi..].iter());
                    let p = a + w.chars().count();
                    Some(crate::textops::Out { text: out, sel: (p, p) })
                });
                return;
            } else if cands.len() > 1 {
                self.open_palette(PaletteMode::Complete);
                return;
            }
        }
        let mut vcol = 0usize;
        for c in &chars[ls..lo] {
            vcol += if *c == '\t' { tab - vcol % tab } else { 1 };
        }
        let ins = if self.s.translate_tabs { " ".repeat(tab - vcol % tab) } else { "\t".to_string() };
        self.edit_active(|t, lo, hi| {
            let c: Vec<char> = t.chars().collect();
            let mut out: String = c[..lo].iter().collect();
            out.push_str(&ins);
            out.extend(c[hi..].iter());
            let p = lo + ins.chars().count();
            Some(crate::textops::Out { text: out, sel: (p, p) })
        });
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
            ui.toggle_value(&mut self.find.case, "Aa").on_hover_text("Case sensitive");
            ui.toggle_value(&mut self.find.whole, "\"ab\"").on_hover_text("Whole word");
            ui.toggle_value(&mut self.find.wrap, "↺").on_hover_text("Wrap around");
            let mut in_sel = self.find.in_sel.is_some();
            if ui.toggle_value(&mut in_sel, "▭").on_hover_text("In selection").changed() {
                let (lo, hi) = self.sel();
                self.find.in_sel = if in_sel && lo != hi { Some((lo, hi)) } else { None };
            }
            ui.toggle_value(&mut self.find.highlight, "▤").on_hover_text("Highlight matches");
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
                        let n = self.find_all(&d.text).len();
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
                // The original's Find Results text: a "Searching…" header, then per file
                // `path:` followed by ` %4d: line` rows (sub_140068eaa), then the summary.
                let mut rows: Vec<(Option<usize>, String, u8)> = Vec::new(); // (hit, text, 0 plain / 1 file / 2 hit)
                if self.fif.header.is_empty() && self.fif.hits.is_empty() {
                    rows.push((None, "No find results yet".into(), 0));
                } else {
                    rows.push((None, self.fif.header.clone(), 0));
                    rows.push((None, String::new(), 0));
                }
                let mut last: Option<&PathBuf> = None;
                for (i, h) in self.fif.hits.iter().enumerate() {
                    if last != Some(&h.path) {
                        if last.is_some() {
                            rows.push((None, String::new(), 0));
                        }
                        rows.push((None, format!("{}:", h.path.display()), 1));
                        last = Some(&h.path);
                    }
                    rows.push((Some(i), format!(" {:>4}: {}", h.line + 1, h.text.trim_end()), 2));
                }
                if self.fif.rx.is_none() && !self.fif.footer.is_empty() {
                    rows.push((None, String::new(), 0));
                    for l in self.fif.footer.lines() {
                        rows.push((None, l.to_string(), 0));
                    }
                }
                let mut open: Option<usize> = None;
                egui::ScrollArea::vertical().auto_shrink([false, false]).show_rows(ui, 18.0, rows.len(), |ui, range| {
                    for k in range {
                        let (hit, text, kind) = &rows[k];
                        match kind {
                            1 => {
                                ui.label(RichText::new(text).font(mono.clone()).color(p.accent));
                            }
                            2 => {
                                let sel = self.fif.cur == *hit;
                                if ui.add(egui::Button::selectable(sel, RichText::new(text).font(mono.clone())).frame_when_inactive(false)).clicked() {
                                    open = *hit;
                                }
                            }
                            _ => {
                                ui.label(RichText::new(text).font(mono.clone()).color(p.fg));
                            }
                        }
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
