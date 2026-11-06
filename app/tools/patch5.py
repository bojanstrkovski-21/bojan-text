# One-off patch: Find Results text in the original's format
# (sub_140068cc4 header, sub_140068eaa file blocks, sub_140069544 summary).
import io

def rd(p):
    return io.open(p, encoding='utf-8').read()

def wr(p, s):
    io.open(p, 'w', encoding='utf-8', newline='\n').write(s)

def rep(s, old, new, count=1):
    assert old in s, old[:70]
    return s.replace(old, new, count)

p = 'src/app/features.rs'
s = rd(p)
s = rep(s, "    pub title: String,\n}", "    pub title: String,\n    /// `Searching N files for \"q\" (regex, case sensitive, whole word)` (sub_140068cc4)\n    pub header: String,\n    pub flags: String,\n    pub query: String,\n    /// `N matches across M files` (sub_140069544), shown when the search is done\n    pub footer: String,\n}")
s = rep(s, "pub enum FifMsg {", "pub enum FifMsg {\n    Total(usize),")
# thread: tell the total number of files first
s = rep(s, "            let mut n = 0;\n            for f in files {\n                if cc.load(Ordering::Relaxed) {\n                    break;\n                }",
        "            let _ = tx.send(FifMsg::Total(files.len()));\n            let mut n = 0;\n            for f in files {\n                if cc.load(Ordering::Relaxed) {\n                    break;\n                }")
s = rep(s, "        self.fif.title = format!(\"Searching for \\\"{q}\\\"\");",
        '''        let mut flags: Vec<&str> = Vec::new();
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
        self.fif.header = format!("Searching \\"{q}\\"{}", self.fif.flags);
        self.fif.footer = String::new();
        self.fif.title = String::new();''')
s = rep(s, "                FifMsg::Files(n) => self.fif.files = n,",
        "                FifMsg::Files(n) => self.fif.files = n,\n                FifMsg::Total(n) => {\n                    self.fif.header = format!(\"Searching {n} file{} for \\\"{}\\\"{}\", if n == 1 { \"\" } else { \"s\" }, self.fif.query, self.fif.flags);\n                }")
old_done = s[s.index("            self.fif.title = format!(\"{} matches across"):]
old_done = old_done[:old_done.index("\n") + 1]
s = s.replace(old_done, '''            // summary line, sub_140069544: "N matches across M files" / "1 match in 1 file"
            let n = self.fif.hits.len();
            let m = files.len();
            let cancelled = self.fif.cancelled;
            self.fif.footer = format!(
                "{}{}{}",
                if cancelled { "Cancelled!\\n" } else { "" },
                if n == 1 { "1 match".to_string() } else { format!("{n} matches") },
                match m {
                    0 => String::new(),
                    1 => " in 1 file".to_string(),
                    k => format!(" across {k} files"),
                }
            );
            self.fif.title = self.fif.footer.clone();
''', 1)
s = rep(s, "    pub footer: String,\n}", "    pub footer: String,\n    pub cancelled: bool,\n}")
# cancel flag
s = rep(s, "            \"cancel_fif\" => {\n                if let Some(c) = &self.fif.cancel {\n                    c.store(true, Ordering::Relaxed);\n                }",
        "            \"cancel_fif\" => {\n                if let Some(c) = &self.fif.cancel {\n                    c.store(true, Ordering::Relaxed);\n                    self.fif.cancelled = true;\n                }")
s = rep(s, "        self.fif.hits.clear();\n        self.fif.cur = None;\n        self.fif.files = 0;", "        self.fif.hits.clear();\n        self.fif.cur = None;\n        self.fif.files = 0;\n        self.fif.cancelled = false;")
wr(p, s)

p = 'src/app/keys.rs'
s = rd(p)
a = s.index("            Some(BottomPanel::FindResults) => {")
b = s.index("            Some(BottomPanel::Build) => {")
NEW = '''            Some(BottomPanel::FindResults) => {
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
'''
s = s[:a] + NEW + s[b:]
wr(p, s)
print('patched5')
