//! Lists for the quick panels added with the full menu set: paste history,
//! completions, symbols, snippets, build variants, package files, projects, fonts.

use super::*;

impl App {
    pub(super) fn palette_items(&mut self, m: PaletteMode) -> Vec<(String, String, String)> {
        match m {
            PaletteMode::PasteHistory => self
                .paste_history
                .iter()
                .enumerate()
                .map(|(i, t)| {
                    let first: String = t.lines().next().unwrap_or("").chars().take(80).collect();
                    let more = t.lines().count();
                    (first, format!("paste_hist:{i}"), if more > 1 { format!("{more} lines") } else { String::new() })
                })
                .collect(),
            PaletteMode::Complete => {
                let Some(d) = self.docs.get(self.active) else { return Vec::new() };
                let (lo, _) = self.sel();
                let c: Vec<char> = d.text.chars().collect();
                let mut a = lo;
                while a > 0 && (c[a - 1].is_alphanumeric() || c[a - 1] == '_') {
                    a -= 1;
                }
                let prefix: String = c[a..lo].iter().collect();
                let pl = prefix.to_lowercase();
                let mut words: Vec<String> = Vec::new();
                let mut seen = HashSet::new();
                for w in d.text.split(|ch: char| !(ch.is_alphanumeric() || ch == '_')) {
                    if w.chars().count() >= 3 && w != prefix && w.to_lowercase().starts_with(&pl) && seen.insert(w.to_string()) {
                        words.push(w.to_string());
                    }
                }
                let mut v: Vec<(String, String, String)> = words.into_iter().take(300).map(|w| (w.clone(), format!("complete:{}|{w}", prefix.chars().count()), String::new())).collect();
                // snippets for this file type
                let scope = self.active_scope();
                for (i, sn) in self.extras.snippets.iter().enumerate() {
                    if !sn.trigger.is_empty() && sn.trigger.to_lowercase().starts_with(&pl) && scope_matches(&sn.scope, &scope) {
                        v.push((sn.trigger.clone(), format!("snippet_at:{}|{i}", prefix.chars().count()), format!("snippet · {}", sn.description)));
                    }
                }
                self.pal_query = String::new();
                v
            }
            PaletteMode::Symbols => {
                let Some(d) = self.docs.get(self.active) else { return Vec::new() };
                let name = self.syntax_name_for(d);
                let syn = highlight::syntax_by_name(&self.syntax_set, &name).unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());
                highlight::symbols(&d.text, syn, &self.syntax_set)
                    .into_iter()
                    .map(|(n, b)| {
                        let ch = byte_to_char(&d.text, b);
                        let line = d.text[..b].matches('\n').count() + 1;
                        (n, format!("sym:{ch}"), format!("line {line}"))
                    })
                    .collect()
            }
            PaletteMode::ProjectSymbols => {
                let mut files: Vec<PathBuf> = Vec::new();
                for f in self.all_folders() {
                    files.extend(index_files(&f));
                }
                let open: Vec<(PathBuf, String)> = self.docs.iter().filter_map(|d| d.path.clone().map(|p| (p, d.text.clone()))).collect();
                if files.is_empty() {
                    files = open.iter().map(|(p, _)| p.clone()).collect();
                }
                let ss = self.syntax_set.clone();
                let (tx, rx) = std::sync::mpsc::channel();
                std::thread::spawn(move || {
                    let mut out = Vec::new();
                    for f in files.into_iter().take(1500) {
                        let text = match open.iter().find(|(p, _)| *p == f) {
                            Some((_, t)) => t.clone(),
                            None => {
                                if std::fs::metadata(&f).map(|m| m.len() > 600_000).unwrap_or(true) {
                                    continue;
                                }
                                match std::fs::read(&f) {
                                    Ok(b) if !b.iter().take(2048).any(|x| *x == 0) => String::from_utf8_lossy(&b).replace("\r\n", "\n"),
                                    _ => continue,
                                }
                            }
                        };
                        let syn = highlight::find_syntax(&ss, Some(&f), text.lines().next().unwrap_or(""));
                        if syn.name == "Plain Text" {
                            continue;
                        }
                        let fname = f.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                        for (n, b) in highlight::symbols(&text, syn, &ss) {
                            let line = text[..b].matches('\n').count() + 1;
                            out.push((n, format!("psym:{}|{b}", f.display()), format!("{fname}:{line}")));
                        }
                        if out.len() > 50_000 {
                            break;
                        }
                    }
                    let _ = tx.send(out);
                });
                self.project_symbols_rx = Some(rx);
                Vec::new()
            }
            PaletteMode::Snippets => {
                let scope = self.active_scope();
                self.extras
                    .snippets
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| scope_matches(&s.scope, &scope))
                    .map(|(i, s)| (s.description.clone(), format!("snippet_at:0|{i}"), if s.trigger.is_empty() { s.package.clone() } else { format!("{}  ·  {}", s.trigger, s.package) }))
                    .collect()
            }
            PaletteMode::BuildWith => {
                let mut v = Vec::new();
                for b in self.build_systems() {
                    v.push((b.name.clone(), format!("buildwith:{}|", b.name), b.package.clone()));
                    for (vn, _, _) in &b.variants {
                        v.push((format!("{} - {vn}", b.name), format!("buildwith:{}|{vn}", b.name), b.package.clone()));
                    }
                }
                v
            }
            PaletteMode::PackageFiles => {
                let mut v = Vec::new();
                for dir in highlight::default_package_dirs() {
                    let Ok(rd) = std::fs::read_dir(&dir) else { continue };
                    for e in rd.flatten() {
                        let p = e.path();
                        if p.extension().map(|x| x == "sublime-package").unwrap_or(false) {
                            let pkg = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                            if let Ok(f) = std::fs::File::open(&p) {
                                if let Ok(mut z) = zip::ZipArchive::new(f) {
                                    for i in 0..z.len() {
                                        if let Ok(en) = z.by_index(i) {
                                            if let Ok(n) = en.name().map(|s| s.to_string()) {
                                                if !n.ends_with('/') {
                                                    v.push((format!("{pkg}/{n}"), format!("pkgfile:{}|{n}", p.display()), String::new()));
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                v
            }
            PaletteMode::Projects => self.s.recent_projects.iter().map(|p| {
                let name = Path::new(p).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                (name, format!("project:{p}"), p.clone())
            }).collect(),
            PaletteMode::Fonts => {
                let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
                let mut v: Vec<(String, String, String)> = std::fs::read_dir(Path::new(&windir).join("Fonts"))
                    .map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.extension().map(|x| { let x = x.to_string_lossy().to_ascii_lowercase(); x == "ttf" || x == "otf" }).unwrap_or(false))
                        .map(|p| (p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(), format!("font:{}", p.display()), String::new())).collect())
                    .unwrap_or_default();
                v.sort();
                v.insert(0, ("Consolas (default)".into(), "font:".into(), String::new()));
                v
            }
            _ => Vec::new(),
        }
    }

    fn active_scope(&self) -> String {
        self.docs.get(self.active).map(|d| {
            let name = self.syntax_name_for(d);
            highlight::syntax_by_name(&self.syntax_set, &name).map(|s| s.scope.build_string()).unwrap_or_default()
        }).unwrap_or_default()
    }

    /// Handle a chosen quick-panel id from the lists above. Returns true when handled.
    pub(super) fn palette_choose(&mut self, id: &str) -> bool {
        if let Some(i) = id.strip_prefix("paste_hist:") {
            if let Some(t) = self.paste_history.get(i.parse::<usize>().unwrap_or(0)).cloned() {
                self.replace_selection(&t, 0);
            }
        } else if let Some(rest) = id.strip_prefix("complete:") {
            let (n, w) = rest.split_once('|').unwrap_or(("0", ""));
            self.replace_selection(w, n.parse().unwrap_or(0));
        } else if let Some(rest) = id.strip_prefix("snippet_at:") {
            let (n, i) = rest.split_once('|').unwrap_or(("0", "0"));
            if let Some(sn) = self.extras.snippets.get(i.parse::<usize>().unwrap_or(0)).cloned() {
                let (body, at) = crate::textops::expand_snippet(&sn.content);
                let back = n.parse().unwrap_or(0);
                let (lo, _) = self.sel();
                self.replace_selection(&body, back);
                let start = lo - back.min(lo);
                self.select(start + at, start + at);
            }
        } else if let Some(c) = id.strip_prefix("sym:") {
            let c: usize = c.parse().unwrap_or(0);
            self.push_jump();
            self.select(c, c);
        } else if let Some(rest) = id.strip_prefix("psym:") {
            let (p, b) = rest.rsplit_once('|').unwrap_or((rest, "0"));
            self.push_jump();
            self.open_path(Path::new(p));
            if let Some(d) = self.docs.get_mut(self.active) {
                let c = byte_to_char(&d.text, b.parse().unwrap_or(0));
                d.view.select = Some((c, c));
            }
        } else if let Some(rest) = id.strip_prefix("buildwith:") {
            let (n, v) = rest.split_once('|').unwrap_or((rest, ""));
            let choice = (n.to_string(), if v.is_empty() { None } else { Some(v.to_string()) });
            self.build.choice = Some(choice.clone());
            self.run_build(Some(choice));
        } else if let Some(rest) = id.strip_prefix("pkgfile:") {
            let (p, inner) = rest.split_once('|').unwrap_or((rest, ""));
            let text = std::fs::File::open(p).ok().and_then(|f| zip::ZipArchive::new(f).ok()).and_then(|mut z| {
                let mut e = z.by_name(inner).ok()?;
                let mut s = String::new();
                std::io::Read::read_to_string(&mut e, &mut s).ok()?;
                Some(s)
            });
            if let Some(t) = text {
                let pkg = Path::new(p).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                let short = inner.rsplit('/').next().unwrap_or(inner);
                self.open_generated(&format!("{short} ({pkg})"), t.replace("\r\n", "\n"), false);
            } else {
                self.log("That package file isn't text");
            }
        } else if let Some(f) = id.strip_prefix("font:") {
            self.s.font_file = if f.is_empty() { None } else { Some(f.to_string()) };
            self.fonts_dirty = true;
        } else {
            return false;
        }
        true
    }

    /// Replace the selection (and `back` chars before it) with `t`.
    fn replace_selection(&mut self, t: &str, back: usize) {
        let t = t.to_string();
        self.edit_active(|text, lo, hi| {
            let c: Vec<char> = text.chars().collect();
            let a = lo - back.min(lo);
            let mut out: String = c[..a].iter().collect();
            out.push_str(&t);
            out.extend(c[hi..].iter());
            let p = a + t.chars().count();
            Some(crate::textops::Out { text: out, sel: (p, p) })
        });
    }
}

/// Does a snippet/build `selector` apply to a syntax scope (e.g. "source.python")?
pub fn scope_matches(selector: &str, scope: &str) -> bool {
    if selector.trim().is_empty() {
        return true;
    }
    selector.split(|c: char| c == ',' || c == '|').any(|part| {
        let first = part.trim().split_whitespace().next().unwrap_or("");
        let first = first.trim_start_matches('(');
        !first.is_empty() && !first.starts_with('-') && scope.starts_with(first)
    })
}
