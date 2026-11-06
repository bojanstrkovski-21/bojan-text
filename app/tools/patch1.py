# One-off source patch: Enter auto-indent + the original's Tab rules.
import io

def rd(p):
    return io.open(p, encoding='utf-8').read()

def wr(p, s):
    io.open(p, 'w', encoding='utf-8', newline='\n').write(s)

# ---- textops
p = 'src/textops.rs'
s = rd(p)
NEWLINE = '''/// Enter key (Default (Windows).sublime-keymap): inserts "\\n" and, with `auto_indent`,
/// copies the current line's indent. When `{` is right before the cursor and `}` right
/// after it, the "Add Line in Braces" macro runs instead (blank indented line between).
/// `trim_automatic_white_space`: a line holding only auto-inserted whitespace is emptied.
pub fn newline(text: &str, lo: usize, hi: usize, unit: &str, auto_indent: bool) -> Out {
    let c: Vec<char> = text.chars().collect();
    let ls = line_start(&c, lo);
    let le = line_end(&c, lo);
    let ind: String = if auto_indent { c[ls..le].iter().take_while(|ch| **ch == ' ' || **ch == '\\t').take(lo - ls).collect() } else { String::new() };
    let only_ws_before = c[ls..lo].iter().all(|ch| *ch == ' ' || *ch == '\\t');
    let nothing_after = c[hi..line_end(&c, hi)].iter().all(|ch| ch.is_whitespace());
    let start = if lo == hi && only_ws_before && nothing_after && !ind.is_empty() { ls } else { lo };
    let in_braces = auto_indent && lo == hi && lo > 0 && c[lo - 1] == '{' && c.get(hi) == Some(&'}');
    let (ins, caret) = if in_braces {
        let first = format!("\\n{ind}{unit}");
        let n = first.chars().count();
        (format!("{first}\\n{ind}"), n)
    } else {
        let v = format!("\\n{ind}");
        let n = v.chars().count();
        (v, n)
    };
    let mut out: String = c[..start].iter().collect();
    out.push_str(&ins);
    out.extend(c[hi..].iter());
    let p = start + caret;
    Out { text: out, sel: (p, p) }
}

'''
marker = "/// Reindent: set each line's indent"
assert marker in s
s = s.replace(marker, NEWLINE + marker, 1)
TEST = '''    #[test]
    fn enter_rules() {
        assert_eq!(newline("\\tab", 3, 3, "\\t", true).text, "\\tab\\n\\t");
        let o = newline("{}", 1, 1, "\\t", true);
        assert_eq!(o.text, "{\\n\\t\\n}");
        assert_eq!(o.sel, (3, 3));
        assert_eq!(newline("  \\n", 2, 2, "\\t", true).text, "\\n  \\n");
    }
'''
m2 = "    #[test]\n    fn transpose_rules() {"
assert m2 in s
s = s.replace(m2, TEST + m2, 1)
wr(p, s)

# ---- keys.rs
p = 'src/app/keys.rs'
s = rd(p)
s = s.replace("        let mut tab_key: Option<bool> = None; // Some(shift)",
              "        let mut tab_key: Option<bool> = None; // Some(shift)\n        let mut enter_key = false;", 1)
s = s.replace("                if i.consume_shortcut(&sc(s, Key::Tab)) {\n                    tab_key = Some(true);",
              "                if i.consume_shortcut(&sc(n, Key::Enter)) || i.consume_shortcut(&sc(s, Key::Enter)) {\n                    enter_key = true;\n                }\n                if i.consume_shortcut(&sc(s, Key::Tab)) {\n                    tab_key = Some(true);", 1)
s = s.replace("        if let Some(shift) = tab_key {\n            self.tab_key(shift);",
              "        if enter_key {\n            let unit = if self.s.translate_tabs { \" \".repeat(self.s.tab_size.max(1)) } else { \"\\t\".to_string() };\n            self.edit_active(|t, lo, hi| Some(crate::textops::newline(t, lo, hi, &unit, true)));\n        }\n        if let Some(shift) = tab_key {\n            self.tab_key(shift);", 1)
a = s.index('    /// Tab / Shift+Tab: indent a multi-line selection')
b = s.index('    /// Find / Replace / Find in Files bar.')
NEW = r'''    /// Tab / Shift+Tab, following the original's key map (later rules win):
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

'''
s = s[:a] + NEW + s[b:]
wr(p, s)
print('patched')
