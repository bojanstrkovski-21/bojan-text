//! The main menu, laid out like the original's Main.sublime-menu
//! (File, Edit, Selection, Find, View, Goto, Tools, Project, Preferences, Help).

use super::features::{BottomPanel, Encoding, LineEnding};
use super::*;

type Cmd = Option<String>;

fn item(ui: &mut egui::Ui, label: &str, sc: &str, id: &str, cmd: &mut Cmd) {
    if ui.add(egui::Button::new(label).shortcut_text(sc)).clicked() {
        *cmd = Some(id.to_string());
        ui.close();
    }
}

/// An item this version can't do yet: shown greyed with the reason on hover.
fn off(ui: &mut egui::Ui, label: &str, sc: &str, why: &str) {
    ui.add_enabled(false, egui::Button::new(label).shortcut_text(sc)).on_disabled_hover_text(why);
}

fn check(ui: &mut egui::Ui, label: &str, sc: &str, on: bool, id: &str, cmd: &mut Cmd) {
    let text = format!("{}  {label}", if on { "✔" } else { "    " });
    if ui.add(egui::Button::new(text).shortcut_text(sc)).clicked() {
        *cmd = Some(id.to_string());
        ui.close();
    }
}

const MULTI: &str = "Needs multiple cursors, which this version doesn't have yet";
const PANES: &str = "Needs split panes, which this version doesn't have yet";

impl App {
    pub(super) fn menu_bar(&mut self, ui: &mut egui::Ui) {
        let mut cmd: Cmd = None;
        let doc_le = self.docs.get(self.active).map(|d| d.line_ending);
        let doc_enc = self.docs.get(self.active).map(|d| d.encoding);
        let doc_dirty = self.docs.get(self.active).map(|d| d.dirty() && d.path.is_some()).unwrap_or(false);
        let doc_has_mod = self.docs.get(self.active).map(|d| d.mod_cache.1.is_some()).unwrap_or(false);
        let bookmarks: Vec<usize> = self.docs.get(self.active).map(|d| d.bookmarks.clone()).unwrap_or_default();
        let cur_syntax = self.docs.get(self.active).map(|d| self.syntax_name_for(d)).unwrap_or_default();
        let mut syntaxes: Vec<String> = self.syntax_set.syntaxes().iter().filter(|s| !s.hidden).map(|s| s.name.clone()).collect();
        syntaxes.sort_by_key(|s| s.to_lowercase());
        syntaxes.dedup();
        let builds = self.build_systems();
        let matching_names: Vec<String> = self.build_systems_for_menu();
        let user_macros = Self::saved_macros();
        let mut pkg_macros: Vec<(String, Vec<String>)> = Vec::new();
        for (pkg, name) in &self.extras.macros {
            match pkg_macros.iter_mut().find(|(p, _)| p == pkg) {
                Some((_, v)) => v.push(name.clone()),
                None => pkg_macros.push((pkg.clone(), vec![name.clone()])),
            }
        }
        for (_, v) in pkg_macros.iter_mut() {
            v.sort_by_key(|s| s.to_lowercase());
        }
        pkg_macros.sort_by(|a, b| (a.0 != "Default").cmp(&(b.0 != "Default")).then(a.0.to_lowercase().cmp(&b.0.to_lowercase())));
        let recording = self.recording.is_some();
        let building = self.build.running.load(std::sync::atomic::Ordering::Relaxed);
        let has_build_output = self.build.output.lock().map(|o| o.len() > 1).unwrap_or(false);
        let has_results = !self.fif.hits.is_empty() || !self.build_results().is_empty();
        let has_folders = !self.all_folders().is_empty();
        let has_project = self.project_file.is_some();
        let (groups, cols, max_cols) = (self.groups, self.cols, self.max_columns);
        let hist_empty = self.paste_history.is_empty();
        let yank_empty = self.yank.is_empty();
        let tab_titles: Vec<String> = self.docs.iter().take(10).map(|d| d.title.clone()).collect();

        egui::MenuBar::new().ui(ui, |ui| {
            // ---------------------------------------------------------------- File
            ui.menu_button("File", |ui| {
                item(ui, "New File", "Ctrl+N", "new", &mut cmd);
                item(ui, "Open File…", "Ctrl+O", "open", &mut cmd);
                item(ui, "Open Folder…", "", "open_folder", &mut cmd);
                ui.menu_button("Open Recent", |ui| {
                    if self.s.recent.is_empty() {
                        ui.weak("Nothing yet");
                    }
                    for r in self.s.recent.clone() {
                        item(ui, &r, "", &format!("open_recent:{r}"), &mut cmd);
                    }
                    ui.separator();
                    item(ui, "Clear Items", "", "clear_recent", &mut cmd);
                });
                ui.menu_button("Reopen with Encoding", |ui| {
                    for e in Encoding::ALL {
                        item(ui, e.label(), "", &format!("reopen_enc:{}", e.label()), &mut cmd);
                    }
                });
                item(ui, "Split View", "", "split_view", &mut cmd);
                ui.separator();
                item(ui, "Save", "Ctrl+S", "save", &mut cmd);
                ui.menu_button("Save with Encoding", |ui| {
                    for e in Encoding::ALL {
                        check(ui, e.label(), "", doc_enc == Some(e), &format!("save_enc:{}", e.label()), &mut cmd);
                    }
                });
                item(ui, "Save As…", "Ctrl+Shift+S", "save_as", &mut cmd);
                item(ui, "Save All", "", "save_all", &mut cmd);
                item(ui, "Print…", "", "print", &mut cmd);
                item(ui, "Export Markdown as HTML…", "", "md_export", &mut cmd);
                ui.separator();
                item(ui, "New Window", "Ctrl+Shift+N", "new_window", &mut cmd);
                item(ui, "Close Window", "Ctrl+Shift+W", "close_window", &mut cmd);
                ui.separator();
                item(ui, "Close File", "Ctrl+W", "close", &mut cmd);
                if doc_dirty {
                    item(ui, "Revert File", "", "revert", &mut cmd);
                } else {
                    off(ui, "Revert File", "", "The file has no unsaved changes");
                }
                item(ui, "Close All Files", "", "close_all", &mut cmd);
                ui.separator();
                item(ui, "Exit", "", "exit", &mut cmd);
            });

            // ---------------------------------------------------------------- Edit
            ui.menu_button("Edit", |ui| {
                item(ui, "Undo", "Ctrl+Z", "undo", &mut cmd);
                item(ui, "Redo", "Ctrl+Y", "redo", &mut cmd);
                ui.menu_button("Undo Selection", |ui| {
                    off(ui, "Soft Undo", "Ctrl+U", MULTI);
                    off(ui, "Soft Redo", "Ctrl+Shift+U", MULTI);
                });
                ui.separator();
                item(ui, "Cut", "Ctrl+X", "cut", &mut cmd);
                item(ui, "Copy", "Ctrl+C", "copy", &mut cmd);
                item(ui, "Copy as HTML", "", "copy_html", &mut cmd);
                item(ui, "Paste", "Ctrl+V", "paste", &mut cmd);
                item(ui, "Paste and Indent", "Ctrl+Shift+V", "paste_indent", &mut cmd);
                if hist_empty {
                    off(ui, "Paste from History", "Ctrl+K, Ctrl+V", "Nothing has been copied yet");
                } else {
                    item(ui, "Paste from History", "Ctrl+K, Ctrl+V", "paste_history", &mut cmd);
                }
                ui.separator();
                ui.menu_button("Line", |ui| {
                    item(ui, "Indent", "Ctrl+]", "indent", &mut cmd);
                    item(ui, "Unindent", "Ctrl+[", "unindent", &mut cmd);
                    item(ui, "Reindent", "Ctrl+K, Ctrl+]", "reindent", &mut cmd);
                    item(ui, "Swap Line Up", "Ctrl+Shift+Up", "swap_up", &mut cmd);
                    item(ui, "Swap Line Down", "Ctrl+Shift+Down", "swap_down", &mut cmd);
                    item(ui, "Duplicate Line", "Ctrl+Shift+D", "dup_line", &mut cmd);
                    item(ui, "Delete Line", "Ctrl+Shift+K", "del_line", &mut cmd);
                    item(ui, "Join Lines", "Ctrl+Shift+J", "join_lines", &mut cmd);
                });
                ui.menu_button("Comment", |ui| {
                    item(ui, "Toggle Comment", "Ctrl+/", "comment", &mut cmd);
                    item(ui, "Toggle Block Comment", "Ctrl+Shift+/", "block_comment", &mut cmd);
                });
                ui.menu_button("Text", |ui| {
                    if doc_has_mod {
                        item(ui, "Revert Hunk", "Ctrl+K, Ctrl+Z", "revert_mod", &mut cmd);
                        item(ui, "Revert Modification", "Ctrl+K, Ctrl+Shift+Z", "revert_mod", &mut cmd);
                    } else {
                        off(ui, "Revert Hunk", "Ctrl+K, Ctrl+Z", "No changes since the last save");
                        off(ui, "Revert Modification", "Ctrl+K, Ctrl+Shift+Z", "No changes since the last save");
                    }
                    off(ui, "Toggle Diff Hunk", "Ctrl+K, Ctrl+/", "Inline diffs aren't available yet");
                    ui.separator();
                    item(ui, "Insert Line Before", "Ctrl+Shift+Enter", "line_before", &mut cmd);
                    item(ui, "Insert Line After", "Ctrl+Enter", "line_after", &mut cmd);
                    ui.separator();
                    item(ui, "Delete Word Forward", "Ctrl+Delete", "del_word_fwd", &mut cmd);
                    item(ui, "Delete Word Backward", "Ctrl+Backspace", "del_word_back", &mut cmd);
                    item(ui, "Delete Line", "Ctrl+Shift+K", "del_line", &mut cmd);
                    item(ui, "Delete to End", "Ctrl+K, Ctrl+K", "del_to_end", &mut cmd);
                    item(ui, "Delete to Beginning", "Ctrl+K, Ctrl+Backspace", "del_to_start", &mut cmd);
                    ui.separator();
                    item(ui, "Transpose", "Ctrl+T", "transpose", &mut cmd);
                });
                ui.menu_button("Tag", |ui| {
                    item(ui, "Close Tag", "Alt+.", "close_tag", &mut cmd);
                    item(ui, "Expand Selection to Tag", "Ctrl+Shift+A", "expand_tag", &mut cmd);
                    item(ui, "Wrap Selection With Tag", "Alt+Shift+W", "wrap_tag", &mut cmd);
                });
                ui.menu_button("Mark", |ui| {
                    item(ui, "Set Mark", "Ctrl+K, Ctrl+Space", "set_mark", &mut cmd);
                    item(ui, "Select to Mark", "Ctrl+K, Ctrl+A", "select_to_mark", &mut cmd);
                    item(ui, "Delete to Mark", "Ctrl+K, Ctrl+W", "delete_to_mark", &mut cmd);
                    item(ui, "Swap with Mark", "Ctrl+K, Ctrl+X", "swap_with_mark", &mut cmd);
                    item(ui, "Clear Mark", "Ctrl+K, Ctrl+G", "clear_mark", &mut cmd);
                    ui.separator();
                    if yank_empty {
                        off(ui, "Yank", "Ctrl+K, Ctrl+Y", "Nothing has been deleted with the Mark commands yet");
                    } else {
                        item(ui, "Yank", "Ctrl+K, Ctrl+Y", "yank", &mut cmd);
                    }
                });
                ui.menu_button("Code Folding", |ui| {
                    let why = "Folding isn't supported by this editor's text view yet";
                    off(ui, "Fold", "Ctrl+Shift+[", why);
                    off(ui, "Unfold", "Ctrl+Shift+]", why);
                    off(ui, "Unfold All", "Ctrl+K, Ctrl+J", why);
                    ui.separator();
                    off(ui, "Fold All", "Ctrl+K, Ctrl+1", why);
                    for n in 2..=9 {
                        off(ui, &format!("Fold Level {n}"), &format!("Ctrl+K, Ctrl+{n}"), why);
                    }
                    ui.separator();
                    off(ui, "Fold Tag Attributes", "Ctrl+K, Ctrl+T", why);
                });
                ui.menu_button("Convert Case", |ui| {
                    item(ui, "Title Case", "", "title_case", &mut cmd);
                    item(ui, "Upper Case", "Ctrl+K, Ctrl+U", "upper", &mut cmd);
                    item(ui, "Lower Case", "Ctrl+K, Ctrl+L", "lower", &mut cmd);
                    item(ui, "Swap Case", "", "swap_case", &mut cmd);
                    item(ui, "lowerCamelCase", "", "camel", &mut cmd);
                    item(ui, "UpperCamelCase", "", "pascal", &mut cmd);
                    item(ui, "snake_case", "", "snake", &mut cmd);
                    item(ui, "kebab-case", "", "kebab", &mut cmd);
                });
                ui.menu_button("Wrap", |ui| {
                    item(ui, "Wrap Paragraph", "Alt+Q", "wrap_at:0", &mut cmd);
                    for w in [70, 72, 78, 80, 100, 120] {
                        item(ui, &format!("Wrap paragraph at {w} characters"), "", &format!("wrap_at:{w}"), &mut cmd);
                    }
                });
                item(ui, "Show Completions", "Ctrl+Space", "complete", &mut cmd);
                ui.separator();
                item(ui, "Sort Lines", "F9", "sort_lines", &mut cmd);
                item(ui, "Sort Lines (Case Sensitive)", "Ctrl+F9", "sort_case", &mut cmd);
                ui.menu_button("Permute Lines", |ui| {
                    item(ui, "Reverse", "", "reverse_lines", &mut cmd);
                    item(ui, "Unique", "", "unique_lines", &mut cmd);
                    item(ui, "Shuffle", "", "shuffle_lines", &mut cmd);
                });
                ui.menu_button("Permute Selections", |ui| {
                    for l in ["Sort", "Sort (Case Sensitive)", "Reverse", "Unique", "Shuffle"] {
                        off(ui, l, "", MULTI);
                    }
                });
            });

            // ---------------------------------------------------------------- Selection
            ui.menu_button("Selection", |ui| {
                off(ui, "Split into Lines", "Ctrl+Shift+L", MULTI);
                item(ui, "Single Selection", "Escape", "single_selection", &mut cmd);
                ui.separator();
                item(ui, "Select All", "Ctrl+A", "select_all", &mut cmd);
                item(ui, "Expand Selection", "Ctrl+Shift+A", "expand_sel", &mut cmd);
                item(ui, "Expand Selection to Line", "Ctrl+L", "expand_line", &mut cmd);
                item(ui, "Expand Selection to Line Upward", "Alt+L", "expand_line_up", &mut cmd);
                item(ui, "Expand Selection to Word", "Ctrl+D", "expand_word", &mut cmd);
                item(ui, "Expand Selection to Block", "", "expand_para", &mut cmd);
                item(ui, "Expand Selection to Paragraph", "", "expand_para", &mut cmd);
                item(ui, "Expand Selection to Scope", "Ctrl+Shift+Space", "expand_scope", &mut cmd);
                item(ui, "Expand Selection to Brackets", "Ctrl+Shift+M", "expand_brackets", &mut cmd);
                item(ui, "Expand Selection to Indentation", "", "expand_indent", &mut cmd);
                ui.separator();
                off(ui, "Add Previous Line", "Alt+Shift+Up", MULTI);
                off(ui, "Add Next Line", "Alt+Shift+Down", MULTI);
                ui.separator();
                ui.menu_button("Tab Selection", |ui| {
                    let why = "Selecting several tabs at once isn't available yet";
                    off(ui, "Unselect Others", "Ctrl+J, Ctrl+Up", why);
                    off(ui, "Unselect to Left", "Ctrl+J, Ctrl+Left", why);
                    off(ui, "Unselect to Right", "Ctrl+J, Ctrl+Right", why);
                    off(ui, "Unselect Current", "", why);
                    ui.separator();
                    off(ui, "Select to Left", "Ctrl+J, Ctrl+Shift+Left", why);
                    off(ui, "Select to Right", "Ctrl+J, Ctrl+Shift+Right", why);
                    ui.separator();
                    item(ui, "Focus to Left", "Ctrl+J, Ctrl+Pageup", "focus_left", &mut cmd);
                    item(ui, "Focus to Right", "Ctrl+J, Ctrl+Pagedown", "focus_right", &mut cmd);
                });
                ui.separator();
                item(ui, "Edit Color at Cursor…", "Alt+C", "color_at_cursor", &mut cmd);
            });

            // ---------------------------------------------------------------- Find
            ui.menu_button("Find", |ui| {
                item(ui, "Find…", "Ctrl+F", "find", &mut cmd);
                item(ui, "Find Next", "F3", "find_next", &mut cmd);
                item(ui, "Find Previous", "Shift+F3", "find_prev", &mut cmd);
                item(ui, "Incremental Find", "Ctrl+I", "find", &mut cmd);
                ui.separator();
                item(ui, "Replace…", "Ctrl+H", "replace", &mut cmd);
                item(ui, "Replace Next", "Ctrl+Shift+H", "replace_next", &mut cmd);
                ui.separator();
                item(ui, "Quick Find", "Ctrl+F3", "quick_find", &mut cmd);
                off(ui, "Quick Find All", "Alt+F3", MULTI);
                item(ui, "Quick Add Next", "Ctrl+D", "quick_add_next", &mut cmd);
                item(ui, "Quick Skip Next", "Ctrl+K, Ctrl+D", "quick_add_next", &mut cmd);
                ui.separator();
                item(ui, "Use Selection for Find", "Ctrl+E", "use_sel_find", &mut cmd);
                item(ui, "Use Selection for Replace", "Ctrl+Shift+E", "use_sel_replace", &mut cmd);
                ui.separator();
                item(ui, "Find in Files…", "Ctrl+Shift+F", "find_in_files", &mut cmd);
                ui.menu_button("Find Results", |ui| {
                    item(ui, "Show Results Panel", "", "show_results", &mut cmd);
                    if has_results {
                        item(ui, "Next Result", "F4", "next_result", &mut cmd);
                        item(ui, "Previous Result", "Shift+F4", "prev_result", &mut cmd);
                    } else {
                        off(ui, "Next Result", "F4", "Run a search first");
                        off(ui, "Previous Result", "Shift+F4", "Run a search first");
                    }
                });
                if self.fif.rx.is_some() {
                    item(ui, "Cancel Find in Files", "", "cancel_fif", &mut cmd);
                } else {
                    off(ui, "Cancel Find in Files", "", "No search is running");
                }
            });

            // ---------------------------------------------------------------- View
            ui.menu_button("View", |ui| {
                ui.menu_button("Side Bar", |ui| {
                    item(ui, if self.s.show_sidebar { "Hide Side Bar" } else { "Show Side Bar" }, "Ctrl+K, Ctrl+B", "toggle_sidebar", &mut cmd);
                    item(ui, if self.s.show_open_files { "Hide Open Files" } else { "Show Open Files" }, "", "toggle_open_files", &mut cmd);
                });
                item(ui, if self.s.show_minimap { "Hide Minimap" } else { "Show Minimap" }, "", "toggle_minimap", &mut cmd);
                item(ui, if self.s.show_tabs { "Hide Tabs" } else { "Show Tabs" }, "", "toggle_tabs", &mut cmd);
                item(ui, if self.s.show_status_bar { "Hide Status Bar" } else { "Show Status Bar" }, "", "toggle_status", &mut cmd);
                item(ui, "Hide Menu", "", "toggle_menu", &mut cmd);
                item(ui, if self.panel == Some(BottomPanel::Console) { "Hide Console" } else { "Show Console" }, "Ctrl+`", "console", &mut cmd);
                ui.separator();
                item(ui, if self.fullscreen { "Exit Full Screen" } else { "Enter Full Screen" }, "F11", "fullscreen", &mut cmd);
                item(ui, if self.distraction.is_some() { "Exit Distraction Free Mode" } else { "Enter Distraction Free Mode" }, "Shift+F11", "distraction_free", &mut cmd);
                ui.separator();
                ui.menu_button("Layout", |ui| {
                    for (label, g, c, sc) in [
                        ("Single", 1, 1, "Alt+Shift+1"),
                        ("Columns: 2", 2, 2, "Alt+Shift+2"),
                        ("Columns: 3", 3, 3, "Alt+Shift+3"),
                        ("Columns: 4", 4, 4, "Alt+Shift+4"),
                        ("Rows: 2", 2, 1, "Alt+Shift+8"),
                        ("Rows: 3", 3, 1, "Alt+Shift+9"),
                        ("Grid: 4", 4, 2, "Alt+Shift+5"),
                    ] {
                        check(ui, label, sc, groups == g && cols == c, &format!("layout:{g}:{c}"), &mut cmd);
                    }
                });
                ui.menu_button("Groups", |ui| {
                    item(ui, "Move File to New Group", "Ctrl+K, Ctrl+Up", "move_new_group", &mut cmd);
                    item(ui, "New Group", "Ctrl+K, Ctrl+Shift+Up", "new_group", &mut cmd);
                    if groups > 1 {
                        item(ui, "Close Group", "Ctrl+K, Ctrl+Down", "close_group", &mut cmd);
                    } else {
                        off(ui, "Close Group", "Ctrl+K, Ctrl+Down", "There is only one group");
                    }
                    ui.separator();
                    for n in 1..=5 {
                        check(ui, &format!("Max Columns: {n}"), "", max_cols == n, &format!("max_cols:{n}"), &mut cmd);
                    }
                });
                ui.menu_button("Focus Group", |ui| {
                    if groups > 1 {
                        item(ui, "Next", "Ctrl+K, Ctrl+Right", "focus_group_next", &mut cmd);
                        item(ui, "Previous", "Ctrl+K, Ctrl+Left", "focus_group_prev", &mut cmd);
                    } else {
                        off(ui, "Next", "Ctrl+K, Ctrl+Right", "There is only one group");
                        off(ui, "Previous", "Ctrl+K, Ctrl+Left", "There is only one group");
                    }
                    ui.separator();
                    for n in 1..=groups.min(8) {
                        item(ui, &format!("Group {n}"), &format!("Ctrl+{n}"), &format!("focus_group:{n}"), &mut cmd);
                    }
                });
                ui.menu_button("Move File to Group", |ui| {
                    if groups > 1 {
                        item(ui, "Next", "Ctrl+K, Ctrl+Shift+Right", "move_group_next", &mut cmd);
                        item(ui, "Previous", "Ctrl+K, Ctrl+Shift+Left", "move_group_prev", &mut cmd);
                    } else {
                        off(ui, "Next", "Ctrl+K, Ctrl+Shift+Right", "There is only one group");
                        off(ui, "Previous", "Ctrl+K, Ctrl+Shift+Left", "There is only one group");
                    }
                    ui.separator();
                    for n in 1..=groups.min(8) {
                        item(ui, &format!("Group {n}"), &format!("Ctrl+Shift+{n}"), &format!("move_group:{n}"), &mut cmd);
                    }
                });
                ui.separator();
                ui.menu_button("Syntax", |ui| {
                    egui::ScrollArea::vertical().max_height(520.0).show(ui, |ui| {
                        for s in &syntaxes {
                            check(ui, s, "", *s == cur_syntax, &format!("syntax:{s}"), &mut cmd);
                        }
                    });
                });
                ui.menu_button("Indentation", |ui| {
                    check(ui, "Indent Using Spaces", "", self.s.translate_tabs, "toggle_spaces", &mut cmd);
                    ui.separator();
                    for w in 1..=8 {
                        check(ui, &format!("Tab Width: {w}"), "", self.s.tab_size == w, &format!("tab_width:{w}"), &mut cmd);
                    }
                    ui.separator();
                    item(ui, "Guess Settings From Buffer", "", "detect_indent", &mut cmd);
                    ui.separator();
                    item(ui, "Convert Indentation to Spaces", "", "to_spaces", &mut cmd);
                    item(ui, "Convert Indentation to Tabs", "", "to_tabs", &mut cmd);
                });
                ui.menu_button("Line Endings", |ui| {
                    check(ui, "Windows", "", doc_le == Some(LineEnding::Windows), "line_ending:windows", &mut cmd);
                    check(ui, "Unix", "", doc_le == Some(LineEnding::Unix), "line_ending:unix", &mut cmd);
                    check(ui, "Mac OS 9", "", doc_le == Some(LineEnding::Mac), "line_ending:mac", &mut cmd);
                });
                ui.separator();
                check(ui, "Word Wrap", "Alt+Z", self.s.word_wrap, "toggle_wrap", &mut cmd);
                ui.menu_button("Word Wrap Column", |ui| {
                    check(ui, "Automatic", "", self.s.wrap_column == 0, "wrap_col:0", &mut cmd);
                    for w in [70, 72, 78, 80, 100, 120] {
                        check(ui, &w.to_string(), "", self.s.wrap_column == w, &format!("wrap_col:{w}"), &mut cmd);
                    }
                });
                ui.menu_button("Ruler", |ui| {
                    check(ui, "None", "", self.s.rulers.is_empty(), "ruler:0", &mut cmd);
                    for w in [70, 72, 78, 80, 100, 120] {
                        check(ui, &w.to_string(), "", self.s.rulers.contains(&w), &format!("ruler:{w}"), &mut cmd);
                    }
                });
                ui.separator();
                let nospell = "No spelling dictionaries in this version yet";
                off(ui, "Spell Check", "F6", nospell);
                off(ui, "Next Misspelling", "Ctrl+F6", nospell);
                off(ui, "Prev Misspelling", "Ctrl+Shift+F6", nospell);
                ui.menu_button("Dictionary", |ui| {
                    off(ui, "English (American)", "", nospell);
                });
                ui.separator();
                check(ui, "Line Numbers", "", self.s.line_numbers, "toggle_line_numbers", &mut cmd);
                check(ui, "Highlight Current Line", "", self.s.highlight_line, "toggle_highlight_line", &mut cmd);
                check(ui, "Problems Panel", "", self.panel == Some(BottomPanel::Problems), "toggle_problems", &mut cmd);
                ui.menu_button("Markdown Preview", |ui| {
                    item(ui, "Split", "Alt+M", "md_split", &mut cmd);
                    item(ui, "Preview Only", "", "md_preview", &mut cmd);
                    item(ui, "Open in Browser (live)", "Alt+B", "md_browser", &mut cmd);
                    check(ui, "Sync Scrolling", "", self.s.md_sync_scroll, "toggle_md_sync", &mut cmd);
                    check(ui, "Open Markdown Files with Preview", "", self.s.md_auto_split, "toggle_md_auto", &mut cmd);
                });
            });

            // ---------------------------------------------------------------- Goto
            ui.menu_button("Goto", |ui| {
                item(ui, "Goto Anything…", "Ctrl+P", "goto_anything", &mut cmd);
                ui.separator();
                item(ui, "Goto Symbol…", "Ctrl+R", "goto_symbol", &mut cmd);
                item(ui, "Goto Symbol in Project…", "Ctrl+Shift+R", "goto_symbol_project", &mut cmd);
                item(ui, "Goto Definition…", "F12", "goto_def", &mut cmd);
                item(ui, "Goto Reference…", "Shift+F12", "goto_reference", &mut cmd);
                item(ui, "Goto Line…", "Ctrl+G", "goto_line", &mut cmd);
                ui.separator();
                if doc_has_mod {
                    item(ui, "Next Modification", "Ctrl+.", "next_mod", &mut cmd);
                    item(ui, "Previous Modification", "Ctrl+,", "prev_mod", &mut cmd);
                } else {
                    off(ui, "Next Modification", "Ctrl+.", "No changes since the last save");
                    off(ui, "Previous Modification", "Ctrl+,", "No changes since the last save");
                }
                ui.separator();
                item(ui, "Jump Back", "Alt+-", "jump_back", &mut cmd);
                item(ui, "Jump Forward", "Alt+Shift+-", "jump_forward", &mut cmd);
                ui.separator();
                ui.menu_button("Switch File", |ui| {
                    item(ui, "Next File", "Ctrl+Pagedown", "next_tab", &mut cmd);
                    item(ui, "Previous File", "Ctrl+Pageup", "prev_tab", &mut cmd);
                    ui.separator();
                    item(ui, "Next File in Stack", "Ctrl+Tab", "next_in_stack", &mut cmd);
                    item(ui, "Previous File in Stack", "Ctrl+Shift+Tab", "prev_in_stack", &mut cmd);
                    ui.separator();
                    item(ui, "Switch Header/Implementation", "Alt+O", "switch_header", &mut cmd);
                    ui.separator();
                    for (k, name) in tab_titles.iter().enumerate() {
                        let n = k + 1;
                        let sc = if n == 10 { "Alt+0".to_string() } else { format!("Alt+{n}") };
                        item(ui, name, &sc, &format!("tab:{n}"), &mut cmd);
                    }
                });
                ui.menu_button("Scroll", |ui| {
                    item(ui, "Scroll to Selection", "Ctrl+K, Ctrl+C", "scroll_to_sel", &mut cmd);
                    item(ui, "Line Up", "Ctrl+Up", "scroll_up", &mut cmd);
                    item(ui, "Line Down", "Ctrl+Down", "scroll_down", &mut cmd);
                });
                ui.menu_button("Bookmarks", |ui| {
                    item(ui, "Toggle Bookmark", "Ctrl+F2", "toggle_bookmark", &mut cmd);
                    item(ui, "Next Bookmark", "F2", "next_bookmark", &mut cmd);
                    item(ui, "Prev Bookmark", "Shift+F2", "prev_bookmark", &mut cmd);
                    item(ui, "Clear Bookmarks", "Ctrl+Shift+F2", "clear_bookmarks", &mut cmd);
                    off(ui, "Select All Bookmarks", "Alt+F2", MULTI);
                    ui.separator();
                    if bookmarks.is_empty() {
                        off(ui, "(No Bookmarks)", "", "Toggle a bookmark with Ctrl+F2");
                    }
                    for l in &bookmarks {
                        item(ui, &format!("Line {}", l + 1), "", &format!("bookmark_goto:{l}"), &mut cmd);
                    }
                });
                ui.separator();
                item(ui, "Jump to Matching Bracket", "Ctrl+M", "matching_bracket", &mut cmd);
            });

            // ---------------------------------------------------------------- Tools
            ui.menu_button("Tools", |ui| {
                item(ui, "Command Palette…", "Ctrl+Shift+P", "palette_cmd", &mut cmd);
                item(ui, "Snippets…", "", "snippets", &mut cmd);
                ui.separator();
                ui.menu_button("Build System", |ui| {
                    check(ui, "Automatic", "", self.build.choice.is_none(), "build_auto", &mut cmd);
                    ui.separator();
                    egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
                        for b in &builds {
                            let on = self.build.choice.as_ref().map(|c| c.0 == b.name).unwrap_or(false);
                            let label = if matching_names.contains(&b.name) { format!("{}  ·", b.name) } else { b.name.clone() };
                            check(ui, &label, "", on, &format!("build_sys:{}", b.name), &mut cmd);
                        }
                    });
                    ui.separator();
                    item(ui, "New Build System…", "", "new_build_system", &mut cmd);
                });
                item(ui, "Build", "Ctrl+B", "build", &mut cmd);
                item(ui, "Build With…", "Ctrl+Shift+B", "build_with", &mut cmd);
                if building {
                    item(ui, "Cancel Build", "Ctrl+Break", "cancel_build", &mut cmd);
                } else {
                    off(ui, "Cancel Build", "Ctrl+Break", "No build is running");
                }
                ui.menu_button("Build Results", |ui| {
                    if has_build_output {
                        item(ui, "Show Build Results", "Shift+Escape", "show_build", &mut cmd);
                        item(ui, "Next Result", "F4", "next_result_build", &mut cmd);
                        item(ui, "Previous Result", "Shift+F4", "prev_result_build", &mut cmd);
                    } else {
                        off(ui, "Show Build Results", "Shift+Escape", "Run a build first");
                        off(ui, "Next Result", "F4", "Run a build first");
                        off(ui, "Previous Result", "Shift+F4", "Run a build first");
                    }
                });
                check(ui, "Save All on Build", "", self.s.save_all_on_build, "toggle_save_on_build", &mut cmd);
                ui.separator();
                item(ui, if recording { "Stop Recording Macro" } else { "Record Macro" }, "Ctrl+Q", "record_macro", &mut cmd);
                if self.last_macro.is_empty() {
                    off(ui, "Playback Macro", "Ctrl+Shift+Q", "Record a macro first");
                    off(ui, "Save Macro…", "", "Record a macro first");
                } else {
                    item(ui, "Playback Macro", "Ctrl+Shift+Q", "play_macro", &mut cmd);
                    item(ui, "Save Macro…", "", "save_macro", &mut cmd);
                }
                ui.menu_button("Macros", |ui| {
                    for (pkg, names) in &pkg_macros {
                        ui.menu_button(pkg, |ui| {
                            egui::ScrollArea::vertical().max_height(480.0).show(ui, |ui| {
                                for n in names {
                                    item(ui, n, "", &format!("pkg_macro:{pkg}|{n}"), &mut cmd);
                                }
                            });
                        });
                    }
                    if !user_macros.is_empty() {
                        ui.menu_button("User", |ui| {
                            for m in &user_macros {
                                let n = m.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                                item(ui, &n, "", &format!("macro_file:{}", m.display()), &mut cmd);
                            }
                        });
                    }
                    if pkg_macros.is_empty() && user_macros.is_empty() {
                        ui.weak("No macros");
                    }
                });
                ui.separator();
                ui.menu_button("Developer", |ui| {
                    off(ui, "New Plugin…", "", "Python plugins need Sublime Text's own plugin host");
                    item(ui, "New Snippet…", "", "new_snippet", &mut cmd);
                    item(ui, "New Syntax…", "", "new_syntax", &mut cmd);
                    off(ui, "New Syntax from Plain text.tmLanguage…", "", "Old-style .tmLanguage files aren't supported");
                    ui.separator();
                    off(ui, "Profile Plugins", "", "Python plugins need Sublime Text's own plugin host");
                    ui.separator();
                    item(ui, "View Package File…", "", "view_package_file", &mut cmd);
                    item(ui, "Show Scope Name", "Ctrl+Alt+Shift+P", "scope_name", &mut cmd);
                });
                item(ui, "Enable Package Control", "", "package_control", &mut cmd);
                ui.separator();
                item(ui, "Color Codes in This File…", "", "colors_list", &mut cmd);
                check(ui, "Colorizer", "", self.s.colorizer, "toggle_colorizer", &mut cmd);
                ui.separator();
                item(ui, "Language Servers (LSP)…", "", "lsp", &mut cmd);
                item(ui, "Sublime Packages…", "", "packages", &mut cmd);
            });

            // ---------------------------------------------------------------- Project
            ui.menu_button("Project", |ui| {
                item(ui, "Open Project…", "", "open_project", &mut cmd);
                item(ui, "Switch Project…", "", "switch_project", &mut cmd);
                item(ui, "Quick Switch Project…", "Alt+Shift+P", "switch_project", &mut cmd);
                ui.menu_button("Open Recent", |ui| {
                    for p in self.s.recent_projects.clone() {
                        item(ui, &p, "", &format!("project:{p}"), &mut cmd);
                    }
                    if !self.s.recent_projects.is_empty() {
                        ui.separator();
                    }
                    item(ui, "Clear Items", "", "clear_recent_projects", &mut cmd);
                    item(ui, "Remove Deleted", "", "remove_deleted_projects", &mut cmd);
                });
                ui.separator();
                item(ui, "Save Project As…", "", "save_project", &mut cmd);
                if has_project {
                    item(ui, "Close Project", "", "close_project", &mut cmd);
                    item(ui, "Edit Project", "", "edit_project", &mut cmd);
                } else {
                    off(ui, "Close Project", "", "No project is open");
                    off(ui, "Edit Project", "", "No project is open");
                }
                ui.separator();
                off(ui, "New Workspace for Project", "", "One window per project in this version");
                item(ui, "Save Workspace As…", "", "save_project", &mut cmd);
                ui.separator();
                item(ui, "Add Folder to Project…", "", "add_folder", &mut cmd);
                if has_folders {
                    item(ui, "Remove all Folders from Project", "", "remove_folders", &mut cmd);
                    item(ui, "Refresh Folders", "", "refresh_folders", &mut cmd);
                } else {
                    off(ui, "Remove all Folders from Project", "", "No folders are open");
                    off(ui, "Refresh Folders", "", "No folders are open");
                }
            });

            // ---------------------------------------------------------------- Preferences
            ui.menu_button("Preferences", |ui| {
                item(ui, "Browse Packages…", "", "browse_packages", &mut cmd);
                ui.separator();
                item(ui, "Settings", "", "settings", &mut cmd);
                off(ui, "Settings – Syntax Specific", "", "One set of settings for all languages in this version");
                off(ui, "Settings – Distraction Free", "", "Distraction Free mode hides the side bar, tabs, minimap, menu and status bar");
                item(ui, "Key Bindings", "", "key_bindings", &mut cmd);
                item(ui, "Mouse Bindings", "", "mouse_bindings", &mut cmd);
                ui.separator();
                item(ui, "Select Color Scheme…", "", "pick_scheme", &mut cmd);
                item(ui, "Customize Color Scheme", "", "customize_scheme", &mut cmd);
                item(ui, "Select Theme…", "", "theme", &mut cmd);
                item(ui, "Customize Theme", "", "theme", &mut cmd);
                ui.separator();
                ui.menu_button("Font", |ui| {
                    item(ui, "Choose…", "", "choose_font", &mut cmd);
                    item(ui, "Larger", "Ctrl+=", "zoom_in", &mut cmd);
                    item(ui, "Smaller", "Ctrl+-", "zoom_out", &mut cmd);
                    item(ui, "Reset Size", "Ctrl+0", "font_reset", &mut cmd);
                });
            });

            // ---------------------------------------------------------------- Help
            ui.menu_button("Help", |ui| {
                item(ui, "Documentation", "", "help", &mut cmd);
                item(ui, "Changelog…", "", "changelog", &mut cmd);
                item(ui, "Licenses and Attribution…", "", "licenses", &mut cmd);
                ui.separator();
                item(ui, "About Bojan Text", "", "about", &mut cmd);
            });
        });

        if let Some(c) = cmd {
            self.dispatch(&c, ui.ctx());
        }
    }

    fn build_systems_for_menu(&self) -> Vec<String> {
        let scope = self.docs.get(self.active).map(|d| {
            let name = self.syntax_name_for(d);
            highlight::syntax_by_name(&self.syntax_set, &name).map(|s| s.scope.build_string()).unwrap_or_default()
        }).unwrap_or_default();
        self.build_systems()
            .into_iter()
            .filter(|b| b.selector.split(|c: char| c == ',' || c == '|').any(|s| {
                let s = s.trim().split_whitespace().next().unwrap_or("");
                !s.is_empty() && scope.starts_with(s)
            }))
            .map(|b| b.name)
            .collect()
    }

    /// Run a menu / palette / shortcut command id.
    pub(super) fn dispatch(&mut self, c: &str, ctx: &egui::Context) {
        match c {
            "exit" => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            "palette_cmd" => self.open_palette(PaletteMode::Commands),
            "noop" => {}
            "toggle_line_numbers" => self.s.line_numbers = !self.s.line_numbers,
            "toggle_highlight_line" => self.s.highlight_line = !self.s.highlight_line,
            "toggle_md_sync" => self.s.md_sync_scroll = !self.s.md_sync_scroll,
            "toggle_md_auto" => self.s.md_auto_split = !self.s.md_auto_split,
            "next_result_build" => {
                self.panel = Some(BottomPanel::Build);
                self.run("next_result");
            }
            "prev_result_build" => {
                self.panel = Some(BottomPanel::Build);
                self.run("prev_result");
            }
            "new_build_system" => self.open_generated(
                "untitled.sublime-build",
                "{\n\t\"cmd\": [\"make\"],\n\t\"selector\": \"source.c\",\n\t\"file_regex\": \"^(..[^:]*):([0-9]+):?([0-9]+)?:? (.*)$\"\n}\n".into(),
                false,
            ),
            _ if c.starts_with("open_recent:") => {
                let p = c["open_recent:".len()..].to_string();
                self.open_path(Path::new(&p));
            }
            _ if c.starts_with("syntax:") => {
                if let Some(d) = self.docs.get_mut(self.active) {
                    d.syntax_override = Some(c[7..].to_string());
                    d.view.hl.invalidate();
                }
            }
            _ if c.starts_with("build_sys:") => {
                let name = c[10..].to_string();
                self.build.choice = Some((name, None));
            }
            _ if c.starts_with("reopen_enc:") || c.starts_with("save_enc:") => {
                let (reopen, label) = if let Some(l) = c.strip_prefix("reopen_enc:") { (true, l) } else { (false, &c[9..]) };
                let Some(enc) = Encoding::ALL.into_iter().find(|e| e.label() == label) else { return };
                if reopen {
                    if let Some(p) = self.docs.get(self.active).and_then(|d| d.path.clone()) {
                        if let Ok(bytes) = std::fs::read(&p) {
                            let (t, _) = features::decode(&bytes, Some(enc));
                            let t = t.replace("\r\n", "\n");
                            let d = &mut self.docs[self.active];
                            d.text = t.clone();
                            d.saved_text = t;
                            d.saved_hash = hash_text(&d.text);
                            d.encoding = enc;
                        }
                    }
                } else if let Some(d) = self.docs.get_mut(self.active) {
                    d.encoding = enc;
                    let i = self.active;
                    self.save_doc(i, false);
                }
            }
            other => self.run(other),
        }
    }
}

pub fn key_bindings_doc() -> String {
    let rows: &[(&str, &str)] = &[
        ("Command palette", "Ctrl+Shift+P"),
        ("Goto anything / symbol / line", "Ctrl+P / Ctrl+R / Ctrl+G"),
        ("Goto symbol in project", "Ctrl+Shift+R"),
        ("Goto definition / reference", "F12 / Shift+F12"),
        ("Jump back / forward", "Alt+- / Alt+Shift+-"),
        ("Next / previous file", "Ctrl+PageDown / Ctrl+PageUp"),
        ("File stack", "Ctrl+Tab / Ctrl+Shift+Tab"),
        ("Tab 1–10", "Alt+1 … Alt+0"),
        ("New / open / save / save as", "Ctrl+N / Ctrl+O / Ctrl+S / Ctrl+Shift+S"),
        ("Close file / window", "Ctrl+W / Ctrl+Shift+W"),
        ("New window", "Ctrl+Shift+N"),
        ("Undo / redo", "Ctrl+Z / Ctrl+Y"),
        ("Cut / copy / paste (whole line when nothing selected)", "Ctrl+X / Ctrl+C / Ctrl+V"),
        ("Paste and indent / paste from history", "Ctrl+Shift+V / Ctrl+K, Ctrl+V"),
        ("Indent / unindent", "Ctrl+] / Ctrl+[  (or Tab / Shift+Tab on a selection)"),
        ("Swap line up / down", "Ctrl+Shift+Up / Ctrl+Shift+Down"),
        ("Duplicate / delete / join lines", "Ctrl+Shift+D / Ctrl+Shift+K / Ctrl+J"),
        ("Insert line after / before", "Ctrl+Enter / Ctrl+Shift+Enter"),
        ("Delete word forward / back", "Ctrl+Del / Ctrl+Backspace"),
        ("Delete to end / beginning of line", "Ctrl+K, Ctrl+K / Ctrl+K, Ctrl+Backspace"),
        ("Transpose", "Ctrl+T"),
        ("Toggle comment / block comment", "Ctrl+/ / Ctrl+Shift+/"),
        ("Close tag / expand to tag / wrap with tag", "Alt+. / Ctrl+Shift+A / Alt+Shift+W"),
        ("Mark: set / select / delete / swap / clear / yank", "Ctrl+K then Ctrl+Space / A / W / X / G / Y"),
        ("Upper / lower case", "Ctrl+K, Ctrl+U / Ctrl+K, Ctrl+L"),
        ("Wrap paragraph", "Alt+Q"),
        ("Show completions", "Ctrl+Space"),
        ("Sort lines / case sensitive", "F9 / Ctrl+F9"),
        ("Expand selection / to line / word / brackets / indentation", "Ctrl+Shift+Space / Ctrl+L / Ctrl+D / Ctrl+Shift+M / Ctrl+Shift+J"),
        ("Find / replace / incremental", "Ctrl+F / Ctrl+H / Ctrl+I"),
        ("Find next / previous", "F3 / Shift+F3"),
        ("Replace next", "Ctrl+Shift+H"),
        ("Quick find / quick add next", "Ctrl+F3 / Ctrl+D"),
        ("Use selection for find / replace", "Ctrl+E / Ctrl+Shift+E"),
        ("Find in files / next result / previous", "Ctrl+Shift+F / F4 / Shift+F4"),
        ("Bookmarks: toggle / next / prev / clear", "Ctrl+F2 / F2 / Shift+F2 / Ctrl+Shift+F2"),
        ("Matching bracket", "Ctrl+M"),
        ("Scroll line up / down", "Ctrl+Up / Ctrl+Down"),
        ("Side bar", "Ctrl+K, Ctrl+B"),
        ("Console", "Ctrl+`"),
        ("Full screen / distraction free", "F11 / Shift+F11"),
        ("Word wrap", "Alt+Z"),
        ("Build / build with / cancel", "Ctrl+B / Ctrl+Shift+B / Ctrl+Break"),
        ("Record / play macro", "Ctrl+Q / Ctrl+Shift+Q"),
        ("Show scope name", "Ctrl+Alt+Shift+P"),
        ("Markdown preview split / browser", "Alt+M / Alt+B"),
        ("Edit color at cursor", "Alt+C (or Alt+Click a color)"),
        ("Font larger / smaller / reset", "Ctrl+= / Ctrl+- / Ctrl+0"),
    ];
    let mut s = String::from("# Key Bindings\n\nThese follow the original editor's Windows key bindings.\n\n| Action | Keys |\n|---|---|\n");
    for (a, k) in rows {
        s.push_str(&format!("| {a} | {k} |\n"));
    }
    s
}

pub fn mouse_bindings_doc() -> String {
    "# Mouse Bindings\n\n| Action | Mouse |\n|---|---|\n| Place the cursor | Click |\n| Select words / lines | Double / triple click |\n| Extend the selection | Shift+Click |\n| Edit a color code | Alt+Click the color (or click its dot in the margin) |\n| Close a tab | Middle click the tab |\n| Scroll the file | Click or drag on the minimap |\n| Open a result | Click it in Find Results / Build Results |\n".into()
}

pub fn help_doc() -> String {
    "# Bojan Text Help\n\n## Getting around\n- **Ctrl+Shift+P** opens the command palette: every menu command is in it.\n- **Ctrl+P** jumps to any open file or file in your folders; **Ctrl+R** to a function or heading; **Ctrl+G** to a line.\n- **Alt+-** jumps back to where you were.\n\n## Colors\nAny color code (`#ff8800`, `rgb(…)`, `hsl(…)`, `oklch(…)`, `0xFF8800`, `Color32::from_rgb(…)`, named colors in style files) is shown in its own color. **Alt+Click** one to change it with a picker.\n\n## Markdown\n**Alt+M** shows the preview next to the text, **Alt+B** opens it in your browser and keeps it up to date while you type.\n\n## Sublime Text packages\nLanguages, color schemes, snippets and build systems are read from your Sublime Text install (Tools ▸ Sublime Packages). Python plugins are listed but can't run here.\n\n## Language servers\nInstall a language server (rust-analyzer, pyright, typescript-language-server, clangd…) and open a saved file: errors appear underlined and in the Problems panel, F12 goes to a definition, Shift+F12 finds references.\n\n## Building\n**Ctrl+B** runs the build system that fits the file (Tools ▸ Build System). Click a result line or press F4 to jump to it.\n\n## Settings\nPreferences ▸ Settings opens the settings file; save it to apply. Preferences ▸ Customize Color Scheme makes an editable copy of the current colors.\n".into()
}

pub fn changelog_doc() -> String {
    "# Changelog\n\n## 0.2\n- Full menu set: File, Edit, Selection, Find, View, Goto, Tools, Project, Preferences, Help\n- Find in files, replace next, regex search, quick find\n- Line tools: swap, join, duplicate, indent, insert before/after, transpose\n- Case conversion including snake_case, camelCase and kebab-case\n- Wrap paragraph, sort / reverse / unique / shuffle lines\n- Marks, bookmarks, jump back / forward, changed-line markers\n- Goto symbol (file and project), references\n- Build systems from Sublime packages, build results panel\n- Macros, snippets, projects (.sublime-project), settings file, encodings and line endings\n- Full screen and distraction free mode, rulers, wrap column\n\n## 0.1\n- First version: editor, themes, colorizer, Markdown preview, Sublime packages, language servers\n".into()
}

pub fn licenses_doc() -> String {
    "# Licenses and Attribution\n\nBojan Text is built with these open-source libraries:\n\n| Library | Used for | License |\n|---|---|---|\n| egui / eframe | window and interface | MIT / Apache-2.0 |\n| syntect | syntax highlighting | MIT |\n| fancy-regex | regular expressions | MIT |\n| pulldown-cmark | Markdown | MIT |\n| yaml-rust | reading syntax files | MIT / Apache-2.0 |\n| serde, serde_json | settings | MIT / Apache-2.0 |\n| zip | reading .sublime-package files | MIT |\n| image | screenshots | MIT / Apache-2.0 |\n| rfd | file dialogs | MIT |\n| arboard | clipboard | MIT / Apache-2.0 |\n\nLanguage and color-scheme files are read at run time from your own Sublime Text installation; they are not copied into this program.\n".into()
}
