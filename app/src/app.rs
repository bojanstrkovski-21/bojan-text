use crate::colors::{ColorMatch, NamedMode, Rgba};
use crate::editor::{self, byte_to_char, char_to_byte, hash_text, ColorStyle, ViewCfg, ViewState};
use crate::highlight::{self, PackageReport, ScanResult, Scheme};
use crate::lsp::{self, Diag, LspClient, LspEvent, ServerSpec};
use crate::markdown::{self, MdStyle, MdView, PreviewServer};
use crate::theme::{self, mix, Palette, TabStyle, ThemeOptions};
use eframe::egui::{self, Color32, FontId, Key, KeyboardShortcut, Modifiers, RichText};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use syntect::parsing::SyntaxSet;

mod features;
mod keys;
mod menus;
mod palette;
mod ui;

use features::{BottomPanel, BuildState, Encoding, FindInFiles, LineEnding};

pub const APP_NAME: &str = "Bojan Text";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MdMode {
    Off,
    Split,
    Preview,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub scheme: String,
    pub theme: ThemeOptions,
    pub show_sidebar: bool,
    pub show_minimap: bool,
    pub word_wrap: bool,
    pub tab_size: usize,
    pub translate_tabs: bool,
    pub highlight_line: bool,
    pub line_numbers: bool,
    pub colorizer: bool,
    pub color_style: ColorStyle,
    pub named_colors: NamedMode,
    pub gutter_swatches: bool,
    pub md_auto_split: bool,
    pub md_sync_scroll: bool,
    pub use_sublime_packages: bool,
    pub extra_package_dirs: Vec<String>,
    pub lsp_enabled: bool,
    pub recent: Vec<String>,
    pub show_problems: bool,
    pub show_tabs: bool,
    pub show_status_bar: bool,
    pub show_menu: bool,
    pub show_open_files: bool,
    pub rulers: Vec<usize>,
    pub wrap_column: usize,
    pub save_all_on_build: bool,
    pub font_file: Option<String>,
    pub recent_projects: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            scheme: "Bojan Night".into(),
            theme: ThemeOptions::default(),
            show_sidebar: true,
            show_minimap: true,
            word_wrap: false,
            tab_size: 4,
            translate_tabs: false,
            highlight_line: true,
            line_numbers: true,
            colorizer: true,
            color_style: ColorStyle::Fill,
            named_colors: NamedMode::StyleFiles,
            gutter_swatches: true,
            md_auto_split: true,
            md_sync_scroll: true,
            use_sublime_packages: true,
            extra_package_dirs: Vec::new(),
            lsp_enabled: true,
            recent: Vec::new(),
            show_problems: false,
            show_tabs: true,
            show_status_bar: true,
            show_menu: true,
            show_open_files: true,
            rulers: Vec::new(),
            wrap_column: 0,
            save_all_on_build: true,
            font_file: None,
            recent_projects: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct SessionDoc {
    pub path: Option<String>,
    pub title: String,
    pub text: Option<String>,
    pub md_mode: Option<MdMode>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Session {
    pub docs: Vec<SessionDoc>,
    pub active: usize,
    pub folder: Option<String>,
    #[serde(default)]
    pub extra_folders: Vec<String>,
    #[serde(default)]
    pub project_file: Option<String>,
}

pub struct Doc {
    pub id: u64,
    pub path: Option<PathBuf>,
    pub title: String,
    pub text: String,
    pub saved_hash: u64,
    pub syntax_override: Option<String>,
    pub md_mode: MdMode,
    pub view: ViewState,
    pub md: MdView,
    pub preview_scroll: f32,
    pub preview_content_h: f32,
    pub preview_view_h: f32,
    pub line_ending: LineEnding,
    pub encoding: Encoding,
    /// text as last saved (for changed-line markers and Revert Modification)
    pub saved_text: String,
    pub bookmarks: Vec<usize>,
    pub mod_cache: (u64, Option<(usize, usize)>),
    /// split pane this file is shown in (View ▸ Layout)
    pub group: usize,
    /// non-zero: this is a split copy (File ▸ Split View) kept in sync with its twin
    pub clone_id: u64,
}

impl Doc {
    fn dirty(&self) -> bool {
        hash_text(&self.text) != self.saved_hash
    }
    fn is_markdown(&self) -> bool {
        self.path
            .as_ref()
            .and_then(|p| p.extension())
            .map(|e| {
                let e = e.to_string_lossy().to_ascii_lowercase();
                e == "md" || e == "markdown" || e == "mdown"
            })
            .unwrap_or(false)
            || self.syntax_override.as_deref() == Some("Markdown")
    }
    fn ext(&self) -> String {
        self.path.as_ref().and_then(|p| p.extension()).map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum PaletteMode {
    Commands,
    Files,
    GotoLine,
    Syntax,
    Scheme,
    PasteHistory,
    Complete,
    Symbols,
    ProjectSymbols,
    Snippets,
    BuildWith,
    PackageFiles,
    Projects,
    Fonts,
}

#[derive(Default)]
struct FindState {
    open: bool,
    replace_open: bool,
    query: String,
    replace: String,
    case: bool,
    regex: bool,
    /// whole_word
    whole: bool,
    /// wrap (default on)
    wrap: bool,
    /// highlight matches (default on)
    highlight: bool,
    /// in_selection: search only inside this char range
    in_sel: Option<(usize, usize)>,
    focus: bool,
}

struct ColorEdit {
    doc_id: u64,
    start: usize,
    end: usize,
    original: String,
    m: ColorMatch,
    color: Color32,
}

struct Shot {
    path: PathBuf,
    frame: u32,
    requested: bool,
    image: Arc<Mutex<Option<Arc<egui::ColorImage>>>>,
}

pub struct App {
    s: Settings,
    docs: Vec<Doc>,
    active: usize,
    next_id: u64,
    syntax_set: Arc<SyntaxSet>,
    schemes: Vec<Scheme>,
    scheme_idx: usize,
    pal: Palette,
    applied: Option<(String, ThemeOptions)>,
    scan_rx: Option<Receiver<ScanResult>>,
    pkg_report: Option<PackageReport>,
    set_gen: u64,
    folder: Option<PathBuf>,
    expanded: HashSet<PathBuf>,
    dir_cache: HashMap<PathBuf, Vec<(PathBuf, bool)>>,
    palette: Option<PaletteMode>,
    pal_query: String,
    pal_sel: usize,
    pal_focus: bool,
    find: FindState,
    win_theme: bool,
    win_packages: bool,
    win_lsp: bool,
    win_about: bool,
    win_colors: bool,
    color_edit: Option<ColorEdit>,
    servers: Vec<ServerSpec>,
    clients: HashMap<String, LspClient>,
    lsp_errors: HashMap<String, String>,
    diags: HashMap<String, Vec<Diag>>,
    hover_popup: Option<(String, egui::Pos2)>,
    hover_sent: Option<(u64, usize)>,
    hover_since: Option<(u64, usize, f64)>,
    preview_server: Option<PreviewServer>,
    live_browser: Option<u64>,
    status: Option<(String, Instant)>,
    shot: Option<Shot>,
    code_cache: HashMap<(u64, String), egui::text::LayoutJob>,
    file_index: Option<(PathBuf, Vec<PathBuf>)>,
    frame: u64,
    // full menu set
    extra_folders: Vec<PathBuf>,
    project_file: Option<PathBuf>,
    console: Vec<String>,
    panel: Option<BottomPanel>,
    fif: FindInFiles,
    build: BuildState,
    extras: highlight::Extras,
    mark: Option<(u64, usize)>,
    yank: String,
    jumps: Vec<(u64, usize)>,
    jump_idx: usize,
    mru: Vec<u64>,
    paste_history: Vec<String>,
    recording: Option<Vec<egui::Event>>,
    last_macro: Vec<egui::Event>,
    play_macro: Option<Vec<egui::Event>>,
    pending_events: Vec<egui::Event>,
    viewport_cmds: Vec<egui::ViewportCommand>,
    fullscreen: bool,
    distraction: Option<(bool, bool, bool, bool, bool)>,
    fonts_dirty: bool,
    scope_popup: Option<String>,
    chord_k: Option<Instant>,
    /// palette lists computed when the palette opens: (label, id, detail)
    pal_items: Vec<(String, String, String)>,
    project_symbols_rx: Option<Receiver<Vec<(String, String, String)>>>,
    // syntax chooser (status bar)
    syntax_popup: bool,
    syntax_anchor: egui::Rect,
    syntax_scrolled: bool,
    // split panes (View ▸ Layout / Groups)
    groups: usize,
    cols: usize,
    max_columns: usize,
    focus_group: usize,
    group_active: Vec<Option<u64>>,
    last_active_id: Option<u64>,
    clone_sync: HashMap<u64, u64>,
    next_clone: u64,
}

const SAMPLE_WELCOME: &str = include_str!("../assets/samples/Welcome.md");
const SAMPLE_CSS: &str = include_str!("../assets/samples/colors.css");
const SAMPLE_RS: &str = include_str!("../assets/samples/palette.rs");

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, screenshot: Option<PathBuf>, files: Vec<PathBuf>) -> Self {
        theme::install_fonts(&cc.egui_ctx);
        let restore = screenshot.is_none();
        let s: Settings = if restore { cc.storage.and_then(|st| eframe::get_value(st, "settings")).unwrap_or_default() } else { Settings::default() };
        let session: Option<Session> = if restore { cc.storage.and_then(|st| eframe::get_value(st, "session")) } else { None };
        let schemes = highlight::builtin_schemes();
        let scheme_idx = schemes.iter().position(|x| x.name == s.scheme).unwrap_or(0);
        let pal = theme::palette_from(&schemes[scheme_idx].theme, &s.theme);
        let mut app = Self {
            s,
            docs: Vec::new(),
            active: 0,
            next_id: 1,
            syntax_set: Arc::new(SyntaxSet::load_defaults_newlines()),
            schemes,
            scheme_idx,
            pal,
            applied: None,
            scan_rx: None,
            pkg_report: None,
            set_gen: 0,
            folder: None,
            expanded: HashSet::new(),
            dir_cache: HashMap::new(),
            palette: None,
            pal_query: String::new(),
            pal_sel: 0,
            pal_focus: false,
            find: FindState { wrap: true, highlight: true, ..Default::default() },
            win_theme: false,
            win_packages: false,
            win_lsp: false,
            win_about: false,
            win_colors: false,
            color_edit: None,
            servers: Vec::new(),
            clients: HashMap::new(),
            lsp_errors: HashMap::new(),
            diags: HashMap::new(),
            hover_popup: None,
            hover_sent: None,
            hover_since: None,
            preview_server: None,
            live_browser: None,
            status: None,
            shot: screenshot.map(|p| Shot { path: p, frame: 0, requested: false, image: Arc::new(Mutex::new(None)) }),
            code_cache: HashMap::new(),
            file_index: None,
            frame: 0,
            extra_folders: Vec::new(),
            project_file: None,
            console: Vec::new(),
            panel: None,
            fif: FindInFiles::default(),
            build: BuildState::default(),
            extras: highlight::Extras::default(),
            mark: None,
            yank: String::new(),
            jumps: Vec::new(),
            jump_idx: 0,
            mru: Vec::new(),
            paste_history: Vec::new(),
            recording: None,
            last_macro: Vec::new(),
            play_macro: None,
            pending_events: Vec::new(),
            viewport_cmds: Vec::new(),
            fullscreen: false,
            distraction: None,
            fonts_dirty: false,
            scope_popup: None,
            chord_k: None,
            pal_items: Vec::new(),
            project_symbols_rx: None,
            syntax_popup: false,
            syntax_anchor: egui::Rect::NOTHING,
            syntax_scrolled: false,
            groups: 1,
            cols: 1,
            max_columns: 3,
            focus_group: 0,
            group_active: vec![None],
            last_active_id: None,
            clone_sync: HashMap::new(),
            next_clone: 1,
        };
        if app.s.font_file.is_some() {
            theme::install_fonts_with(&cc.egui_ctx, app.s.font_file.as_deref());
        }
        // user color schemes (Preferences ▸ Customize Color Scheme)
        if let Ok(rd) = std::fs::read_dir(features::config_dir().join("Schemes")) {
            for e in rd.flatten() {
                let p = e.path();
                if p.extension().map(|x| x == "sublime-color-scheme").unwrap_or(false) {
                    if let Ok(t) = std::fs::read_to_string(&p) {
                        let stem = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                        if let Some(sc) = highlight::load_sublime_scheme(&t, &stem, "User") {
                            app.schemes.push(sc);
                        }
                    }
                }
            }
            app.scheme_idx = app.schemes.iter().position(|x| x.name == app.s.scheme).unwrap_or(app.scheme_idx);
        }
        let session = if std::env::args().any(|a| a == "--new-window") { None } else { session };
        // Sublime packages + language servers are discovered in the background.
        if app.s.use_sublime_packages && app.shot.is_none() {
            app.start_scan();
        }
        if app.shot.is_none() {
            app.servers = lsp::discover_servers();
        }
        if let Some(sess) = session {
            for d in sess.docs {
                match (&d.path, &d.text) {
                    (Some(p), None) => app.open_path(Path::new(p)),
                    (p, Some(t)) => {
                        let id = app.new_doc(&d.title, t.clone(), p.as_ref().map(PathBuf::from));
                        if let Some(doc) = app.docs.iter_mut().find(|x| x.id == id) {
                            if let Some(p) = &d.path {
                                let saved = std::fs::read(p).map(|b| features::decode(&b, None).0.replace("\r\n", "\n")).unwrap_or_default();
                                doc.saved_hash = hash_text(&saved);
                                doc.saved_text = saved;
                            }
                            if let Some(m) = d.md_mode {
                                doc.md_mode = m;
                            }
                        }
                    }
                    _ => {}
                }
            }
            app.active = sess.active.min(app.docs.len().saturating_sub(1));
            app.folder = sess.folder.map(PathBuf::from).filter(|p| p.is_dir());
            app.extra_folders = sess.extra_folders.into_iter().map(PathBuf::from).filter(|p| p.is_dir()).collect();
            app.project_file = sess.project_file.map(PathBuf::from).filter(|p| p.is_file());
        }
        for f in files {
            if f.is_dir() {
                app.folder = Some(f);
            } else {
                app.open_path(&f);
            }
        }
        if app.docs.is_empty() {
            let w = app.new_doc("Welcome.md", SAMPLE_WELCOME.into(), None);
            app.new_doc("colors.css", SAMPLE_CSS.into(), None);
            app.new_doc("palette.rs", SAMPLE_RS.into(), None);
            if let Some(d) = app.docs.iter_mut().find(|d| d.id == w) {
                d.syntax_override = Some("Markdown".into());
                d.md_mode = MdMode::Split;
            }
            for d in app.docs.iter_mut() {
                d.saved_hash = hash_text(&d.text);
            }
            app.active = 0;
        }
        app
    }

    fn start_scan(&mut self) {
        let mut dirs = highlight::default_package_dirs();
        for d in &self.s.extra_package_dirs {
            dirs.push(PathBuf::from(d));
        }
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let r = highlight::scan_sublime_packages_cached(&dirs);
            let _ = tx.send(r);
        });
        self.scan_rx = Some(rx);
    }

    fn new_doc(&mut self, title: &str, text: String, path: Option<PathBuf>) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let mut d = Doc {
            id,
            path,
            title: title.to_string(),
            saved_hash: hash_text(&text),
            text,
            syntax_override: None,
            md_mode: MdMode::Off,
            view: ViewState::default(),
            md: MdView::default(),
            preview_scroll: 0.0,
            preview_content_h: 0.0,
            preview_view_h: 0.0,
            line_ending: if cfg!(windows) { LineEnding::Windows } else { LineEnding::Unix },
            encoding: Encoding::Utf8,
            saved_text: String::new(),
            bookmarks: Vec::new(),
            mod_cache: (0, None),
            group: self.focus_group.min(self.groups.saturating_sub(1)),
            clone_id: 0,
        };
        if d.path.is_some() {
            d.saved_text = d.text.clone();
        }
        if d.is_markdown() && self.s.md_auto_split {
            d.md_mode = MdMode::Split;
        }
        d.view.focus = true;
        self.docs.push(d);
        self.active = self.docs.len() - 1;
        id
    }

    pub fn open_path(&mut self, p: &Path) {
        if let Some(i) = self.docs.iter().position(|d| d.path.as_deref() == Some(p)) {
            self.active = i;
            return;
        }
        match std::fs::read(p) {
            Ok(bytes) => {
                let (raw, enc) = features::decode(&bytes, None);
                let le = features::detect_line_ending(&raw);
                let text = raw.replace("\r\n", "\n").replace('\r', "\n");
                let title = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                let id = self.new_doc(&title, text, Some(p.to_path_buf()));
                if let Some(d) = self.docs.iter_mut().find(|d| d.id == id) {
                    d.encoding = enc;
                    d.line_ending = le;
                }
                let ps = p.display().to_string();
                self.s.recent.retain(|r| r != &ps);
                self.s.recent.insert(0, ps);
                self.s.recent.truncate(12);
            }
            Err(e) => self.flash(format!("Could not open {}: {e}", p.display())),
        }
    }

    fn flash(&mut self, msg: String) {
        self.status = Some((msg, Instant::now()));
    }

    fn save_doc(&mut self, i: usize, save_as: bool) {
        let Some(d) = self.docs.get(i) else { return };
        let path = if save_as || d.path.is_none() {
            let mut dlg = rfd::FileDialog::new().set_file_name(&d.title);
            if let Some(f) = &self.folder {
                dlg = dlg.set_directory(f);
            }
            dlg.save_file()
        } else {
            d.path.clone()
        };
        let Some(path) = path else { return };
        let nl = match self.docs[i].line_ending {
            LineEnding::Windows => "\r\n",
            LineEnding::Unix => "\n",
            LineEnding::Mac => "\r",
        };
        let text = self.docs[i].text.replace('\n', nl);
        let bytes = features::encode(&text, self.docs[i].encoding);
        match std::fs::write(&path, bytes) {
            Ok(_) => {
                let d = &mut self.docs[i];
                d.saved_hash = hash_text(&d.text);
                d.saved_text = d.text.clone();
                d.title = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                d.path = Some(path.clone());
                d.view.hl.invalidate();
                // split copies of the same file share the saved state
                let (cid, h, t, title) = (d.clone_id, d.saved_hash, d.saved_text.clone(), d.title.clone());
                if cid != 0 {
                    for o in self.docs.iter_mut().filter(|o| o.clone_id == cid) {
                        o.saved_hash = h;
                        o.saved_text = t.clone();
                        o.path = Some(path.clone());
                        o.title = title.clone();
                    }
                }
                self.flash(format!("Saved {}", path.display()));
                let key = lsp::norm_key(&path);
                for c in self.clients.values_mut() {
                    if c.opened.contains(&key) {
                        c.did_save(&path);
                    }
                }
                self.after_save(&path);
            }
            Err(e) => self.flash(format!("Save failed: {e}")),
        }
    }

    fn close_doc(&mut self, i: usize) {
        if i < self.docs.len() {
            if self.docs[i].dirty() {
                let res = rfd::MessageDialog::new()
                    .set_title(APP_NAME)
                    .set_description(format!("Save changes to {} before closing?", self.docs[i].title))
                    .set_buttons(rfd::MessageButtons::YesNoCancel)
                    .show();
                match res {
                    rfd::MessageDialogResult::Yes => self.save_doc(i, false),
                    rfd::MessageDialogResult::No => {}
                    _ => return,
                }
            }
            self.docs.remove(i);
            if self.active >= self.docs.len() {
                self.active = self.docs.len().saturating_sub(1);
            }
        }
    }

    fn scheme(&self) -> &Scheme {
        &self.schemes[self.scheme_idx.min(self.schemes.len() - 1)]
    }

    fn syntax_name_for(&self, d: &Doc) -> String {
        if let Some(o) = &d.syntax_override {
            if highlight::syntax_by_name(&self.syntax_set, o).is_some() {
                return o.clone();
            }
        }
        let first = d.text.lines().next().unwrap_or("");
        let path = d.path.clone().or_else(|| Some(PathBuf::from(&d.title)));
        highlight::find_syntax(&self.syntax_set, path.as_deref(), first).name.clone()
    }

    fn set_scheme(&mut self, idx: usize) {
        self.scheme_idx = idx;
        self.s.scheme = self.schemes[idx].name.clone();
        self.code_cache.clear();
    }

    fn named_colors_for(&self, d: &Doc) -> bool {
        let ext = if d.path.is_some() { d.ext() } else { d.title.rsplit('.').next().unwrap_or("").to_ascii_lowercase() };
        match self.s.named_colors {
            NamedMode::Off => false,
            NamedMode::Everywhere => true,
            NamedMode::StyleFiles => matches!(ext.as_str(), "css" | "scss" | "sass" | "less" | "styl" | "html" | "htm" | "vue" | "svelte" | "svg" | "xml"),
        }
    }

    // -----------------------------------------------------------------------
    // commands

    fn commands(&self) -> Vec<(&'static str, &'static str, &'static str)> {
        vec![
            ("New File", "new", "Ctrl+N"),
            ("Open File…", "open", "Ctrl+O"),
            ("Open Folder…", "open_folder", ""),
            ("Save", "save", "Ctrl+S"),
            ("Save As…", "save_as", "Ctrl+Shift+S"),
            ("Save All", "save_all", ""),
            ("Close File", "close", "Ctrl+W"),
            ("Find…", "find", "Ctrl+F"),
            ("Replace…", "replace", "Ctrl+H"),
            ("Goto Anything", "goto_anything", "Ctrl+P"),
            ("Goto Line…", "goto_line", "Ctrl+G"),
            ("Goto Definition", "goto_def", "F12"),
            ("View: Toggle Side Bar", "toggle_sidebar", "Ctrl+K, Ctrl+B"),
            ("View: Toggle Minimap", "toggle_minimap", ""),
            ("View: Toggle Word Wrap", "toggle_wrap", "Alt+Z"),
            ("View: Toggle Problems Panel", "toggle_problems", ""),
            ("Markdown: Preview Split / Off", "md_split", "Alt+M"),
            ("Markdown: Preview Only", "md_preview", ""),
            ("Markdown: Open Preview in Browser (live)", "md_browser", "Alt+B"),
            ("Markdown: Export HTML…", "md_export", ""),
            ("Colors: List Color Codes in File", "colors_list", ""),
            ("Colors: Toggle Colorizer", "toggle_colorizer", ""),
            ("Colors: Edit Color at Cursor", "color_at_cursor", "Alt+C"),
            ("Preferences: Theme & Appearance…", "theme", ""),
            ("Preferences: Select Color Scheme…", "pick_scheme", ""),
            ("Set Syntax…", "pick_syntax", ""),
            ("Sublime Packages: Show Loaded Packages…", "packages", ""),
            ("Sublime Packages: Rescan", "rescan", ""),
            ("Language Servers…", "lsp", ""),
            ("Language Servers: Restart All", "lsp_restart", ""),
            ("Edit: Toggle Comment", "comment", "Ctrl+/"),
            ("Edit: Duplicate Line", "dup_line", "Ctrl+Shift+D"),
            ("Edit: Delete Line", "del_line", "Ctrl+Shift+K"),
            ("Edit: Sort Lines", "sort_lines", "F9"),
            ("Edit: Convert to Upper Case", "upper", ""),
            ("Edit: Convert to Lower Case", "lower", ""),
            ("Edit: Indentation to Spaces", "to_spaces", ""),
            ("Edit: Indentation to Tabs", "to_tabs", ""),
            ("View: Zoom In", "zoom_in", "Ctrl+="),
            ("View: Zoom Out", "zoom_out", "Ctrl+-"),
            ("About Bojan Text", "about", ""),
            ("File: New Window", "new_window", "Ctrl+Shift+N"),
            ("File: Revert File", "revert", ""),
            ("File: Close All Files", "close_all", ""),
            ("File: Print…", "print", ""),
            ("Edit: Undo", "undo", "Ctrl+Z"),
            ("Edit: Redo", "redo", "Ctrl+Y"),
            ("Edit: Copy as HTML", "copy_html", ""),
            ("Edit: Paste and Indent", "paste_indent", "Ctrl+Shift+V"),
            ("Edit: Paste from History", "paste_history", "Ctrl+K, Ctrl+V"),
            ("Edit: Indent", "indent", "Ctrl+]"),
            ("Edit: Unindent", "unindent", "Ctrl+["),
            ("Edit: Swap Line Up", "swap_up", "Ctrl+Shift+Up"),
            ("Edit: Swap Line Down", "swap_down", "Ctrl+Shift+Down"),
            ("Edit: Join Lines", "join_lines", "Ctrl+J"),
            ("Edit: Toggle Block Comment", "block_comment", "Ctrl+Shift+/"),
            ("Edit: Insert Line Before", "line_before", "Ctrl+Shift+Enter"),
            ("Edit: Insert Line After", "line_after", "Ctrl+Enter"),
            ("Edit: Delete to End of Line", "del_to_end", "Ctrl+K, Ctrl+K"),
            ("Edit: Delete to Beginning of Line", "del_to_start", "Ctrl+K, Ctrl+Backspace"),
            ("Edit: Transpose", "transpose", "Ctrl+T"),
            ("Edit: Revert Modification", "revert_mod", "Ctrl+K, Ctrl+Z"),
            ("Edit: Close Tag", "close_tag", "Alt+."),
            ("Edit: Wrap Selection With Tag", "wrap_tag", "Alt+Shift+W"),
            ("Edit: Set Mark", "set_mark", "Ctrl+K, Ctrl+Space"),
            ("Edit: Select to Mark", "select_to_mark", "Ctrl+K, Ctrl+A"),
            ("Edit: Delete to Mark", "delete_to_mark", "Ctrl+K, Ctrl+W"),
            ("Edit: Swap with Mark", "swap_with_mark", "Ctrl+K, Ctrl+X"),
            ("Edit: Clear Mark", "clear_mark", "Ctrl+K, Ctrl+G"),
            ("Edit: Yank", "yank", "Ctrl+K, Ctrl+Y"),
            ("Convert Case: Title Case", "title_case", ""),
            ("Convert Case: Swap Case", "swap_case", ""),
            ("Convert Case: lowerCamelCase", "camel", ""),
            ("Convert Case: UpperCamelCase", "pascal", ""),
            ("Convert Case: snake_case", "snake", ""),
            ("Convert Case: kebab-case", "kebab", ""),
            ("Edit: Wrap Paragraph at Ruler", "wrap_at:0", "Alt+Q"),
            ("Edit: Show Completions", "complete", "Ctrl+Space"),
            ("Edit: Sort Lines (Case Sensitive)", "sort_case", "Ctrl+F9"),
            ("Permute Lines: Reverse", "reverse_lines", ""),
            ("Permute Lines: Unique", "unique_lines", ""),
            ("Permute Lines: Shuffle", "shuffle_lines", ""),
            ("Selection: Expand Selection", "expand_sel", "Ctrl+Shift+Space"),
            ("Selection: Expand to Line", "expand_line", "Ctrl+L"),
            ("Selection: Expand to Word", "expand_word", "Ctrl+D"),
            ("Selection: Expand to Paragraph", "expand_para", ""),
            ("Selection: Expand to Scope", "expand_scope", ""),
            ("Selection: Expand to Brackets", "expand_brackets", "Ctrl+Shift+M"),
            ("Selection: Expand to Indentation", "expand_indent", "Ctrl+Shift+J"),
            ("Selection: Expand to Tag", "expand_tag", "Ctrl+Shift+A"),
            ("Find: Find Next", "find_next", "F3"),
            ("Find: Find Previous", "find_prev", "Shift+F3"),
            ("Find: Replace Next", "replace_next", "Ctrl+Shift+H"),
            ("Find: Quick Find", "quick_find", "Ctrl+F3"),
            ("Find: Use Selection for Find", "use_sel_find", "Ctrl+E"),
            ("Find: Find in Files…", "find_in_files", "Ctrl+Shift+F"),
            ("Find: Next Result", "next_result", "F4"),
            ("View: Toggle Tabs", "toggle_tabs", ""),
            ("View: Toggle Status Bar", "toggle_status", ""),
            ("View: Toggle Menu", "toggle_menu", ""),
            ("View: Toggle Open Files in Side Bar", "toggle_open_files", ""),
            ("View: Console", "console", "Ctrl+`"),
            ("View: Full Screen", "fullscreen", "F11"),
            ("View: Distraction Free Mode", "distraction_free", "Shift+F11"),
            ("Indentation: Indent Using Spaces", "toggle_spaces", ""),
            ("Indentation: Guess Settings From Buffer", "detect_indent", ""),
            ("Line Endings: Windows", "line_ending:windows", ""),
            ("Line Endings: Unix", "line_ending:unix", ""),
            ("Ruler: 80", "ruler:80", ""),
            ("Ruler: None", "ruler:0", ""),
            ("Goto Symbol…", "goto_symbol", "Ctrl+R"),
            ("Goto Symbol in Project…", "goto_symbol_project", "Ctrl+Shift+R"),
            ("Goto Reference…", "goto_reference", "Shift+F12"),
            ("Goto: Next Modification", "next_mod", ""),
            ("Goto: Jump Back", "jump_back", "Alt+-"),
            ("Goto: Jump Forward", "jump_forward", "Alt+Shift+-"),
            ("Goto: Switch Header/Implementation", "switch_header", "Alt+O"),
            ("Goto: Jump to Matching Bracket", "matching_bracket", "Ctrl+M"),
            ("Bookmarks: Toggle", "toggle_bookmark", "Ctrl+F2"),
            ("Bookmarks: Next", "next_bookmark", "F2"),
            ("Bookmarks: Previous", "prev_bookmark", "Shift+F2"),
            ("Bookmarks: Clear", "clear_bookmarks", "Ctrl+Shift+F2"),
            ("Snippets…", "snippets", ""),
            ("Build", "build", "Ctrl+B"),
            ("Build With…", "build_with", "Ctrl+Shift+B"),
            ("Cancel Build", "cancel_build", "Ctrl+Break"),
            ("Build Results: Show", "show_build", ""),
            ("Macro: Record / Stop", "record_macro", "Ctrl+Q"),
            ("Macro: Playback", "play_macro", "Ctrl+Shift+Q"),
            ("Macro: Save…", "save_macro", ""),
            ("Developer: New Snippet", "new_snippet", ""),
            ("Developer: New Syntax", "new_syntax", ""),
            ("Developer: View Package File", "view_package_file", ""),
            ("Developer: Show Scope Name", "scope_name", "Ctrl+Alt+Shift+P"),
            ("Project: Open Project…", "open_project", ""),
            ("Project: Switch Project…", "switch_project", "Ctrl+Alt+P"),
            ("Project: Save Project As…", "save_project", ""),
            ("Project: Close Project", "close_project", ""),
            ("Project: Add Folder…", "add_folder", ""),
            ("Project: Refresh Folders", "refresh_folders", ""),
            ("Preferences: Settings", "settings", ""),
            ("Preferences: Key Bindings", "key_bindings", ""),
            ("Preferences: Customize Color Scheme", "customize_scheme", ""),
            ("Preferences: Browse Packages", "browse_packages", ""),
            ("Preferences: Choose Font…", "choose_font", ""),
            ("Preferences: Reset Font Size", "font_reset", "Ctrl+0"),
            ("Help: Documentation", "help", ""),
            ("Help: Changelog", "changelog", ""),
        ]
    }

    fn run(&mut self, id: &str) {
        match id {
            "new" => {
                let n = self.docs.iter().filter(|d| d.path.is_none() && d.title.starts_with("untitled")).count();
                self.new_doc(&if n == 0 { "untitled".to_string() } else { format!("untitled {}", n + 1) }, String::new(), None);
            }
            "open" => {
                let mut dlg = rfd::FileDialog::new();
                if let Some(f) = &self.folder {
                    dlg = dlg.set_directory(f);
                }
                if let Some(files) = dlg.pick_files() {
                    for f in files {
                        self.open_path(&f);
                    }
                }
            }
            "open_folder" => {
                if let Some(f) = rfd::FileDialog::new().pick_folder() {
                    self.folder = Some(f);
                    self.dir_cache.clear();
                    self.file_index = None;
                    self.s.show_sidebar = true;
                }
            }
            "save" => self.save_doc(self.active, false),
            "save_as" => self.save_doc(self.active, true),
            "save_all" => {
                for i in 0..self.docs.len() {
                    if self.docs[i].dirty() && self.docs[i].path.is_some() {
                        self.save_doc(i, false);
                    }
                }
            }
            "close" => self.close_doc(self.active),
            "find" | "replace" => {
                self.find.open = true;
                self.find.replace_open = id == "replace";
                self.fif.open = false;
                self.find.focus = true;
                self.seed_find();
            }
            "goto_anything" => self.open_palette(PaletteMode::Files),
            "goto_line" => self.open_palette(PaletteMode::GotoLine),
            "pick_syntax" => self.open_palette(PaletteMode::Syntax),
            "pick_scheme" => self.open_palette(PaletteMode::Scheme),
            "goto_def" => self.lsp_definition(),
            "toggle_sidebar" => self.s.show_sidebar = !self.s.show_sidebar,
            "toggle_minimap" => self.s.show_minimap = !self.s.show_minimap,
            "toggle_wrap" => self.s.word_wrap = !self.s.word_wrap,
            "toggle_problems" => self.panel = if self.panel == Some(BottomPanel::Problems) { None } else { Some(BottomPanel::Problems) },
            "md_split" => {
                if let Some(d) = self.docs.get_mut(self.active) {
                    d.md_mode = if d.md_mode == MdMode::Split { MdMode::Off } else { MdMode::Split };
                }
            }
            "md_preview" => {
                if let Some(d) = self.docs.get_mut(self.active) {
                    d.md_mode = if d.md_mode == MdMode::Preview { MdMode::Off } else { MdMode::Preview };
                }
            }
            "md_browser" => self.open_browser_preview(),
            "md_export" => self.export_html(),
            "colors_list" => self.win_colors = true,
            "toggle_colorizer" => self.s.colorizer = !self.s.colorizer,
            "color_at_cursor" => {
                if let Some(d) = self.docs.get(self.active) {
                    let ch = d.view.cursor.map(|c| c.0).unwrap_or(0);
                    let b = char_to_byte(&d.text, ch);
                    let hit = d.view.colors.at(b).or_else(|| if b > 0 { d.view.colors.at(b - 1) } else { None }).cloned();
                    let id = d.id;
                    if let Some(m) = hit {
                        self.begin_color_edit(id, m);
                    } else {
                        self.flash("No color code at the cursor".into());
                    }
                }
            }
            "theme" => self.win_theme = true,
            "packages" => self.win_packages = true,
            "rescan" => {
                self.start_scan();
                self.win_packages = true;
            }
            "lsp" => self.win_lsp = true,
            "lsp_restart" => {
                self.clients.clear();
                self.lsp_errors.clear();
                self.diags.clear();
                self.servers = lsp::discover_servers();
            }
            "to_spaces" | "to_tabs" => {
                let ts = self.s.tab_size.max(1);
                let tabs = id == "to_tabs";
                if let Some(d) = self.docs.get_mut(self.active) {
                    let trailing = d.text.ends_with('\n');
                    d.text = d
                        .text
                        .lines()
                        .map(|l| {
                            let ind: String = l.chars().take_while(|c| *c == '\t' || *c == ' ').collect();
                            let n: usize = ind.chars().map(|c| if c == '\t' { ts } else { 1 }).sum();
                            let lead = if tabs { format!("{}{}", "\t".repeat(n / ts), " ".repeat(n % ts)) } else { " ".repeat(n) };
                            format!("{}{}", lead, &l[ind.len()..])
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                        + if trailing { "\n" } else { "" };
                }
                self.s.translate_tabs = !tabs;
            }
            "zoom_in" => self.s.theme.editor_font_size = (self.s.theme.editor_font_size + 1.0).min(40.0),
            "zoom_out" => self.s.theme.editor_font_size = (self.s.theme.editor_font_size - 1.0).max(8.0),
            "about" => self.win_about = true,
            other => {
                if !self.run_extra(other) {
                    self.log(format!("Unknown command: {other}"));
                }
            }
        }
    }

    fn seed_find(&mut self) {
        if let Some(d) = self.docs.get(self.active) {
            if let Some((a, b)) = d.view.cursor {
                if a != b {
                    let (lo, hi) = (a.min(b), a.max(b));
                    let s: String = d.text.chars().skip(lo).take(hi - lo).collect();
                    if !s.contains('\n') {
                        self.find.query = s;
                    }
                }
            }
        }
    }

    fn open_palette(&mut self, m: PaletteMode) {
        self.palette = Some(m);
        self.pal_query.clear();
        self.pal_sel = 0;
        self.pal_focus = true;
        self.pal_items = self.palette_items(m);
        if m == PaletteMode::Files {
            if let Some(f) = self.folder.clone() {
                if self.file_index.as_ref().map(|(p, _)| p != &f).unwrap_or(true) {
                    let mut all = index_files(&f);
                    for x in self.extra_folders.clone() {
                        all.extend(index_files(&x));
                    }
                    self.file_index = Some((f.clone(), all));
                }
            }
        }
    }

    fn line_op(&mut self, f: impl FnOnce(&mut Vec<String>, usize, usize)) {
        let Some(d) = self.docs.get_mut(self.active) else { return };
        let (a, b) = d.view.cursor.unwrap_or((0, 0));
        let (lo, hi) = (a.min(b), a.max(b));
        let la = d.text.chars().take(lo).filter(|c| *c == '\n').count();
        let lb = d.text.chars().take(hi).filter(|c| *c == '\n').count();
        let trailing = d.text.ends_with('\n');
        let mut lines: Vec<String> = d.text.split('\n').map(String::from).collect();
        if trailing {
            lines.pop();
        }
        if lines.is_empty() {
            lines.push(String::new());
        }
        let lb = lb.min(lines.len().saturating_sub(1));
        let la = la.min(lb);
        f(&mut lines, la, lb);
        d.text = lines.join("\n") + if trailing { "\n" } else { "" };
    }

    fn sel_op(&mut self, f: impl Fn(&str) -> String) {
        let Some(d) = self.docs.get_mut(self.active) else { return };
        let Some((a, b)) = d.view.cursor else { return };
        let (lo, hi) = (a.min(b), a.max(b));
        if lo == hi {
            return;
        }
        let (bl, bh) = (char_to_byte(&d.text, lo), char_to_byte(&d.text, hi));
        let rep = f(&d.text[bl..bh]);
        d.text.replace_range(bl..bh, &rep);
        d.view.select = Some((lo, lo + rep.chars().count()));
    }

    fn toggle_comment(&mut self) {
        let ext = self
            .docs
            .get(self.active)
            .map(|d| if d.path.is_some() { d.ext() } else { d.title.rsplit('.').next().unwrap_or("").to_string() })
            .unwrap_or_default();
        let (open, close) = match ext.as_str() {
            "py" | "sh" | "bash" | "rb" | "yaml" | "yml" | "toml" | "pl" | "r" | "ps1" | "conf" | "ini" | "cmake" | "mk" => ("# ", ""),
            "lua" | "sql" | "hs" => ("-- ", ""),
            "html" | "htm" | "xml" | "md" | "markdown" | "svg" | "vue" => ("<!-- ", " -->"),
            "css" => ("/* ", " */"),
            "bat" | "cmd" => ("REM ", ""),
            _ => ("// ", ""),
        };
        let open = open.to_string();
        let close = close.to_string();
        self.line_op(move |lines, a, b| {
            let all = lines[a..=b].iter().filter(|l| !l.trim().is_empty()).all(|l| l.trim_start().starts_with(open.trim_end()));
            for l in &mut lines[a..=b] {
                if l.trim().is_empty() {
                    continue;
                }
                let ind = l.len() - l.trim_start().len();
                if all {
                    let rest = l[ind..].to_string();
                    let rest = rest.strip_prefix(open.as_str()).or_else(|| rest.strip_prefix(open.trim_end())).unwrap_or(&rest).to_string();
                    let rest = if close.is_empty() {
                        rest.clone()
                    } else {
                        rest.strip_suffix(close.as_str()).or_else(|| rest.strip_suffix(close.trim_start())).unwrap_or(&rest).to_string()
                    };
                    *l = format!("{}{}", &l[..ind], rest);
                } else {
                    *l = format!("{}{}{}{}", &l[..ind], open, &l[ind..], close);
                }
            }
        });
    }

    fn begin_color_edit(&mut self, doc_id: u64, m: ColorMatch) {
        let Some(d) = self.docs.iter().find(|d| d.id == doc_id) else { return };
        let Some(original) = d.text.get(m.start..m.end).map(String::from) else { return };
        self.color_edit = Some(ColorEdit { doc_id, start: m.start, end: m.end, original, color: m.color.to_color32(), m });
    }

    // -----------------------------------------------------------------------
    // markdown

    fn md_html(&self, d: &Doc, live: bool) -> String {
        markdown::to_html(&d.text, &d.title, self.pal.dark, &self.syntax_set, &self.scheme().theme, live)
    }

    fn open_browser_preview(&mut self) {
        let Some(d) = self.docs.get(self.active) else { return };
        if self.preview_server.is_none() {
            self.preview_server = PreviewServer::start();
        }
        let base = d.path.as_ref().and_then(|p| p.parent().map(Path::to_path_buf));
        let id = d.id;
        if let Some(srv) = &self.preview_server {
            srv.publish(self.md_html(d, true), base);
            markdown::open_url(&srv.url());
            self.live_browser = Some(id);
            self.flash("Opened live preview in your browser — it refreshes as you type".into());
        } else {
            let dir = std::env::temp_dir().join("bojan_text_preview");
            let _ = std::fs::create_dir_all(&dir);
            let f = dir.join("preview.html");
            if std::fs::write(&f, self.md_html(d, false)).is_ok() {
                markdown::open_url(&f.display().to_string());
            }
        }
    }

    fn export_html(&mut self) {
        let Some(d) = self.docs.get(self.active) else { return };
        let name = format!("{}.html", d.title.trim_end_matches(".md").trim_end_matches(".markdown"));
        if let Some(p) = rfd::FileDialog::new().set_file_name(&name).save_file() {
            let html = self.md_html(d, false);
            match std::fs::write(&p, html) {
                Ok(_) => self.flash(format!("Exported {}", p.display())),
                Err(e) => self.flash(format!("Export failed: {e}")),
            }
        }
    }

    // -----------------------------------------------------------------------
    // LSP

    fn spec_for(&self, ext: &str) -> Option<ServerSpec> {
        if ext.is_empty() {
            return None;
        }
        self.servers.iter().filter(|s| s.exts.iter().any(|e| e == ext)).find(|s| s.found.is_some()).cloned()
    }

    fn lsp_tick(&mut self) {
        if !self.s.lsp_enabled || self.shot.is_some() {
            return;
        }
        let wanted: Vec<(ServerSpec, PathBuf)> = self
            .docs
            .iter()
            .filter_map(|d| {
                let p = d.path.clone()?;
                Some((self.spec_for(&d.ext())?, p))
            })
            .collect();
        for (spec, p) in &wanted {
            if !self.clients.contains_key(&spec.name) && !self.lsp_errors.contains_key(&spec.name) {
                let root = self.folder.clone().filter(|f| p.starts_with(f)).or_else(|| p.parent().map(Path::to_path_buf));
                match LspClient::start(spec, root.as_deref()) {
                    Ok(c) => {
                        self.clients.insert(spec.name.clone(), c);
                    }
                    Err(e) => {
                        self.lsp_errors.insert(spec.name.clone(), e);
                    }
                }
            }
        }
        for d in &self.docs {
            let Some(p) = &d.path else { continue };
            if let Some(spec) = self.spec_for(&d.ext()) {
                if let Some(c) = self.clients.get_mut(&spec.name) {
                    c.sync(p, &d.text, hash_text(&d.text));
                }
            }
        }
        let mut events = Vec::new();
        for c in self.clients.values_mut() {
            events.extend(c.poll());
        }
        for e in events {
            match e {
                LspEvent::Diagnostics(k, v) => {
                    self.diags.insert(k, v);
                }
                LspEvent::Hover(t) => {
                    self.hover_popup = Some((t, egui::Pos2::ZERO));
                }
                LspEvent::Definition(p, line, ch) => {
                    self.open_path(&p);
                    if let Some(d) = self.docs.get_mut(self.active) {
                        let c = line_col_to_char(&d.text, line, ch);
                        d.view.select = Some((c, c));
                    }
                }
                LspEvent::References(list) => {
                    self.fif.hits = list
                        .into_iter()
                        .map(|(p, line, ch)| {
                            let text = std::fs::read_to_string(&p).ok().and_then(|t| t.lines().nth(line).map(String::from)).unwrap_or_default();
                            let col = lsp::char_col_from_utf16(&text, ch);
                            features::Hit { path: p, line, col, len: 0, text }
                        })
                        .collect();
                    self.fif.cur = None;
                    self.fif.title = format!("{} references (language server)", self.fif.hits.len());
                    self.panel = Some(BottomPanel::FindResults);
                }
                LspEvent::Status(s) => self.flash(s),
            }
        }
    }

    fn lsp_position(d: &Doc, ch: usize) -> (usize, usize) {
        let before: String = d.text.chars().take(ch).collect();
        let line = before.matches('\n').count();
        let line_start = before.rfind('\n').map(|i| i + 1).unwrap_or(0);
        let col16 = before[line_start..].encode_utf16().count();
        (line, col16)
    }

    fn lsp_definition(&mut self) {
        let Some(d) = self.docs.get(self.active) else { return };
        let Some(p) = d.path.clone() else { return };
        let ch = d.view.cursor.map(|c| c.0).unwrap_or(0);
        let (line, col) = Self::lsp_position(d, ch);
        if let Some(spec) = self.spec_for(&d.ext()) {
            if let Some(c) = self.clients.get_mut(&spec.name) {
                c.definition(&p, line, col);
                return;
            }
        }
        self.flash("No language server for this file — see Tools ▸ Language Servers".into());
    }

    fn handle_drops(&mut self, ctx: &egui::Context) {
        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).filter(|p| !p.as_os_str().is_empty()).collect());
        for p in dropped {
            if p.is_dir() {
                self.folder = Some(p);
                self.dir_cache.clear();
                self.s.show_sidebar = true;
            } else {
                self.open_path(&p);
            }
        }
    }

    fn session(&self) -> Session {
        Session {
            docs: self
                .docs
                .iter()
                .map(|d| SessionDoc {
                    path: d.path.as_ref().map(|p| p.display().to_string()),
                    title: d.title.clone(),
                    // hot exit (original default hot_exit = "always"): keep unsaved text
                    text: if d.path.is_none() || d.dirty() { Some(d.text.clone()) } else { None },
                    md_mode: Some(d.md_mode),
                })
                .collect(),
            active: self.active,
            folder: self.folder.as_ref().map(|p| p.display().to_string()),
            extra_folders: self.extra_folders.iter().map(|p| p.display().to_string()).collect(),
            project_file: self.project_file.as_ref().map(|p| p.display().to_string()),
        }
    }
}

fn line_col_to_char(text: &str, line: usize, col16: usize) -> usize {
    let ls: usize = text.split('\n').take(line).map(|l| l.chars().count() + 1).sum();
    let lt = text.split('\n').nth(line).unwrap_or("");
    ls + lsp::char_col_from_utf16(lt, col16)
}

fn logo(p: &egui::Painter, r: egui::Rect, accent: Color32, bg: Color32) {
    p.rect_filled(r, 14.0, accent);
    let c = r.center();
    let s = r.width() * 0.22;
    p.rect_filled(egui::Rect::from_min_size(egui::pos2(c.x - s, c.y - s * 1.3), egui::vec2(s * 1.6, s * 1.2)), 6.0, bg);
    p.rect_filled(egui::Rect::from_min_size(egui::pos2(c.x - s, c.y + s * 0.05), egui::vec2(s * 1.9, s * 1.25)), 6.0, bg);
    p.rect_filled(egui::Rect::from_min_size(egui::pos2(c.x - s * 1.45, c.y - s * 1.5), egui::vec2(s * 0.3, s * 3.0)), 2.0, bg);
}

fn fuzzy(hay: &str, q: &str) -> Option<i32> {
    if q.is_empty() {
        return Some(0);
    }
    if let Some(p) = hay.find(q) {
        return Some(1000 - p as i32 - hay.len() as i32 / 4);
    }
    let mut score = 0;
    let mut it = hay.chars().enumerate();
    let mut last = 0usize;
    for qc in q.chars() {
        if qc == ' ' {
            continue;
        }
        loop {
            match it.next() {
                Some((i, hc)) if hc == qc => {
                    score += if i == last + 1 { 8 } else { 2 };
                    last = i;
                    break;
                }
                Some(_) => {}
                None => return None,
            }
        }
    }
    Some(score - hay.len() as i32 / 8)
}

fn count_matches(text: &str, q: &str, case: bool) -> usize {
    if q.is_empty() {
        return 0;
    }
    if case {
        text.matches(q).count()
    } else {
        text.to_ascii_lowercase().matches(&q.to_ascii_lowercase()).count()
    }
}

fn replace_ci(text: &str, q: &str, rep: &str) -> String {
    let lower = text.to_ascii_lowercase();
    let ql = q.to_ascii_lowercase();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while let Some(p) = lower[i..].find(&ql) {
        out.push_str(&text[i..i + p]);
        out.push_str(rep);
        i += p + ql.len();
    }
    out.push_str(&text[i..]);
    out
}

fn read_dir_sorted(dir: &Path) -> Vec<(PathBuf, bool)> {
    let mut v: Vec<(PathBuf, bool)> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| {
                    let p = e.path();
                    let d = p.is_dir();
                    (p, d)
                })
                .collect()
        })
        .unwrap_or_default();
    v.retain(|(p, _)| {
        let n = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        !(n == ".git" || n == "node_modules" || n == "target" || n == "__pycache__")
    });
    v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.file_name().map(|n| n.to_ascii_lowercase()).cmp(&b.0.file_name().map(|n| n.to_ascii_lowercase()))));
    v
}

fn index_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        for (p, is_dir) in read_dir_sorted(&d) {
            if is_dir {
                stack.push(p);
            } else {
                out.push(p);
                if out.len() > 20000 {
                    return out;
                }
            }
        }
    }
    out
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.frame += 1;

        // background package scan finished?
        if let Some(rx) = &self.scan_rx {
            if let Ok(res) = rx.try_recv() {
                self.scan_rx = None;
                if self.s.use_sublime_packages {
                    self.syntax_set = Arc::new(res.set);
                    self.set_gen += 1;
                    let current = self.scheme().name.clone();
                    self.schemes.retain(|s| !s.source.starts_with("Sublime:"));
                    let mut seen: HashSet<String> = self.schemes.iter().map(|s| s.name.clone()).collect();
                    for s in res.schemes {
                        if seen.insert(s.name.clone()) {
                            self.schemes.push(s);
                        }
                    }
                    self.scheme_idx = self.schemes.iter().position(|s| s.name == current).unwrap_or(0);
                    let total = res.report.total_loaded();
                    self.flash(format!("Loaded {total} languages from Sublime Text packages"));
                }
                self.pkg_report = Some(res.report);
                self.extras = res.extras;
                self.code_cache.clear();
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(200));
            }
        }

        // theme
        let key = (self.scheme().name.clone(), self.s.theme.clone());
        if self.applied.as_ref() != Some(&key) {
            self.pal = theme::palette_from(&self.scheme().theme, &self.s.theme);
            theme::apply(&ctx, &self.pal, &self.s.theme);
            self.applied = Some(key);
            self.code_cache.clear();
        }

        if self.fonts_dirty {
            self.fonts_dirty = false;
            theme::install_fonts_with(&ctx, self.s.font_file.as_deref());
        }
        for cmd in std::mem::take(&mut self.viewport_cmds) {
            ctx.send_viewport_cmd(cmd);
        }
        self.lsp_tick();
        if !self.clients.is_empty() || self.build.running.load(std::sync::atomic::Ordering::Relaxed) || self.fif.rx.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(150));
        }
        self.poll_find_in_files();
        // most-recently-used order (Ctrl+Tab)
        if let Some(d) = self.docs.get(self.active) {
            let id = d.id;
            if self.mru.first() != Some(&id) {
                self.mru.retain(|x| *x != id);
                self.mru.insert(0, id);
            }
        }
        let live: HashSet<u64> = self.docs.iter().map(|d| d.id).collect();
        self.mru.retain(|x| live.contains(x));
        // macros: capture typing while recording; replay a recorded macro
        self.macro_capture(&ctx);
        let mut pending = std::mem::take(&mut self.pending_events);
        if let Some(m) = self.play_macro.take() {
            pending.extend(m);
        }
        if !pending.is_empty() {
            self.macro_playback(&ctx, pending);
        }
        // test switches for --screenshot runs: --layout G:C, --split-view, --syntax-popup
        if self.shot.is_some() && self.frame == 8 {
            let args: Vec<String> = std::env::args().collect();
            if let Some(i) = args.iter().position(|a| a == "--layout") {
                let mut it = args.get(i + 1).map(|v| v.split(':').map(|x| x.parse::<usize>().unwrap_or(1)).collect::<Vec<_>>()).unwrap_or_default().into_iter();
                let (g, c) = (it.next().unwrap_or(1), it.next().unwrap_or(1));
                self.set_layout(g, c);
                if self.groups > 1 && self.docs.len() > 1 {
                    self.active = 1;
                    self.move_file_to_group(1);
                    self.active = 0;
                }
            }
            if args.iter().any(|a| a == "--split-view") {
                self.split_view();
            }
            if args.iter().any(|a| a == "--syntax-popup") {
                self.syntax_popup = true;
            }
        }
        // --demo-typing (screenshot runs only): type into a new file to test Enter / Tab
        if self.shot.is_some() && std::env::args().any(|a| a == "--demo-typing") {
            let key = |k: Key| egui::Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE };
            match self.frame {
                10 => {
                    self.run("new");
                    if let Some(d) = self.docs.get_mut(self.active) {
                        d.syntax_override = Some("Rust".into());
                    }
                }
                14 => self.pending_events.push(egui::Event::Text("fn main() {}".into())),
                18 => self.pending_events.push(key(Key::ArrowLeft)),
                22 => self.pending_events.push(key(Key::Enter)),
                26 => self.pending_events.push(egui::Event::Text("let x = 1;".into())),
                30 => self.pending_events.push(key(Key::Enter)),
                34 => self.pending_events.push(egui::Event::Text("let ab".into())),
                38 => self.pending_events.push(key(Key::Enter)),
                42 => self.pending_events.push(egui::Event::Text("ab".into())),
                46 => self.pending_events.push(key(Key::Tab)),
                _ => {}
            }
        }
        if self.shot.is_some() && self.frame == 12 && std::env::args().any(|a| a == "--demo-find") {
            self.folder = std::env::current_dir().ok().map(|d| d.join("src"));
            self.find.query = "fn newline".into();
            self.find.open = true;
            self.fif.open = true;
            self.fif.where_ = "<open folders>".into();
            self.start_find_in_files();
        }
        self.sync_clones();
        self.sync_groups();
        self.shortcuts(&ctx);
        self.handle_drops(&ctx);

        // live browser preview
        if let Some(id) = self.live_browser {
            if let Some(d) = self.docs.iter().find(|d| d.id == id) {
                if let Some(srv) = &self.preview_server {
                    let base = d.path.as_ref().and_then(|p| p.parent().map(Path::to_path_buf));
                    srv.publish(self.md_html(d, true), base);
                }
            }
        }

        // window title
        if self.frame % 15 == 1 {
            let t = self
                .docs
                .get(self.active)
                .map(|d| {
                    let path = d.path.as_ref().map(|p| p.display().to_string()).unwrap_or(d.title.clone());
                    format!("{}{} - {}", path, if d.dirty() { " •" } else { "" }, APP_NAME)
                })
                .unwrap_or(APP_NAME.to_string());
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(t));
        }

        let p = self.pal;
        if self.s.show_menu {
            egui::Panel::top("menubar").frame(egui::Frame::new().fill(p.status_bg).inner_margin(egui::Margin::symmetric(6, 3))).show(ui, |ui| {
                self.menu_bar(ui);
            });
        }
        if self.s.show_status_bar {
            egui::Panel::bottom("status").exact_size(26.0).frame(egui::Frame::new().fill(p.status_bg).inner_margin(egui::Margin::symmetric(12, 0))).show(ui, |ui| {
                self.status_bar(ui);
            });
        }
        if self.panel.is_some() {
            egui::Panel::bottom("bottom_panel").resizable(true).default_size(180.0).frame(egui::Frame::new().fill(p.sidebar_bg).inner_margin(egui::Margin::same(8))).show(ui, |ui| {
                self.bottom_panel(ui);
            });
        }
        if self.s.show_sidebar {
            egui::Panel::left("sidebar")
                .resizable(true)
                .default_size(220.0)
                .frame(egui::Frame::new().fill(p.sidebar_bg).inner_margin(egui::Margin { left: 10, right: 6, top: 0, bottom: 0 }))
                .show(ui, |ui| {
                    self.sidebar(ui);
                });
        }
        egui::CentralPanel::no_frame().show(ui, |ui| {
            if self.find.open {
                egui::Panel::top("findbar").frame(egui::Frame::new().fill(mix(p.sidebar_bg, p.bg, 0.5)).inner_margin(egui::Margin::symmetric(4, 6))).show(ui, |ui| {
                    self.find_bar(ui);
                });
            }
            if !self.s.show_tabs && !self.s.show_menu && self.distraction.is_none() {
                egui::Panel::top("menu_toggle").frame(egui::Frame::new().fill(p.bg)).show(ui, |ui| {
                    if ui.add(egui::Button::new(RichText::new("☰").size(16.0).color(p.muted)).frame(false)).on_hover_text("Show the menu again").clicked() {
                        self.s.show_menu = true;
                    }
                });
            }
            egui::CentralPanel::no_frame().show(ui, |ui| {
                self.panes(ui);
            });
        });

        self.command_palette(&ctx);
        self.syntax_chooser(&ctx);
        self.windows(&ctx);

        // Show Scope Name popup
        if let Some(sc) = self.scope_popup.clone() {
            let mut open = true;
            egui::Window::new("Scope Name").open(&mut open).collapsible(false).resizable(false).show(&ctx, |ui| {
                ui.label(RichText::new(sc.replace(' ', "\n")).monospace());
                if ui.button("Copy").clicked() {
                    ui.ctx().copy_text(sc.clone());
                }
            });
            if !open {
                self.scope_popup = None;
            }
        }
        if self.recording.is_some() {
            egui::Area::new(egui::Id::new("rec_badge")).anchor(egui::Align2::RIGHT_TOP, egui::vec2(-16.0, 40.0)).show(&ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.label(RichText::new("● Recording macro (Ctrl+Q to stop)").color(Color32::from_rgb(0xf0, 0x5d, 0x5d)));
                });
            });
        }

        // --screenshot: draw ~120 frames, capture, save, exit
        if let Some(shot) = self.shot.as_mut() {
            shot.frame += 1;
            ctx.request_repaint();
            if shot.frame >= 120 && !shot.requested {
                shot.requested = true;
                let slot = shot.image.clone();
                ctx.request_screenshot(move |img| {
                    *slot.lock().unwrap() = Some(img);
                });
            }
            let img = shot.image.lock().unwrap().take();
            if let Some(img) = img {
                let [w, h] = img.size;
                let mut buf = Vec::with_capacity(w * h * 4);
                for px in &img.pixels {
                    let [r, g, b, a] = px.to_srgba_unmultiplied();
                    buf.extend_from_slice(&[r, g, b, a]);
                }
                if let Some(parent) = shot.path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if let Some(im) = image::RgbaImage::from_raw(w as u32, h as u32, buf) {
                    if let Err(e) = im.save(&shot.path) {
                        eprintln!("screenshot save failed: {e}");
                    }
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            if shot.frame > 900 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        if self.shot.is_some() {
            return;
        }
        eframe::set_value(storage, "settings", &self.s);
        eframe::set_value(storage, "session", &self.session());
    }
}
