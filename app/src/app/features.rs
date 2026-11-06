//! Behaviour behind the full menu set: editing commands, marks, bookmarks,
//! jump history, find in files, build systems, macros, projects, settings
//! file, encodings and line endings.

use super::*;
use crate::textops::{self, Out};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineEnding {
    Windows,
    Unix,
    Mac,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Encoding {
    Utf8,
    Utf8Bom,
    Utf16Le,
    Utf16Be,
    Latin1,
}

impl Encoding {
    pub fn label(self) -> &'static str {
        match self {
            Encoding::Utf8 => "UTF-8",
            Encoding::Utf8Bom => "UTF-8 with BOM",
            Encoding::Utf16Le => "UTF-16 LE",
            Encoding::Utf16Be => "UTF-16 BE",
            Encoding::Latin1 => "Western (ISO 8859-1)",
        }
    }
    pub const ALL: [Encoding; 5] = [Encoding::Utf8, Encoding::Utf8Bom, Encoding::Utf16Le, Encoding::Utf16Be, Encoding::Latin1];
}

/// Decode file bytes; `force` picks an encoding instead of detecting one.
pub fn decode(bytes: &[u8], force: Option<Encoding>) -> (String, Encoding) {
    let enc = force.unwrap_or_else(|| {
        if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
            Encoding::Utf8Bom
        } else if bytes.starts_with(&[0xFF, 0xFE]) {
            Encoding::Utf16Le
        } else if bytes.starts_with(&[0xFE, 0xFF]) {
            Encoding::Utf16Be
        } else if std::str::from_utf8(bytes).is_ok() {
            Encoding::Utf8
        } else {
            Encoding::Latin1
        }
    });
    let text = match enc {
        Encoding::Utf8 => String::from_utf8_lossy(bytes).to_string(),
        Encoding::Utf8Bom => String::from_utf8_lossy(bytes.strip_prefix(&[0xEF, 0xBB, 0xBF][..]).unwrap_or(bytes)).to_string(),
        Encoding::Utf16Le | Encoding::Utf16Be => {
            let b = bytes.strip_prefix(if enc == Encoding::Utf16Le { &[0xFF, 0xFE][..] } else { &[0xFE, 0xFF][..] }).unwrap_or(bytes);
            let units: Vec<u16> = b.chunks_exact(2).map(|c| if enc == Encoding::Utf16Le { u16::from_le_bytes([c[0], c[1]]) } else { u16::from_be_bytes([c[0], c[1]]) }).collect();
            String::from_utf16_lossy(&units)
        }
        Encoding::Latin1 => bytes.iter().map(|&b| b as char).collect(),
    };
    (text, enc)
}

pub fn encode(text: &str, enc: Encoding) -> Vec<u8> {
    match enc {
        Encoding::Utf8 => text.as_bytes().to_vec(),
        Encoding::Utf8Bom => [&[0xEF, 0xBB, 0xBF][..], text.as_bytes()].concat(),
        Encoding::Utf16Le => [0xFF, 0xFE].into_iter().chain(text.encode_utf16().flat_map(|u| u.to_le_bytes())).collect(),
        Encoding::Utf16Be => [0xFE, 0xFF].into_iter().chain(text.encode_utf16().flat_map(|u| u.to_be_bytes())).collect(),
        Encoding::Latin1 => text.chars().map(|c| if (c as u32) < 256 { c as u8 } else { b'?' }).collect(),
    }
}

pub fn detect_line_ending(raw: &str) -> LineEnding {
    if raw.contains("\r\n") {
        LineEnding::Windows
    } else if raw.contains('\n') {
        LineEnding::Unix
    } else if raw.contains('\r') {
        LineEnding::Mac
    } else if cfg!(windows) {
        LineEnding::Windows
    } else {
        LineEnding::Unix
    }
}

pub fn config_dir() -> PathBuf {
    let base = std::env::var("APPDATA").map(PathBuf::from).unwrap_or_else(|_| std::env::temp_dir());
    base.join("BojanText")
}

pub fn settings_path() -> PathBuf {
    config_dir().join("Bojan Text.sublime-settings")
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BottomPanel {
    Problems,
    FindResults,
    Build,
    Console,
}

#[derive(Clone, Debug)]
pub struct Hit {
    pub path: PathBuf,
    pub line: usize,
    pub col: usize, // chars
    pub len: usize, // chars
    pub text: String,
}

#[derive(Default)]
pub struct FindInFiles {
    pub open: bool,
    pub where_: String,
    pub query: String,
    pub hits: Vec<Hit>,
    pub cur: Option<usize>,
    pub files: usize,
    pub cancel: Option<Arc<AtomicBool>>,
    pub rx: Option<Receiver<FifMsg>>,
    pub title: String,
    /// `Searching N files for "q" (regex, case sensitive, whole word)` (sub_140068cc4)
    pub header: String,
    pub flags: String,
    /// `N matches across M files` (sub_140069544), shown when the search is done
    pub footer: String,
    pub cancelled: bool,
}

pub enum FifMsg {
    Total(usize),
    Hit(Hit),
    Files(usize),
    Done,
}

#[derive(Default)]
pub struct BuildState {
    pub choice: Option<(String, Option<String>)>, // (system, variant)
    pub output: Arc<Mutex<Vec<String>>>,
    pub child: Option<Arc<Mutex<Option<std::process::Child>>>>,
    pub running: Arc<AtomicBool>,
    pub regex: String,
    pub dir: PathBuf,
    pub cur: Option<usize>,
}

impl App {
    // ------------------------------------------------------------------
    // helpers

    pub(super) fn all_folders(&self) -> Vec<PathBuf> {
        self.folder.iter().cloned().chain(self.extra_folders.iter().cloned()).collect()
    }

    pub(super) fn sel(&self) -> (usize, usize) {
        self.docs.get(self.active).and_then(|d| d.view.cursor).map(|(a, b)| (a.min(b), a.max(b))).unwrap_or((0, 0))
    }

    pub(super) fn log(&mut self, msg: impl Into<String>) {
        let m = msg.into();
        self.console.push(m.clone());
        if self.console.len() > 500 {
            self.console.remove(0);
        }
        self.status = Some((m, Instant::now()));
    }

    /// Run a text transform on the active document.
    pub(super) fn edit_active(&mut self, f: impl FnOnce(&str, usize, usize) -> Option<Out>) {
        let (lo, hi) = self.sel();
        let Some(d) = self.docs.get_mut(self.active) else { return };
        if let Some(o) = f(&d.text, lo, hi) {
            d.text = o.text;
            d.view.select = Some(o.sel);
        }
    }

    pub(super) fn select(&mut self, a: usize, b: usize) {
        if let Some(d) = self.docs.get_mut(self.active) {
            d.view.select = Some((a, b));
        }
    }

    pub(super) fn selected_text(&self) -> String {
        let (lo, hi) = self.sel();
        self.docs.get(self.active).map(|d| d.text.chars().skip(lo).take(hi - lo).collect()).unwrap_or_default()
    }

    fn word_or_selection(&self) -> String {
        let (lo, hi) = self.sel();
        if lo != hi {
            return self.selected_text();
        }
        let Some(d) = self.docs.get(self.active) else { return String::new() };
        let c: Vec<char> = d.text.chars().collect();
        let (a, b) = textops::word_at(&c, lo);
        c[a..b].iter().collect()
    }

    pub(super) fn push_jump(&mut self) {
        if let Some(d) = self.docs.get(self.active) {
            let here = (d.id, self.sel().0);
            self.jumps.truncate(self.jump_idx);
            if self.jumps.last() != Some(&here) {
                self.jumps.push(here);
            }
            self.jump_idx = self.jumps.len();
        }
    }

    fn go_jump(&mut self, back: bool) {
        if back {
            if self.jump_idx == self.jumps.len() {
                self.push_jump();
                self.jump_idx = self.jumps.len().saturating_sub(1);
            }
            if self.jump_idx == 0 {
                return;
            }
            self.jump_idx -= 1;
        } else {
            if self.jump_idx + 1 >= self.jumps.len() {
                return;
            }
            self.jump_idx += 1;
        }
        let (doc, ch) = self.jumps[self.jump_idx];
        if let Some(i) = self.docs.iter().position(|d| d.id == doc) {
            self.active = i;
            self.select(ch, ch);
        }
    }

    fn clipboard_set(&mut self, t: String) {
        if t.is_empty() {
            return;
        }
        if let Ok(mut cb) = arboard::Clipboard::new() {
            let _ = cb.set_text(t.clone());
        }
        self.paste_history.retain(|x| x != &t);
        self.paste_history.insert(0, t);
        self.paste_history.truncate(15);
    }

    fn clipboard_get(&self) -> String {
        arboard::Clipboard::new().ok().and_then(|mut c| c.get_text().ok()).unwrap_or_default().replace("\r\n", "\n")
    }

    fn insert_text(&mut self, t: &str) {
        let t = t.to_string();
        self.edit_active(|text, lo, hi| {
            let c: Vec<char> = text.chars().collect();
            let mut out: String = c[..lo].iter().collect();
            out.push_str(&t);
            out.extend(c[hi..].iter());
            let p = lo + t.chars().count();
            Some(Out { text: out, sel: (p, p) })
        });
    }

    pub(super) fn copy_or_cut(&mut self, cut: bool) {
        let (lo, hi) = self.sel();
        let Some(d) = self.docs.get(self.active) else { return };
        let c: Vec<char> = d.text.chars().collect();
        // like the original: with nothing selected, the whole line is copied
        let (a, b) = if lo == hi {
            let (a, b) = textops::line_span(&c, lo, hi);
            (a, if b < c.len() { b + 1 } else { b })
        } else {
            (lo, hi)
        };
        let mut t: String = c[a..b].iter().collect();
        if lo == hi && !t.ends_with('\n') {
            t.push('\n');
        }
        self.clipboard_set(t);
        if cut {
            let out: String = c[..a].iter().chain(c[b..].iter()).collect();
            if let Some(d) = self.docs.get_mut(self.active) {
                d.text = out;
                d.view.select = Some((a, a));
            }
        }
    }

    fn paste(&mut self, indent: bool) {
        let t = self.clipboard_get();
        if t.is_empty() {
            return;
        }
        let (lo, hi) = self.sel();
        // a whole copied line pastes above the current line (like the original)
        if lo == hi && t.ends_with('\n') && t.matches('\n').count() == 1 && !indent {
            let Some(d) = self.docs.get(self.active) else { return };
            let c: Vec<char> = d.text.chars().collect();
            let a = textops::line_start(&c, lo);
            let mut out: String = c[..a].iter().collect();
            out.push_str(&t);
            out.extend(c[a..].iter());
            let p = lo + t.chars().count();
            if let Some(d) = self.docs.get_mut(self.active) {
                d.text = out;
                d.view.select = Some((p, p));
            }
            return;
        }
        if indent {
            let Some(d) = self.docs.get(self.active) else { return };
            let c: Vec<char> = d.text.chars().collect();
            let ls = textops::line_start(&c, lo);
            let ind: String = c[ls..lo].iter().take_while(|ch| **ch == ' ' || **ch == '\t').collect();
            let lines: Vec<&str> = t.split('\n').collect();
            let common = lines.iter().skip(1).filter(|l| !l.trim().is_empty()).map(|l| l.len() - l.trim_start().len()).min().unwrap_or(0);
            let fixed: Vec<String> = lines
                .iter()
                .enumerate()
                .map(|(i, l)| if i == 0 { l.trim_start().to_string() } else if l.trim().is_empty() { String::new() } else { format!("{ind}{}", &l[common.min(l.len() - l.trim_start().len())..]) })
                .collect();
            self.insert_text(&fixed.join("\n"));
        } else {
            self.insert_text(&t);
        }
    }

    pub(super) fn editor_focus_id(&self) -> Option<egui::Id> {
        self.docs.get(self.active).map(|d| egui::Id::new(("doc", d.id)).with("te"))
    }

    /// Lines changed since the last save, as a line range of the current text.
    pub(super) fn modified_range(&mut self, idx: usize) -> Option<(usize, usize)> {
        let d = self.docs.get_mut(idx)?;
        let h = hash_text(&d.text);
        if d.mod_cache.0 == h && h != 0 {
            return d.mod_cache.1;
        }
        let old: Vec<&str> = d.saved_text.split('\n').collect();
        let new: Vec<&str> = d.text.split('\n').collect();
        let r = if d.path.is_none() && d.saved_text.is_empty() {
            None
        } else if old == new {
            None
        } else {
            let mut p = 0;
            while p < old.len() && p < new.len() && old[p] == new[p] {
                p += 1;
            }
            let mut s = 0;
            while s < old.len() - p && s < new.len() - p && old[old.len() - 1 - s] == new[new.len() - 1 - s] {
                s += 1;
            }
            Some((p, new.len() - s))
        };
        d.mod_cache = (h, r);
        r
    }

    fn revert_modification(&mut self) {
        let Some(d) = self.docs.get(self.active) else { return };
        let old: Vec<&str> = d.saved_text.split('\n').collect();
        let new: Vec<&str> = d.text.split('\n').collect();
        let mut p = 0;
        while p < old.len() && p < new.len() && old[p] == new[p] {
            p += 1;
        }
        let mut s = 0;
        while s < old.len() - p && s < new.len() - p && old[old.len() - 1 - s] == new[new.len() - 1 - s] {
            s += 1;
        }
        if p == new.len() && p == old.len() {
            return;
        }
        let mut out: Vec<&str> = new[..p].to_vec();
        out.extend_from_slice(&old[p..old.len() - s]);
        out.extend_from_slice(&new[new.len() - s..]);
        let text = out.join("\n");
        let ch: usize = out[..p].iter().map(|l| l.chars().count() + 1).sum();
        if let Some(d) = self.docs.get_mut(self.active) {
            d.text = text;
            d.view.select = Some((ch, ch));
        }
    }

    fn current_line(&self) -> usize {
        let (lo, _) = self.sel();
        self.docs.get(self.active).map(|d| d.text.chars().take(lo).filter(|c| *c == '\n').count()).unwrap_or(0)
    }

    fn goto_line0(&mut self, line: usize) {
        if let Some(d) = self.docs.get_mut(self.active) {
            let ch: usize = d.text.split('\n').take(line).map(|l| l.chars().count() + 1).sum();
            d.view.select = Some((ch, ch));
        }
    }

    fn detect_indentation(&mut self) {
        let Some(d) = self.docs.get(self.active) else { return };
        let mut tabs = 0;
        let mut spaces = std::collections::HashMap::new();
        let mut prev = 0usize;
        for l in d.text.lines() {
            if l.trim().is_empty() {
                continue;
            }
            if l.starts_with('\t') {
                tabs += 1;
            } else {
                let n = l.len() - l.trim_start_matches(' ').len();
                let delta = n.abs_diff(prev);
                if delta == 2 || delta == 4 || delta == 8 || delta == 3 {
                    *spaces.entry(delta).or_insert(0) += 1;
                }
                prev = n;
            }
        }
        let sp_total: usize = spaces.values().sum();
        if sp_total > tabs {
            let width = spaces.into_iter().max_by_key(|(_, c)| *c).map(|(w, _)| w).unwrap_or(4);
            self.s.translate_tabs = true;
            self.s.tab_size = width;
            self.log(format!("Indentation: {width} spaces"));
        } else if tabs > 0 {
            self.s.translate_tabs = false;
            self.log("Indentation: tabs");
        }
    }

    fn switch_header(&mut self) {
        let Some(p) = self.docs.get(self.active).and_then(|d| d.path.clone()) else { return };
        let ext = p.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
        let others: &[&str] = match ext.as_str() {
            "h" | "hh" | "hpp" | "hxx" => &["c", "cc", "cpp", "cxx", "m", "mm"],
            "c" | "cc" | "cpp" | "cxx" | "m" | "mm" => &["h", "hh", "hpp", "hxx"],
            _ => &[],
        };
        for o in others {
            let cand = p.with_extension(o);
            if cand.exists() {
                self.open_path(&cand);
                return;
            }
        }
        self.log("No matching header/implementation file");
    }

    // ------------------------------------------------------------------
    // find

    /// The query as (pattern, match case, regex) with `whole_word` applied.
    pub(super) fn find_spec(&self) -> (String, bool, bool) {
        let q = self.find.query.clone();
        if self.find.whole && !q.is_empty() {
            let inner = if self.find.regex { format!("(?:{q})") } else { fancy_regex::escape(&q).to_string() };
            (format!(r"\b{inner}\b"), self.find.case, true)
        } else {
            (q, self.find.case, self.find.regex)
        }
    }

    fn find_spec_for(&self, q: &str) -> (String, bool, bool) {
        let mut me = self.find_spec();
        if !self.find.whole {
            me.0 = q.to_string();
        }
        me
    }

    /// All matches in `text` (byte ranges), limited to the "in selection" range when on.
    pub(super) fn find_all(&self, text: &str) -> Vec<(usize, usize)> {
        let (q, case, regex) = self.find_spec();
        let mut m = editor::find_matches(text, &q, case, regex);
        if let Some((a, b)) = self.find.in_sel {
            let (ba, bb) = (char_to_byte(text, a), char_to_byte(text, b));
            m.retain(|x| x.0 >= ba && x.1 <= bb);
        }
        m
    }

    pub(super) fn find_step(&mut self, forward: bool) {
        let q = self.find.query.clone();
        if q.is_empty() {
            return;
        }
        let (case, regex) = (self.find.case, self.find.regex);
        let (lo, hi) = self.sel();
        let Some(d) = self.docs.get(self.active) else { return };
        let matches = self.find_all(&d.text);
        let _ = (case, regex);
        if matches.is_empty() {
            self.log(format!("Unable to find {q}"));
            return;
        }
        let cur = char_to_byte(&d.text, if forward { hi } else { lo });
        let m = if forward {
            matches.iter().find(|m| m.0 >= cur && !(m.0 == cur && m.1 == cur)).or(if self.find.wrap { matches.first() } else { None })
        } else {
            matches.iter().rev().find(|m| m.1 <= cur && m.0 < cur).or(if self.find.wrap { matches.last() } else { None })
        };
        if let Some(&(a, b)) = m {
            let (ca, cb) = (byte_to_char(&d.text, a), byte_to_char(&d.text, b));
            self.select(ca, cb);
        }
    }

    pub(super) fn replace_step(&mut self) {
        let q = self.find.query.clone();
        if q.is_empty() {
            return;
        }
        let (lo, hi) = self.sel();
        let rep = self.find.replace.clone();
        let (case, regex) = (self.find.case, self.find.regex);
        let (sq, sc, sr) = self.find_spec_for(&q);
        if let Some(d) = self.docs.get_mut(self.active) {
            let (bl, bh) = (char_to_byte(&d.text, lo), char_to_byte(&d.text, hi));
            let sel = d.text[bl..bh].to_string();
            let hit = editor::find_matches(&sel, &sq, sc, sr).first().map(|m| *m == (0, sel.len())).unwrap_or(false);
            if lo != hi && hit {
                let new = if regex {
                    let pat = if case { sq.clone() } else { format!("(?i){sq}") };
                    fancy_regex::Regex::new(&pat).map(|re| re.replace(&sel, rep.as_str()).to_string()).unwrap_or(rep.clone())
                } else {
                    rep.clone()
                };
                d.text.replace_range(bl..bh, &new);
                let c = lo + new.chars().count();
                d.view.cursor = Some((c, c));
            }
        }
        self.find_step(true);
    }

    pub(super) fn replace_all(&mut self) {
        let q = self.find.query.clone();
        if q.is_empty() {
            return;
        }
        let rep = self.find.replace.clone();
        let (case, regex) = (self.find.case, self.find.regex);
        let (spec_q, spec_c, spec_r) = self.find_spec();
        let in_sel = self.find.in_sel;
        let Some(d) = self.docs.get_mut(self.active) else { return };
        let n;
        if regex {
            let pat = if case { spec_q.clone() } else { format!("(?i){spec_q}") };
            let Ok(re) = fancy_regex::Regex::new(&pat) else { return };
            n = re.find_iter(&d.text).count();
            d.text = re.replace_all(&d.text, rep.as_str()).to_string();
        } else {
            let m = { let (sq, sc, sr) = (spec_q.clone(), spec_c, spec_r); let mut v = editor::find_matches(&d.text, &sq, sc, sr); if let Some((a, b)) = in_sel { let (ba, bb) = (char_to_byte(&d.text, a), char_to_byte(&d.text, b)); v.retain(|x| x.0 >= ba && x.1 <= bb); } v };
            n = m.len();
            for (a, b) in m.into_iter().rev() {
                d.text.replace_range(a..b, &rep);
            }
        }
        self.log(format!("Replaced {n} occurrences"));
    }

    fn quick_find(&mut self, add_next: bool) {
        let (lo, hi) = self.sel();
        let Some(d) = self.docs.get(self.active) else { return };
        let c: Vec<char> = d.text.chars().collect();
        if lo == hi {
            let (a, b) = textops::word_at(&c, lo);
            if a != b {
                if add_next {
                    self.select(a, b);
                    return;
                }
                self.find.query = c[a..b].iter().collect();
            } else {
                return;
            }
        } else {
            self.find.query = c[lo..hi].iter().collect();
        }
        let (case, regex) = (self.find.case, self.find.regex);
        self.find.case = true;
        self.find.regex = false;
        self.find_step(true);
        self.find.case = case;
        self.find.regex = regex;
    }

    // ------------------------------------------------------------------
    // find in files

    pub(super) fn start_find_in_files(&mut self) {
        if let Some(c) = &self.fif.cancel {
            c.store(true, Ordering::Relaxed);
        }
        let q = self.find.query.clone();
        if q.is_empty() {
            return;
        }
        let roots: Vec<PathBuf> = if self.fif.where_.trim().is_empty() || self.fif.where_.trim() == "<open folders>" {
            let mut r = self.all_folders();
            if r.is_empty() {
                r = self.docs.iter().filter_map(|d| d.path.clone()).collect();
            }
            r
        } else {
            self.fif.where_.split(',').map(|s| PathBuf::from(s.trim())).collect()
        };
        let open_files: Vec<(PathBuf, String)> = self.docs.iter().filter_map(|d| d.path.clone().map(|p| (p, d.text.clone()))).collect();
        let (qspec, case, regex) = self.find_spec();
        let cancel = Arc::new(AtomicBool::new(false));
        let (tx, rx) = std::sync::mpsc::channel();
        let cc = cancel.clone();
        let qq = qspec;
        std::thread::spawn(move || {
            let mut files = Vec::new();
            for r in &roots {
                if r.is_file() {
                    files.push(r.clone());
                } else {
                    files.extend(index_files(r));
                }
            }
            let _ = tx.send(FifMsg::Total(files.len()));
            let mut n = 0;
            for f in files {
                if cc.load(Ordering::Relaxed) {
                    break;
                }
                let text = match open_files.iter().find(|(p, _)| *p == f) {
                    Some((_, t)) => t.clone(),
                    None => {
                        let Ok(meta) = std::fs::metadata(&f) else { continue };
                        if meta.len() > 4_000_000 {
                            continue;
                        }
                        let Ok(bytes) = std::fs::read(&f) else { continue };
                        if bytes.iter().take(4096).any(|b| *b == 0) {
                            continue; // binary
                        }
                        decode(&bytes, None).0.replace("\r\n", "\n")
                    }
                };
                n += 1;
                if n % 50 == 0 {
                    let _ = tx.send(FifMsg::Files(n));
                }
                for (ln, line) in text.split('\n').enumerate() {
                    for (a, b) in editor::find_matches(line, &qq, case, regex).into_iter().take(20) {
                        let col = line[..a].chars().count();
                        let len = line[a..b].chars().count();
                        let shown: String = line.chars().take(300).collect();
                        let _ = tx.send(FifMsg::Hit(Hit { path: f.clone(), line: ln, col, len, text: shown }));
                    }
                }
            }
            let _ = tx.send(FifMsg::Files(n));
            let _ = tx.send(FifMsg::Done);
        });
        self.fif.hits.clear();
        self.fif.cur = None;
        self.fif.files = 0;
        self.fif.cancelled = false;
        let mut flags: Vec<&str> = Vec::new();
        if self.find.regex {
            flags.push("regex");
        }
        if self.find.case {
            flags.push("case sensitive");
        }
        if self.find.whole {
            flags.push("whole word");
        }
        self.fif.flags = if flags.is_empty() { String::new() } else { format!(" ({})", flags.join(", ")) };
        self.fif.query = q.clone();
        self.fif.header = format!("Searching \"{q}\"{}", self.fif.flags);
        self.fif.footer = String::new();
        self.fif.title = String::new();
        self.fif.cancel = Some(cancel);
        self.fif.rx = Some(rx);
        self.panel = Some(BottomPanel::FindResults);
    }

    pub(super) fn poll_find_in_files(&mut self) {
        let Some(rx) = &self.fif.rx else { return };
        let mut done = false;
        while let Ok(m) = rx.try_recv() {
            match m {
                FifMsg::Hit(h) => {
                    if self.fif.hits.len() < 20000 {
                        self.fif.hits.push(h);
                    }
                }
                FifMsg::Files(n) => self.fif.files = n,
                FifMsg::Total(n) => {
                    self.fif.header = format!("Searching {n} file{} for \"{}\"{}", if n == 1 { "" } else { "s" }, self.fif.query, self.fif.flags);
                }
                FifMsg::Done => done = true,
            }
        }
        if done {
            self.fif.rx = None;
            self.fif.cancel = None;
            let files: std::collections::HashSet<&PathBuf> = self.fif.hits.iter().map(|h| &h.path).collect();
            // summary line, sub_140069544: "N matches across M files" / "1 match in 1 file"
            let n = self.fif.hits.len();
            let m = files.len();
            let cancelled = self.fif.cancelled;
            self.fif.footer = format!(
                "{}{}{}",
                if cancelled { "Cancelled!\n" } else { "" },
                if n == 1 { "1 match".to_string() } else { format!("{n} matches") },
                match m {
                    0 => String::new(),
                    1 => " in 1 file".to_string(),
                    k => format!(" across {k} files"),
                }
            );
            self.fif.title = self.fif.footer.clone();
        }
    }

    pub(super) fn open_hit(&mut self, h: Hit) {
        self.push_jump();
        self.open_path(&h.path);
        if let Some(d) = self.docs.get_mut(self.active) {
            let ls: usize = d.text.split('\n').take(h.line).map(|l| l.chars().count() + 1).sum();
            d.view.select = Some((ls + h.col, ls + h.col + h.len));
        }
    }

    fn next_result(&mut self, forward: bool) {
        if self.panel == Some(BottomPanel::Build) {
            let results = self.build_results();
            if results.is_empty() {
                return;
            }
            let n = results.len();
            let i = match self.build.cur {
                None => if forward { 0 } else { n - 1 },
                Some(c) => if forward { (c + 1) % n } else { (c + n - 1) % n },
            };
            self.build.cur = Some(i);
            let (p, line, col, _) = results[i].clone();
            self.open_hit(Hit { path: p, line: line.saturating_sub(1), col: col.saturating_sub(1), len: 0, text: String::new() });
            return;
        }
        let n = self.fif.hits.len();
        if n == 0 {
            return;
        }
        let i = match self.fif.cur {
            None => if forward { 0 } else { n - 1 },
            Some(c) => if forward { (c + 1) % n } else { (c + n - 1) % n },
        };
        self.fif.cur = Some(i);
        let h = self.fif.hits[i].clone();
        self.open_hit(h);
    }

    // ------------------------------------------------------------------
    // build systems

    pub(super) fn build_systems(&self) -> Vec<highlight::BuildSystem> {
        let mut v = highlight::builtin_builds();
        for b in &self.extras.builds {
            v.retain(|x| x.name != b.name);
            v.push(b.clone());
        }
        v
    }

    fn doc_scope(&self) -> String {
        self.docs.get(self.active).map(|d| {
            let name = self.syntax_name_for(d);
            highlight::syntax_by_name(&self.syntax_set, &name).map(|s| s.scope.build_string()).unwrap_or_default()
        }).unwrap_or_default()
    }

    fn matching_builds(&self) -> Vec<highlight::BuildSystem> {
        let scope = self.doc_scope();
        let folders = self.all_folders();
        let mut v: Vec<(i32, highlight::BuildSystem)> = self
            .build_systems()
            .into_iter()
            .filter_map(|b| {
                let sel_ok = !b.selector.is_empty()
                    && b.selector.split(|c: char| c == ',' || c == '|').any(|s| {
                        let s = s.trim().split_whitespace().next().unwrap_or("");
                        !s.is_empty() && scope.starts_with(s)
                    });
                let key_ok = !b.keyfiles.is_empty() && folders.iter().any(|f| b.keyfiles.iter().any(|k| f.join(k).exists()));
                if key_ok {
                    Some((2, b))
                } else if sel_ok {
                    Some((1, b))
                } else {
                    None
                }
            })
            .collect();
        v.sort_by_key(|(s, _)| -*s);
        v.into_iter().map(|(_, b)| b).collect()
    }

    fn expand_vars(&self, s: &str) -> String {
        let d = self.docs.get(self.active);
        let file = d.and_then(|d| d.path.clone()).unwrap_or_default();
        let folder = self.folder.clone().or_else(|| file.parent().map(Path::to_path_buf)).unwrap_or_default();
        let pairs = [
            ("$file_path", file.parent().map(|p| p.display().to_string()).unwrap_or_default()),
            ("$file_base_name", file.file_stem().map(|p| p.to_string_lossy().to_string()).unwrap_or_default()),
            ("$file_extension", file.extension().map(|p| p.to_string_lossy().to_string()).unwrap_or_default()),
            ("$file_name", file.file_name().map(|p| p.to_string_lossy().to_string()).unwrap_or_default()),
            ("$file", file.display().to_string()),
            ("$folder", folder.display().to_string()),
            ("$project_path", self.project_file.as_ref().and_then(|p| p.parent()).map(|p| p.display().to_string()).unwrap_or(folder.display().to_string())),
            ("$packages", std::env::var("APPDATA").map(|a| format!("{a}\\Sublime Text\\Packages")).unwrap_or_default()),
        ];
        let mut out = s.replace("${", "$").replace('}', "");
        if !s.contains("${") {
            out = s.to_string();
        }
        for (k, v) in pairs {
            out = out.replace(k, &v);
        }
        out
    }

    pub(super) fn run_build(&mut self, system: Option<(String, Option<String>)>) {
        if self.build.running.load(Ordering::Relaxed) {
            self.log("A build is already running (Tools ▸ Cancel Build)");
            return;
        }
        let systems = self.build_systems();
        let pick = system.or_else(|| self.build.choice.clone()).and_then(|(n, v)| systems.iter().find(|b| b.name == n).cloned().map(|b| (b, v))).or_else(|| self.matching_builds().into_iter().next().map(|b| (b, None)));
        let Some((b, variant)) = pick else {
            self.log("No build system for this file — pick one in Tools ▸ Build System");
            return;
        };
        if self.s.save_all_on_build {
            self.run("save_all");
        }
        let (cmd, shell) = match &variant {
            Some(v) => b.variants.iter().find(|x| &x.0 == v).map(|x| (x.1.clone(), x.2.clone())).unwrap_or((b.cmd.clone(), b.shell_cmd.clone())),
            None => (b.cmd.clone(), b.shell_cmd.clone()),
        };
        let file_dir = self.docs.get(self.active).and_then(|d| d.path.as_ref()).and_then(|p| p.parent()).map(Path::to_path_buf);
        let wd = if b.working_dir.is_empty() { file_dir.clone() } else { Some(PathBuf::from(self.expand_vars(&b.working_dir))) };
        let wd = wd.filter(|p| p.is_dir()).or(file_dir).or_else(|| self.folder.clone()).unwrap_or_else(|| std::env::temp_dir());
        let mut command = if let Some(sc) = shell {
            let mut c = std::process::Command::new("cmd");
            c.args(["/C", &self.expand_vars(&sc)]);
            c
        } else if let Some(first) = cmd.first() {
            let exe = lsp::which(&self.expand_vars(first)).map(|p| p.display().to_string()).unwrap_or(self.expand_vars(first));
            let mut c = std::process::Command::new(exe);
            c.args(cmd[1..].iter().map(|a| self.expand_vars(a)));
            c
        } else {
            return;
        };
        command.current_dir(&wd).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).stdin(std::process::Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000);
        }
        let title = format!("{}{}", b.name, variant.as_ref().map(|v| format!(" - {v}")).unwrap_or_default());
        let out = Arc::new(Mutex::new(vec![format!("[{title}]  in {}", wd.display())]));
        self.build.output = out.clone();
        self.build.regex = b.file_regex.clone();
        self.build.dir = wd.clone();
        self.build.cur = None;
        self.panel = Some(BottomPanel::Build);
        match command.spawn() {
            Ok(mut child) => {
                let running = Arc::new(AtomicBool::new(true));
                self.build.running = running.clone();
                let stdout = child.stdout.take();
                let stderr = child.stderr.take();
                for stream in [stdout.map(|s| Box::new(s) as Box<dyn Read + Send>), stderr.map(|s| Box::new(s) as Box<dyn Read + Send>)].into_iter().flatten() {
                    let o = out.clone();
                    std::thread::spawn(move || {
                        let r = std::io::BufReader::new(stream);
                        use std::io::BufRead;
                        for line in r.split(b'\n').flatten() {
                            let l = String::from_utf8_lossy(&line).trim_end_matches('\r').to_string();
                            o.lock().unwrap().push(l);
                        }
                    });
                }
                let child = Arc::new(Mutex::new(Some(child)));
                self.build.child = Some(child.clone());
                let o = out.clone();
                let start = Instant::now();
                std::thread::spawn(move || loop {
                    std::thread::sleep(std::time::Duration::from_millis(60));
                    let mut g = child.lock().unwrap();
                    let Some(c) = g.as_mut() else {
                        o.lock().unwrap().push("[Cancelled]".into());
                        running.store(false, Ordering::Relaxed);
                        return;
                    };
                    if let Ok(Some(st)) = c.try_wait() {
                        std::thread::sleep(std::time::Duration::from_millis(80));
                        let code = st.code().unwrap_or(-1);
                        o.lock().unwrap().push(format!("[Finished in {:.1}s{}]", start.elapsed().as_secs_f32(), if code != 0 { format!(" with exit code {code}") } else { String::new() }));
                        running.store(false, Ordering::Relaxed);
                        *g = None;
                        return;
                    }
                });
            }
            Err(e) => {
                out.lock().unwrap().push(format!("Could not start the build: {e}"));
            }
        }
    }

    fn cancel_build(&mut self) {
        if let Some(c) = &self.build.child {
            if let Some(mut ch) = c.lock().unwrap().take() {
                let _ = ch.kill();
            }
        }
    }

    /// (file, line, col, message) parsed from the build output with the system's file_regex.
    pub(super) fn build_results(&self) -> Vec<(PathBuf, usize, usize, String)> {
        if self.build.regex.is_empty() {
            return Vec::new();
        }
        let Ok(re) = fancy_regex::Regex::new(&self.build.regex) else { return Vec::new() };
        let mut v = Vec::new();
        for l in self.build.output.lock().unwrap().iter() {
            if let Ok(Some(c)) = re.captures(l) {
                let f = c.get(1).map(|m| m.as_str()).unwrap_or("");
                if f.is_empty() {
                    continue;
                }
                let p = PathBuf::from(f);
                let p = if p.is_absolute() { p } else { self.build.dir.join(p) };
                let line = c.get(2).and_then(|m| m.as_str().parse().ok()).unwrap_or(1);
                let col = c.get(3).and_then(|m| m.as_str().parse().ok()).unwrap_or(1);
                let msg = c.get(4).map(|m| m.as_str().to_string()).unwrap_or_default();
                v.push((p, line, col, msg));
            }
        }
        v
    }

    // ------------------------------------------------------------------
    // projects

    fn open_project_file(&mut self, p: &Path) {
        let Ok(text) = std::fs::read_to_string(p) else {
            self.log(format!("Could not read {}", p.display()));
            return;
        };
        let Some(v) = highlight::lenient_json(&text) else {
            self.log("That project file is not valid JSON");
            return;
        };
        let base = p.parent().map(Path::to_path_buf).unwrap_or_default();
        let folders: Vec<PathBuf> = v
            .get("folders")
            .and_then(|f| f.as_array())
            .map(|a| a.iter().filter_map(|x| x.get("path").and_then(|p| p.as_str())).map(|s| {
                let pb = PathBuf::from(s);
                if pb.is_absolute() { pb } else { base.join(pb) }
            }).collect())
            .unwrap_or_default();
        self.folder = folders.first().cloned();
        self.extra_folders = folders.into_iter().skip(1).collect();
        self.project_file = Some(p.to_path_buf());
        self.dir_cache.clear();
        self.file_index = None;
        self.s.show_sidebar = true;
        let ps = p.display().to_string();
        self.s.recent_projects.retain(|r| r != &ps);
        self.s.recent_projects.insert(0, ps);
        self.s.recent_projects.truncate(10);
        self.log(format!("Opened project {}", p.display()));
    }

    fn save_project_as(&mut self) {
        let Some(p) = rfd::FileDialog::new().add_filter("Sublime project", &["sublime-project"]).set_file_name("project.sublime-project").save_file() else { return };
        let base = p.parent().map(Path::to_path_buf).unwrap_or_default();
        let folders: Vec<serde_json::Value> = self.all_folders().iter().map(|f| {
            let rel = f.strip_prefix(&base).map(|r| if r.as_os_str().is_empty() { PathBuf::from(".") } else { r.to_path_buf() }).unwrap_or(f.clone());
            serde_json::json!({ "path": rel.display().to_string().replace('\\', "/") })
        }).collect();
        let v = serde_json::json!({ "folders": folders, "settings": {} });
        if std::fs::write(&p, serde_json::to_string_pretty(&v).unwrap_or_default()).is_ok() {
            self.project_file = Some(p.clone());
            let ps = p.display().to_string();
            self.s.recent_projects.retain(|r| r != &ps);
            self.s.recent_projects.insert(0, ps);
            self.log(format!("Saved project {}", p.display()));
        }
    }

    // ------------------------------------------------------------------
    // generated documents

    pub(super) fn open_generated(&mut self, title: &str, text: String, md: bool) {
        if let Some(i) = self.docs.iter().position(|d| d.path.is_none() && d.title == title) {
            self.active = i;
            self.docs[i].text = text;
            self.docs[i].saved_hash = hash_text(&self.docs[i].text);
            return;
        }
        let id = self.new_doc(title, text, None);
        if let Some(d) = self.docs.iter_mut().find(|d| d.id == id) {
            d.saved_hash = hash_text(&d.text);
            d.saved_text = d.text.clone();
            if md {
                d.syntax_override = Some("Markdown".into());
                d.md_mode = MdMode::Preview;
            }
        }
    }

    fn open_settings_file(&mut self) {
        let p = settings_path();
        let _ = std::fs::create_dir_all(config_dir());
        let mut body = serde_json::to_string_pretty(&self.s).unwrap_or_default();
        body = format!("// Bojan Text settings. Edit and save (Ctrl+S) to apply.\n// Delete a line to go back to its default.\n{body}\n");
        let _ = std::fs::write(&p, &body);
        if let Some(i) = self.docs.iter().position(|d| d.path.as_deref() == Some(p.as_path())) {
            self.docs.remove(i);
        }
        self.open_path(&p);
    }

    /// Called after any save: settings / color scheme files apply immediately.
    pub(super) fn after_save(&mut self, path: &Path) {
        if path == settings_path() {
            let text = std::fs::read_to_string(path).unwrap_or_default();
            match highlight::lenient_json(&text).map(serde_json::from_value::<Settings>) {
                Some(Ok(s)) => {
                    self.s = s;
                    if let Some(i) = self.schemes.iter().position(|x| x.name == self.s.scheme) {
                        self.scheme_idx = i;
                    }
                    self.fonts_dirty = true;
                    self.log("Settings applied");
                }
                _ => self.log("Settings not applied: the file is not valid JSON"),
            }
        }
        let is_scheme = path.extension().map(|e| e == "sublime-color-scheme").unwrap_or(false);
        if is_scheme {
            let text = std::fs::read_to_string(path).unwrap_or_default();
            let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            match highlight::load_sublime_scheme(&text, &stem, "User") {
                Some(sc) => {
                    let name = sc.name.clone();
                    self.schemes.retain(|x| x.name != name);
                    self.schemes.push(sc);
                    let i = self.schemes.len() - 1;
                    self.set_scheme(i);
                    self.applied = None;
                    self.log(format!("Color scheme \"{name}\" applied"));
                }
                None => self.log("Color scheme not applied: the file is not valid"),
            }
        }
    }

    fn customize_scheme(&mut self) {
        let sc = self.scheme().clone();
        let dir = config_dir().join("Schemes");
        let _ = std::fs::create_dir_all(&dir);
        let name = if sc.source == "User" { sc.name.clone() } else { format!("{} (custom)", sc.name) };
        let path = dir.join(format!("{name}.sublime-color-scheme"));
        if !path.exists() {
            let _ = std::fs::write(&path, scheme_to_json(&sc.theme, &name));
        }
        self.open_path(&path);
        self.log("Edit the colors and save (Ctrl+S) to apply them");
    }

    // ------------------------------------------------------------------
    // macros

    pub(super) fn macro_capture(&mut self, ctx: &egui::Context) {
        if self.palette.is_some() {
            return;
        }
        if let Some(rec) = self.recording.as_mut() {
            let evs: Vec<egui::Event> = ctx.input(|i| {
                i.events
                    .iter()
                    .filter(|e| match e {
                        egui::Event::Text(_) => true,
                        egui::Event::Key { key, pressed: true, modifiers, .. } => !(*key == Key::Q && modifiers.command),
                        _ => false,
                    })
                    .cloned()
                    .collect()
            });
            rec.extend(evs);
        }
    }

    pub(super) fn macro_playback(&mut self, ctx: &egui::Context, evs: Vec<egui::Event>) {
        if let Some(d) = self.docs.get_mut(self.active) {
            d.view.focus = true;
        }
        ctx.input_mut(|i| i.events.extend(evs));
    }

    fn macros_dir() -> PathBuf {
        config_dir().join("Macros")
    }

    pub(super) fn saved_macros() -> Vec<PathBuf> {
        let mut v: Vec<PathBuf> = std::fs::read_dir(Self::macros_dir()).map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.extension().map(|e| e == "json").unwrap_or(false)).collect()).unwrap_or_default();
        v.sort();
        v
    }

    // ------------------------------------------------------------------
    // the extra commands

    /// Returns false when the id is not one of these commands.
    pub(super) fn run_extra(&mut self, id: &str) -> bool {
        let tab = self.s.tab_size.max(1);
        let unit = if self.s.translate_tabs { " ".repeat(tab) } else { "\t".to_string() };
        match id {
            // File
            "new_window" => {
                if let Ok(exe) = std::env::current_exe() {
                    let _ = std::process::Command::new(exe).arg("--new-window").spawn();
                }
            }
            "close_window" => self.viewport_cmds.push(egui::ViewportCommand::Close),
            // split panes (View ▸ Layout / Groups, File ▸ Split View)
            "split_view" => self.split_view(),
            _ if id.starts_with("layout:") => {
                let mut it = id[7..].split(':');
                let g: usize = it.next().and_then(|x| x.parse().ok()).unwrap_or(1);
                let c: usize = it.next().and_then(|x| x.parse().ok()).unwrap_or(1);
                self.set_layout(g, c);
            }
            "new_group" => self.new_group(),
            "move_new_group" => {
                self.new_group();
                let g = self.groups - 1;
                self.move_file_to_group(g);
            }
            "close_group" => self.close_group(),
            _ if id.starts_with("max_cols:") => self.max_columns = id[9..].parse().unwrap_or(3),
            _ if id.starts_with("focus_group:") => {
                let g: usize = id[12..].parse().unwrap_or(1);
                self.focus_group_to(g.saturating_sub(1));
            }
            "focus_group_next" => {
                let g = (self.focus_group + 1) % self.groups;
                self.focus_group_to(g);
            }
            "focus_group_prev" => {
                let g = (self.focus_group + self.groups - 1) % self.groups;
                self.focus_group_to(g);
            }
            _ if id.starts_with("move_group:") => {
                let g: usize = id[11..].parse().unwrap_or(1);
                self.move_file_to_group(g.saturating_sub(1));
            }
            "move_group_next" => {
                let g = (self.focus_group + 1) % self.groups;
                self.move_file_to_group(g);
            }
            "move_group_prev" => {
                let g = (self.focus_group + self.groups - 1) % self.groups;
                self.move_file_to_group(g);
            }
            _ if id.starts_with("bookmark_goto:") => {
                let l: usize = id[14..].parse().unwrap_or(0);
                self.push_jump();
                self.goto_line0(l);
            }
            "remove_deleted_projects" => self.s.recent_projects.retain(|p| Path::new(p).exists()),
            _ if id.starts_with("pkg_macro:") => {
                // The original's macros run through its macro engine; the common ones
                // from the Default package map onto this editor's own commands.
                let (_, name) = id[10..].split_once('|').unwrap_or(("", ""));
                let cmd = match name {
                    "Add Line Before" => Some("line_before"),
                    "Add Line" => Some("line_after"),
                    "Delete Line" => Some("del_line"),
                    "Delete to BOL" | "Delete to Hard BOL" => Some("del_to_start"),
                    "Delete to EOL" | "Delete to Hard EOL" => Some("del_to_end"),
                    "Add Line in Braces" => Some("line_in_braces"),
                    _ => None,
                };
                match cmd {
                    Some(c) => {
                        self.run(c);
                    }
                    None => self.log(format!("The macro \"{name}\" needs the original's macro engine, which isn't in this version")),
                }
            }
            "line_in_braces" => self.edit_active(|t, lo, _| {
                let c: Vec<char> = t.chars().collect();
                let ls = textops::line_start(&c, lo);
                let ind: String = c[ls..].iter().take_while(|ch| **ch == ' ' || **ch == '\t').collect();
                let ins = format!("\n{ind}\t\n{ind}");
                let mut out: String = c[..lo].iter().collect();
                out.push_str(&ins);
                out.extend(c[lo..].iter());
                let p = lo + 1 + ind.chars().count() + 1;
                Some(Out { text: out, sel: (p, p) })
            }),
            "reindent" =>self.edit_active(|t, lo, hi| Some(textops::reindent(t, lo, hi, &unit))),
            "close_all" => {
                while !self.docs.is_empty() {
                    let n = self.docs.len();
                    self.close_doc(n - 1);
                    if self.docs.len() == n {
                        break; // cancelled
                    }
                }
            }
            "revert" => {
                if let Some(p) = self.docs.get(self.active).and_then(|d| d.path.clone()) {
                    if let Ok(bytes) = std::fs::read(&p) {
                        let enc = self.docs[self.active].encoding;
                        let (t, _) = decode(&bytes, Some(enc));
                        let t = t.replace("\r\n", "\n").replace('\r', "\n");
                        let d = &mut self.docs[self.active];
                        d.text = t.clone();
                        d.saved_text = t;
                        d.saved_hash = hash_text(&d.text);
                        self.log("Reverted to the saved file");
                    }
                }
            }
            "print" => {
                if let Some(d) = self.docs.get(self.active) {
                    let syn = highlight::syntax_by_name(&self.syntax_set, &self.syntax_name_for(d)).unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());
                    let body = syntect::html::highlighted_html_for_string(&d.text, &self.syntax_set, syn, &self.scheme().theme).unwrap_or_default();
                    let html = format!("<!doctype html><html><head><meta charset=utf-8><title>{}</title><style>pre{{font:12px Consolas,monospace;white-space:pre-wrap}}</style></head><body onload=\"window.print()\">{body}</body></html>", d.title);
                    let f = std::env::temp_dir().join("bojan_text_print.html");
                    if std::fs::write(&f, html).is_ok() {
                        markdown::open_url(&f.display().to_string());
                    }
                }
            }
            // Edit
            "undo" | "redo" => {
                let key = if id == "undo" { Key::Z } else { Key::Y };
                self.pending_events.push(egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::COMMAND });
                if let Some(d) = self.docs.get_mut(self.active) {
                    d.view.focus = true;
                }
            }
            "cut" => self.copy_or_cut(true),
            "copy" => self.copy_or_cut(false),
            "paste" => self.paste(false),
            "paste_indent" => self.paste(true),
            "paste_history" => self.open_palette(PaletteMode::PasteHistory),
            "copy_html" => {
                if let Some(d) = self.docs.get(self.active) {
                    let sel = self.selected_text();
                    let src = if sel.is_empty() { d.text.clone() } else { sel };
                    let syn = highlight::syntax_by_name(&self.syntax_set, &self.syntax_name_for(d)).unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());
                    let html = syntect::html::highlighted_html_for_string(&src, &self.syntax_set, syn, &self.scheme().theme).unwrap_or_default();
                    if let Ok(mut cb) = arboard::Clipboard::new() {
                        let _ = cb.set().html(html, Some(src));
                    }
                    self.log("Copied as HTML");
                }
            }
            "indent" => self.edit_active(|t, lo, hi| Some(textops::indent(t, lo, hi, &unit))),
            "unindent" => self.edit_active(|t, lo, hi| Some(textops::unindent(t, lo, hi, tab))),
            "swap_up" => self.edit_active(|t, lo, hi| textops::swap_line(t, lo, hi, true)),
            "swap_down" => self.edit_active(|t, lo, hi| textops::swap_line(t, lo, hi, false)),
            "dup_line" => self.edit_active(|t, lo, hi| Some(textops::duplicate(t, lo, hi))),
            "del_line" => self.edit_active(|t, lo, hi| Some(textops::delete_line(t, lo, hi))),
            "join_lines" => self.edit_active(|t, lo, hi| Some(textops::join_lines(t, lo, hi))),
            "comment" => self.toggle_comment(),
            "block_comment" => {
                let (o, c) = self.block_comment_tokens();
                self.edit_active(|t, lo, hi| Some(textops::toggle_block_comment(t, lo, hi, &o, &c)));
            }
            "revert_mod" => self.revert_modification(),
            "line_before" => self.edit_active(|t, lo, _| Some(textops::insert_line(t, lo, true))),
            "line_after" => self.edit_active(|t, _, hi| Some(textops::insert_line(t, hi, false))),
            "del_word_fwd" => self.edit_active(|t, lo, hi| if lo == hi { Some(textops::delete_word(t, lo, true)) } else { None }),
            "del_word_back" => self.edit_active(|t, lo, hi| if lo == hi { Some(textops::delete_word(t, lo, false)) } else { None }),
            "del_to_end" | "del_to_start" => {
                let end = id == "del_to_end";
                let (lo, _) = self.sel();
                if let Some(d) = self.docs.get(self.active) {
                    let (o, cut) = textops::delete_to(&d.text, lo, end);
                    self.yank = cut;
                    let d = &mut self.docs[self.active];
                    d.text = o.text;
                    d.view.select = Some(o.sel);
                }
            }
            "transpose" => self.edit_active(textops::transpose),
            "close_tag" => self.edit_active(|t, lo, _| textops::close_tag(t, lo)),
            "expand_tag" => {
                // Ctrl+Shift+A: grow to the surrounding tag, else grow the selection normally
                let (lo, hi) = self.sel();
                let scope = self.doc_scope();
                // keymap: tag expansion only where selector is "(text.html, text.xml) - source"
                let markup = (scope.starts_with("text.html") || scope.starts_with("text.xml")) && !scope.starts_with("source");
                if let Some(d) = self.docs.get(self.active) {
                    let c: Vec<char> = d.text.chars().collect();
                    let r = if markup { textops::expand_to_tag(&d.text, lo, hi) } else { None }.or_else(|| Some(textops::expand_smart(&c, lo, hi)));
                    if let Some((a, b)) = r {
                        self.select(a, b);
                    }
                }
            }
            "wrap_tag" => self.edit_active(|t, lo, hi| Some(textops::wrap_with_tag(t, lo, hi, "p"))),
            "set_mark" => {
                if let Some(d) = self.docs.get(self.active) {
                    self.mark = Some((d.id, self.sel().0));
                    self.log("Mark set");
                }
            }
            "select_to_mark" | "delete_to_mark" | "swap_with_mark" => {
                let (lo, _) = self.sel();
                let Some((doc, m)) = self.mark else {
                    self.log("No mark set (Ctrl+K, Ctrl+Space)");
                    return true;
                };
                if self.docs.get(self.active).map(|d| d.id) != Some(doc) {
                    return true;
                }
                let (a, b) = (m.min(lo), m.max(lo));
                match id {
                    "select_to_mark" => self.select(a, b),
                    "delete_to_mark" => {
                        let d = &mut self.docs[self.active];
                        let (ba, bb) = (char_to_byte(&d.text, a), char_to_byte(&d.text, b));
                        self.yank = d.text[ba..bb].to_string();
                        d.text.replace_range(ba..bb, "");
                        d.view.select = Some((a, a));
                    }
                    _ => {
                        self.mark = Some((doc, lo));
                        self.select(m, m);
                    }
                }
            }
            "clear_mark" => self.mark = None,
            "yank" => {
                let y = self.yank.clone();
                self.insert_text(&y);
            }
            "title_case" | "upper" | "lower" | "swap_case" | "camel" | "pascal" | "snake" | "kebab" => {
                let kind = match id {
                    "title_case" => "title",
                    "swap_case" => "swap",
                    k => k,
                };
                self.edit_active(|t, lo, hi| textops::case_op(t, lo, hi, kind));
            }
            _ if id.starts_with("wrap_at:") => {
                let w: usize = id[8..].parse().unwrap_or(0);
                // original order (sub_14039217e / paragraph.py): "wrap_width" setting,
                // then the first ruler, then 78 columns (0x4e)
                let w = if w > 0 {
                    w
                } else if self.s.wrap_column > 0 {
                    self.s.wrap_column
                } else {
                    self.s.rulers.first().copied().filter(|r| *r > 0).unwrap_or(78)
                };
                self.edit_active(|t, lo, hi| Some(textops::wrap_paragraph(t, lo, hi, w)));
            }
            "complete" => self.open_palette(PaletteMode::Complete),
            "sort_lines" | "sort_case" | "reverse_lines" | "unique_lines" | "shuffle_lines" => {
                let kind = match id {
                    "sort_lines" => "sort",
                    "sort_case" => "sort_case",
                    "reverse_lines" => "reverse",
                    "unique_lines" => "unique",
                    _ => "shuffle",
                };
                let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(7);
                self.edit_active(|t, lo, hi| Some(textops::permute(t, lo, hi, kind, seed)));
            }
            // Selection
            "single_selection" => {
                let (lo, _) = self.sel();
                self.select(lo, lo);
            }
            "select_all" => {
                if let Some(d) = self.docs.get(self.active) {
                    let n = d.text.chars().count();
                    self.select(0, n);
                }
            }
            "expand_sel" | "expand_line" | "expand_line_up" | "expand_word" | "expand_para" | "expand_scope" | "expand_brackets" | "expand_indent" => {
                let (lo, hi) = self.sel();
                let Some(d) = self.docs.get(self.active) else { return true };
                let c: Vec<char> = d.text.chars().collect();
                let r = match id {
                    "expand_sel" => Some(textops::expand_smart(&c, lo, hi)),
                    "expand_line" => Some(textops::expand_to_line(&c, lo, hi)),
                    "expand_line_up" => Some(textops::expand_to_line_up(&c, lo, hi)),
                    "expand_word" => Some(textops::word_at(&c, lo)),
                    "expand_para" => Some(textops::paragraph_span(&c, lo, if lo == hi { hi } else { hi })),
                    "expand_scope" => Some(textops::expand_to_scope(&c, lo, hi)),
                    "expand_brackets" => textops::expand_to_brackets(&c, lo, hi),
                    _ => Some(textops::expand_to_indentation(&c, lo, hi)),
                };
                if let Some((a, b)) = r {
                    self.select(a, b);
                }
            }
            "focus_left" | "prev_tab" => {
                if !self.docs.is_empty() {
                    self.active = (self.active + self.docs.len() - 1) % self.docs.len();
                }
            }
            "focus_right" | "next_tab" => {
                if !self.docs.is_empty() {
                    self.active = (self.active + 1) % self.docs.len();
                }
            }
            _ if id.starts_with("tab:") => {
                let n: usize = id[4..].parse().unwrap_or(1);
                if n >= 1 && n <= self.docs.len() {
                    self.active = n - 1;
                }
            }
            // Find
            "find_next" => self.find_step(true),
            "find_prev" => self.find_step(false),
            "replace_next" => self.replace_step(),
            "quick_find" => self.quick_find(false),
            "quick_add_next" => self.quick_find(true),
            "use_sel_find" => {
                self.find.query = self.word_or_selection();
                self.log(format!("Find: {}", self.find.query));
            }
            "use_sel_replace" => {
                self.find.replace = self.selected_text();
                self.log(format!("Replace with: {}", self.find.replace));
            }
            "find_in_files" => {
                self.find.open = true;
                self.find.focus = true;
                self.fif.open = true;
                let w = self.word_or_selection();
                if !w.is_empty() && !w.contains('\n') {
                    self.find.query = w;
                }
                if self.fif.where_.is_empty() {
                    self.fif.where_ = "<open folders>".into();
                }
            }
            "show_results" => self.panel = Some(BottomPanel::FindResults),
            "next_result" => self.next_result(true),
            "prev_result" => self.next_result(false),
            "cancel_fif" => {
                if let Some(c) = &self.fif.cancel {
                    c.store(true, Ordering::Relaxed);
                    self.fif.cancelled = true;
                }
            }
            // View
            "toggle_open_files" => self.s.show_open_files = !self.s.show_open_files,
            "toggle_tabs" => self.s.show_tabs = !self.s.show_tabs,
            "toggle_status" => self.s.show_status_bar = !self.s.show_status_bar,
            "toggle_menu" => self.s.show_menu = !self.s.show_menu,
            "console" => self.panel = if self.panel == Some(BottomPanel::Console) { None } else { Some(BottomPanel::Console) },
            "fullscreen" => {
                self.fullscreen = !self.fullscreen;
                self.viewport_cmds.push(egui::ViewportCommand::Fullscreen(self.fullscreen));
            }
            "distraction_free" => {
                if let Some(prev) = self.distraction.take() {
                    (self.s.show_sidebar, self.s.show_minimap, self.s.show_tabs, self.s.show_status_bar, self.s.show_menu) = prev;
                    self.fullscreen = false;
                } else {
                    self.distraction = Some((self.s.show_sidebar, self.s.show_minimap, self.s.show_tabs, self.s.show_status_bar, self.s.show_menu));
                    self.s.show_sidebar = false;
                    self.s.show_minimap = false;
                    self.s.show_tabs = false;
                    self.s.show_status_bar = false;
                    self.s.show_menu = false;
                    self.fullscreen = true;
                }
                self.viewport_cmds.push(egui::ViewportCommand::Fullscreen(self.fullscreen));
            }
            "toggle_spaces" => self.s.translate_tabs = !self.s.translate_tabs,
            _ if id.starts_with("tab_width:") => self.s.tab_size = id[10..].parse().unwrap_or(4),
            "detect_indent" => self.detect_indentation(),
            _ if id.starts_with("line_ending:") => {
                let le = match &id[12..] {
                    "windows" => LineEnding::Windows,
                    "unix" => LineEnding::Unix,
                    _ => LineEnding::Mac,
                };
                if let Some(d) = self.docs.get_mut(self.active) {
                    d.line_ending = le;
                    d.saved_hash = 0; // needs saving
                }
            }
            _ if id.starts_with("wrap_col:") => {
                self.s.wrap_column = id[9..].parse().unwrap_or(0);
                self.s.word_wrap = true;
            }
            _ if id.starts_with("ruler:") => {
                let n: usize = id[6..].parse().unwrap_or(0);
                self.s.rulers = if n == 0 { Vec::new() } else { vec![n] };
            }
            // Goto
            "goto_symbol" => self.open_palette(PaletteMode::Symbols),
            "goto_symbol_project" => self.open_palette(PaletteMode::ProjectSymbols),
            "goto_reference" => self.lsp_references(),
            "next_mod" | "prev_mod" => {
                if let Some((a, _)) = self.modified_range(self.active) {
                    self.push_jump();
                    self.goto_line0(a);
                } else {
                    self.log("No changes since the last save");
                }
            }
            "jump_back" => self.go_jump(true),
            "jump_forward" => self.go_jump(false),
            "next_in_stack" | "prev_in_stack" => {
                let ids: Vec<u64> = self.mru.clone();
                if ids.len() > 1 {
                    let target = if id == "next_in_stack" { ids[1] } else { *ids.last().unwrap() };
                    if let Some(i) = self.docs.iter().position(|d| d.id == target) {
                        self.active = i;
                    }
                }
            }
            "switch_header" => self.switch_header(),
            "scroll_to_sel" => {
                let (lo, hi) = self.sel();
                self.select(lo, hi);
            }
            "scroll_up" | "scroll_down" => {
                let lh = self.s.theme.editor_font_size * 1.2 + self.s.theme.line_padding;
                if let Some(d) = self.docs.get_mut(self.active) {
                    let y = d.view.scroll_y + if id == "scroll_up" { -lh } else { lh };
                    d.view.set_scroll = Some(y.max(0.0));
                }
            }
            "toggle_bookmark" => {
                let l = self.current_line();
                if let Some(d) = self.docs.get_mut(self.active) {
                    if let Some(p) = d.bookmarks.iter().position(|b| *b == l) {
                        d.bookmarks.remove(p);
                    } else {
                        d.bookmarks.push(l);
                        d.bookmarks.sort();
                    }
                }
            }
            "next_bookmark" | "prev_bookmark" => {
                let l = self.current_line();
                let Some(d) = self.docs.get(self.active) else { return true };
                let bm = &d.bookmarks;
                let t = if id == "next_bookmark" { bm.iter().find(|b| **b > l).or(bm.first()) } else { bm.iter().rev().find(|b| **b < l).or(bm.last()) };
                if let Some(&t) = t {
                    self.push_jump();
                    self.goto_line0(t);
                }
            }
            "clear_bookmarks" => {
                if let Some(d) = self.docs.get_mut(self.active) {
                    d.bookmarks.clear();
                }
            }
            "matching_bracket" => {
                let (lo, _) = self.sel();
                if let Some(d) = self.docs.get(self.active) {
                    let c: Vec<char> = d.text.chars().collect();
                    if let Some(m) = textops::matching_bracket(&c, lo) {
                        let p = if m < lo { m } else { m + 1 };
                        self.select(p, p);
                    }
                }
            }
            // Tools
            "snippets" => self.open_palette(PaletteMode::Snippets),
            "build" => self.run_build(None),
            "build_with" => self.open_palette(PaletteMode::BuildWith),
            "cancel_build" => self.cancel_build(),
            "show_build" => self.panel = Some(BottomPanel::Build),
            "toggle_save_on_build" => self.s.save_all_on_build = !self.s.save_all_on_build,
            "build_auto" => self.build.choice = None,
            "record_macro" => {
                if let Some(m) = self.recording.take() {
                    self.log(format!("Macro recorded ({} steps) — Ctrl+Shift+Q plays it back", m.len()));
                    self.last_macro = m;
                } else {
                    self.recording = Some(Vec::new());
                    self.log("Recording macro… press Ctrl+Q again to stop");
                }
            }
            "play_macro" => {
                let m = self.last_macro.clone();
                self.play_macro = Some(m);
            }
            "save_macro" => {
                if self.last_macro.is_empty() {
                    self.log("Record a macro first (Ctrl+Q)");
                } else {
                    let _ = std::fs::create_dir_all(Self::macros_dir());
                    if let Some(p) = rfd::FileDialog::new().set_directory(Self::macros_dir()).set_file_name("macro.json").save_file() {
                        let _ = std::fs::write(&p, serde_json::to_string(&self.last_macro).unwrap_or_default());
                        self.log(format!("Saved macro {}", p.display()));
                    }
                }
            }
            _ if id.starts_with("macro_file:") => {
                let p = PathBuf::from(&id[11..]);
                if let Some(m) = std::fs::read_to_string(&p).ok().and_then(|t| serde_json::from_str::<Vec<egui::Event>>(&t).ok()) {
                    self.last_macro = m.clone();
                    self.play_macro = Some(m);
                }
            }
            "new_snippet" => self.open_generated(
                "untitled.sublime-snippet",
                "<snippet>\n\t<content><![CDATA[\nHello, ${1:this} is a ${2:snippet}.\n]]></content>\n\t<!-- Optional: a word that triggers it -->\n\t<!-- <tabTrigger>hello</tabTrigger> -->\n\t<!-- Optional: where it applies -->\n\t<!-- <scope>source.python</scope> -->\n</snippet>\n".into(),
                false,
            ),
            "new_syntax" => self.open_generated(
                "untitled.sublime-syntax",
                "%YAML 1.2\n---\n# See the Sublime Text syntax documentation\nname: My Language\nfile_extensions: [mylang]\nscope: source.mylang\n\ncontexts:\n  main:\n    - match: '\\b(if|else|for|while)\\b'\n      scope: keyword.control.mylang\n    - match: '\"'\n      push: string\n\n  string:\n    - meta_scope: string.quoted.double.mylang\n    - match: '\\\\.'\n      scope: constant.character.escape.mylang\n    - match: '\"'\n      pop: true\n".into(),
                false,
            ),
            "view_package_file" => self.open_palette(PaletteMode::PackageFiles),
            "scope_name" => {
                let (lo, _) = self.sel();
                if let Some(d) = self.docs.get(self.active) {
                    let name = self.syntax_name_for(d);
                    let syn = highlight::syntax_by_name(&self.syntax_set, &name).unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());
                    let scope = highlight::scope_at(&d.text, syn, &self.syntax_set, char_to_byte(&d.text, lo));
                    self.scope_popup = Some(scope.clone());
                    self.console.push(format!("Scope: {scope}"));
                }
            }
            "package_control" => markdown::open_url("https://packagecontrol.io"),
            // Project
            "open_project" => {
                if let Some(p) = rfd::FileDialog::new().add_filter("Sublime project", &["sublime-project"]).pick_file() {
                    self.open_project_file(&p);
                }
            }
            "switch_project" => self.open_palette(PaletteMode::Projects),
            _ if id.starts_with("project:") => {
                let p = PathBuf::from(&id[8..]);
                self.open_project_file(&p);
            }
            "save_project" => self.save_project_as(),
            "close_project" => {
                self.folder = None;
                self.extra_folders.clear();
                self.project_file = None;
                self.dir_cache.clear();
                self.file_index = None;
            }
            "edit_project" => {
                if let Some(p) = self.project_file.clone() {
                    self.open_path(&p);
                } else {
                    self.log("No project open — Project ▸ Save Project As… creates one");
                }
            }
            "add_folder" => {
                if let Some(f) = rfd::FileDialog::new().pick_folder() {
                    if self.folder.is_none() {
                        self.folder = Some(f);
                    } else if !self.extra_folders.contains(&f) {
                        self.extra_folders.push(f);
                    }
                    self.s.show_sidebar = true;
                    self.file_index = None;
                }
            }
            "remove_folders" => {
                self.folder = None;
                self.extra_folders.clear();
                self.file_index = None;
            }
            "refresh_folders" => {
                self.dir_cache.clear();
                self.file_index = None;
            }
            "clear_recent_projects" => self.s.recent_projects.clear(),
            "clear_recent" => self.s.recent.clear(),
            // Preferences
            "browse_packages" => {
                let p = std::env::var("APPDATA").map(|a| PathBuf::from(a).join("Sublime Text").join("Packages")).ok().filter(|p| p.is_dir()).unwrap_or_else(config_dir);
                let _ = std::fs::create_dir_all(&p);
                let _ = std::process::Command::new("explorer").arg(&p).spawn();
            }
            "settings" => self.open_settings_file(),
            "key_bindings" => self.open_generated("Key Bindings.md", super::menus::key_bindings_doc(), true),
            "mouse_bindings" => self.open_generated("Mouse Bindings.md", super::menus::mouse_bindings_doc(), true),
            "customize_scheme" => self.customize_scheme(),
            "choose_font" => self.open_palette(PaletteMode::Fonts),
            "font_reset" => self.s.theme.editor_font_size = ThemeOptions::default().editor_font_size,
            // Help
            "help" => self.open_generated("Bojan Text Help.md", super::menus::help_doc(), true),
            "changelog" => self.open_generated("Changelog.md", super::menus::changelog_doc(), true),
            "licenses" => self.open_generated("Licenses and Attribution.md", super::menus::licenses_doc(), true),
            _ => return false,
        }
        true
    }

    pub(super) fn block_comment_tokens(&self) -> (String, String) {
        let ext = self.docs.get(self.active).map(|d| if d.path.is_some() { d.ext() } else { d.title.rsplit('.').next().unwrap_or("").to_string() }).unwrap_or_default();
        let (a, b) = match ext.as_str() {
            "html" | "htm" | "xml" | "md" | "markdown" | "svg" | "vue" => ("<!-- ", " -->"),
            "py" => ("\"\"\" ", " \"\"\""),
            "lua" => ("--[[ ", " ]]"),
            "hs" => ("{- ", " -}"),
            _ => ("/* ", " */"),
        };
        (a.to_string(), b.to_string())
    }

    fn lsp_references(&mut self) {
        let Some(d) = self.docs.get(self.active) else { return };
        let ch = self.sel().0;
        if let (Some(p), Some(spec)) = (d.path.clone(), self.spec_for(&d.ext())) {
            let (line, col) = Self::lsp_position(d, ch);
            if let Some(c) = self.clients.get_mut(&spec.name) {
                c.references(&p, line, col);
                return;
            }
        }
        // no language server: search the word in all files
        let w = self.word_or_selection();
        if w.is_empty() {
            return;
        }
        self.find.query = w;
        self.find.case = true;
        self.find.regex = false;
        self.start_find_in_files();
    }
}

/// Write a theme as a `.sublime-color-scheme` file the user can edit.
pub fn scheme_to_json(t: &syntect::highlighting::Theme, name: &str) -> String {
    let hex = |c: syntect::highlighting::Color| if c.a == 255 { format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b) } else { format!("#{:02x}{:02x}{:02x}{:02x}", c.r, c.g, c.b, c.a) };
    let s = &t.settings;
    let mut globals = serde_json::Map::new();
    for (k, v) in [
        ("foreground", s.foreground),
        ("background", s.background),
        ("caret", s.caret),
        ("accent", s.accent),
        ("line_highlight", s.line_highlight),
        ("selection", s.selection),
        ("gutter_foreground", s.gutter_foreground),
        ("find_highlight", s.find_highlight),
    ] {
        if let Some(c) = v {
            globals.insert(k.into(), serde_json::Value::String(hex(c)));
        }
    }
    let rules: Vec<serde_json::Value> = t
        .scopes
        .iter()
        .map(|it| {
            let scope = it
                .scope
                .selectors
                .iter()
                .map(|sel| {
                    let mut p = sel.path.to_string();
                    for e in &sel.excludes {
                        p.push_str(&format!(" - {e}"));
                    }
                    p
                })
                .collect::<Vec<_>>()
                .join(", ");
            let mut r = serde_json::Map::new();
            r.insert("scope".into(), scope.into());
            if let Some(c) = it.style.foreground {
                r.insert("foreground".into(), hex(c).into());
            }
            if let Some(c) = it.style.background {
                r.insert("background".into(), hex(c).into());
            }
            if let Some(fs) = it.style.font_style {
                let mut v = Vec::new();
                if fs.contains(syntect::highlighting::FontStyle::BOLD) {
                    v.push("bold");
                }
                if fs.contains(syntect::highlighting::FontStyle::ITALIC) {
                    v.push("italic");
                }
                if fs.contains(syntect::highlighting::FontStyle::UNDERLINE) {
                    v.push("underline");
                }
                if !v.is_empty() {
                    r.insert("font_style".into(), v.join(" ").into());
                }
            }
            serde_json::Value::Object(r)
        })
        .collect();
    let v = serde_json::json!({ "name": name, "author": "Bojan Text", "variables": {}, "globals": globals, "rules": rules });
    serde_json::to_string_pretty(&v).unwrap_or_default()
}

use std::io::Read;
