# One-off source patch: find options from the original's find panel
# (sub_140070bdd: in_selection, regex, case_sensitive, whole_word, reverse, wrap, highlight, ...).
import io

def rd(p):
    return io.open(p, encoding='utf-8').read()

def wr(p, s):
    io.open(p, 'w', encoding='utf-8', newline='\n').write(s)

def rep(s, old, new, count=1):
    assert old in s, old[:70]
    return s.replace(old, new, count)

# ---------------- app.rs: FindState
p = 'src/app.rs'
s = rd(p)
s = rep(s, "    case: bool,\n    regex: bool,\n    focus: bool,\n}", "    case: bool,\n    regex: bool,\n    /// whole_word\n    whole: bool,\n    /// wrap (default on)\n    wrap: bool,\n    /// highlight matches (default on)\n    highlight: bool,\n    /// in_selection: search only inside this char range\n    in_sel: Option<(usize, usize)>,\n    focus: bool,\n}")
s = rep(s, "            find: FindState::default(),", "            find: FindState { wrap: true, highlight: true, ..Default::default() },")
wr(p, s)

# ---------------- features.rs
p = 'src/app/features.rs'
s = rd(p)
# helper methods before find_step
HELP = '''    /// The query as (pattern, match case, regex) with `whole_word` applied.
    pub(super) fn find_spec(&self) -> (String, bool, bool) {
        let q = self.find.query.clone();
        if self.find.whole && !q.is_empty() {
            let inner = if self.find.regex { format!("(?:{q})") } else { fancy_regex::escape(&q).to_string() };
            (format!(r"\\b{inner}\\b"), self.find.case, true)
        } else {
            (q, self.find.case, self.find.regex)
        }
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

    pub(super) fn find_step(&mut self, forward: bool) {'''
s = rep(s, "    pub(super) fn find_step(&mut self, forward: bool) {", HELP)
# find_step: use find_all + wrap
s = rep(s, "        let matches = editor::find_matches(&d.text, &q, case, regex);\n        if matches.is_empty() {",
        "        let matches = self.find_all(&d.text);\n        let _ = (case, regex);\n        if matches.is_empty() {")
s = rep(s, "            matches.iter().find(|m| m.0 >= cur && !(m.0 == cur && m.1 == cur)).or(matches.first())",
        "            matches.iter().find(|m| m.0 >= cur && !(m.0 == cur && m.1 == cur)).or(if self.find.wrap { matches.first() } else { None })")
s = rep(s, "            matches.iter().rev().find(|m| m.1 <= cur && m.0 < cur).or(matches.last())",
        "            matches.iter().rev().find(|m| m.1 <= cur && m.0 < cur).or(if self.find.wrap { matches.last() } else { None })")
# replace_step: matching through the spec; expansion only for real regex mode
s = rep(s, "            let hit = editor::find_matches(&sel, &q, case, regex).first().map(|m| *m == (0, sel.len())).unwrap_or(false);",
        "            let (sq, sc, sr) = (self.find_spec_for(&q));\n            let hit = editor::find_matches(&sel, &sq, sc, sr).first().map(|m| *m == (0, sel.len())).unwrap_or(false);")
s = rep(s, "                let new = if regex {\n                    let pat = if case { q.clone() } else { format!(\"(?i){q}\") };",
        "                let new = if regex {\n                    let pat = if case { sq.clone() } else { format!(\"(?i){sq}\") };")
# replace_all
s = rep(s, "            let m = editor::find_matches(&d.text, &q, case, false);\n            n = m.len();",
        "            let m = { let (sq, sc, sr) = (spec_q.clone(), spec_c, spec_r); let mut v = editor::find_matches(&d.text, &sq, sc, sr); if let Some((a, b)) = in_sel { let (ba, bb) = (char_to_byte(&d.text, a), char_to_byte(&d.text, b)); v.retain(|x| x.0 >= ba && x.1 <= bb); } v };\n            n = m.len();")
s = rep(s, "        let rep = self.find.replace.clone();\n        let (case, regex) = (self.find.case, self.find.regex);\n        let Some(d) = self.docs.get_mut(self.active) else { return };\n        let n;",
        "        let rep = self.find.replace.clone();\n        let (case, regex) = (self.find.case, self.find.regex);\n        let (spec_q, spec_c, spec_r) = self.find_spec();\n        let in_sel = self.find.in_sel;\n        let Some(d) = self.docs.get_mut(self.active) else { return };\n        let n;")
s = rep(s, "            let pat = if case { q.clone() } else { format!(\"(?i){q}\") };\n            let Ok(re) = fancy_regex::Regex::new(&pat) else { return };",
        "            let pat = if case { spec_q.clone() } else { format!(\"(?i){spec_q}\") };\n            let Ok(re) = fancy_regex::Regex::new(&pat) else { return };")
# helper find_spec_for (used by replace_step where query text is a local)
s = rep(s, "    /// All matches in `text` (byte ranges)", "    fn find_spec_for(&self, q: &str) -> (String, bool, bool) {\n        let mut me = self.find_spec();\n        if !self.find.whole {\n            me.0 = q.to_string();\n        }\n        me\n    }\n\n    /// All matches in `text` (byte ranges)")
# find in files: whole word
s = rep(s, "        let (case, regex) = (self.find.case, self.find.regex);\n        let cancel = Arc::new(AtomicBool::new(false));",
        "        let (qspec, case, regex) = self.find_spec();\n        let cancel = Arc::new(AtomicBool::new(false));")
s = rep(s, "        let qq = q.clone();\n        std::thread::spawn(move || {\n            let mut files = Vec::new();", "        let qq = qspec;\n        std::thread::spawn(move || {\n            let mut files = Vec::new();")
wr(p, s)

# ---------------- ui.rs
p = 'src/app/ui.rs'
s = rd(p)
s = rep(s, "                let m = editor::find_matches(&d.text, &self.find.query, self.find.case, self.find.regex);\n                let (lo, hi)",
        "                let m = self.find_all(&d.text);\n                let (lo, hi)")
s = rep(s, "        let find_q = if self.find.open && !self.find.query.is_empty() && !self.fif.open { Some((self.find.query.as_str(), self.find.case, self.find.regex)) } else { None };",
        "        let spec = self.find_spec();\n        let find_q = if self.find.open && self.find.highlight && !self.find.query.is_empty() && !self.fif.open { Some((spec.0.as_str(), spec.1, spec.2)) } else { None };")
wr(p, s)

# ---------------- keys.rs: toggles and count
p = 'src/app/keys.rs'
s = rd(p)
s = rep(s, '            ui.toggle_value(&mut self.find.regex, ".*").on_hover_text("Regular expression");\n            ui.toggle_value(&mut self.find.case, "Aa").on_hover_text("Match case");',
        '''            ui.toggle_value(&mut self.find.regex, ".*").on_hover_text("Regular expression");
            ui.toggle_value(&mut self.find.case, "Aa").on_hover_text("Case sensitive");
            ui.toggle_value(&mut self.find.whole, "\\"ab\\"").on_hover_text("Whole word");
            ui.toggle_value(&mut self.find.wrap, "↺").on_hover_text("Wrap around");
            let mut in_sel = self.find.in_sel.is_some();
            if ui.toggle_value(&mut in_sel, "▭").on_hover_text("In selection").changed() {
                let (lo, hi) = self.sel();
                self.find.in_sel = if in_sel && lo != hi { Some((lo, hi)) } else { None };
            }
            ui.toggle_value(&mut self.find.highlight, "▤").on_hover_text("Highlight matches");''')
s = rep(s, "                        let n = editor::find_matches(&d.text, &self.find.query, self.find.case, self.find.regex).len();",
        "                        let n = self.find_all(&d.text).len();")
wr(p, s)
print('patched3')
