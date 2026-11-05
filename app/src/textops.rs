//! Text editing operations behind the Edit / Selection / Goto menus.
//! Everything works on char indexes (the editor's cursor units).
//! A selection is (lo, hi) with lo <= hi; results carry the new text and selection.

pub struct Out {
    pub text: String,
    pub sel: (usize, usize),
}

fn s(c: &[char]) -> String {
    c.iter().collect()
}

pub fn line_start(c: &[char], i: usize) -> usize {
    let mut i = i.min(c.len());
    while i > 0 && c[i - 1] != '\n' {
        i -= 1;
    }
    i
}

pub fn line_end(c: &[char], i: usize) -> usize {
    let mut i = i.min(c.len());
    while i < c.len() && c[i] != '\n' {
        i += 1;
    }
    i
}

/// First line start and last line end covered by a selection (a selection that
/// ends at the very start of a line does not include that line).
pub fn line_span(c: &[char], lo: usize, hi: usize) -> (usize, usize) {
    let hi2 = if hi > lo && hi > 0 && c.get(hi - 1) == Some(&'\n') { hi - 1 } else { hi };
    (line_start(c, lo), line_end(c, hi2))
}

fn is_word(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

pub fn word_at(c: &[char], i: usize) -> (usize, usize) {
    let mut a = i.min(c.len());
    let mut b = a;
    if (a == c.len() || !is_word(c[a])) && a > 0 && is_word(c[a - 1]) {
        a -= 1;
        b = a;
    }
    if a < c.len() && !is_word(c[a]) {
        return (i, i);
    }
    while a > 0 && is_word(c[a - 1]) {
        a -= 1;
    }
    while b < c.len() && is_word(c[b]) {
        b += 1;
    }
    (a, b)
}

/// Replace chars [a, b) with `rep`.
fn splice(c: &[char], a: usize, b: usize, rep: &str) -> String {
    let mut out: String = c[..a].iter().collect();
    out.push_str(rep);
    out.extend(c[b..].iter());
    out
}

fn lines_of(c: &[char], a: usize, b: usize) -> Vec<String> {
    s(&c[a..b]).split('\n').map(String::from).collect()
}

/// Apply a transform to the full lines under the selection; the new block is selected
/// (or the cursor keeps its column when nothing was selected).
pub fn map_lines(text: &str, lo: usize, hi: usize, f: impl FnOnce(Vec<String>) -> Vec<String>) -> Out {
    let c: Vec<char> = text.chars().collect();
    let (a, b) = line_span(&c, lo, hi);
    let lines = lines_of(&c, a, b);
    let new = f(lines).join("\n");
    let n = new.chars().count();
    let out = splice(&c, a, b, &new);
    let sel = if lo == hi { (lo.min(a + n), lo.min(a + n)) } else { (a, a + n) };
    Out { text: out, sel }
}

pub fn indent(text: &str, lo: usize, hi: usize, unit: &str) -> Out {
    let c: Vec<char> = text.chars().collect();
    let (a, _) = line_span(&c, lo, hi);
    let ulen = unit.chars().count();
    let mut count_before_lo = 0;
    let r = map_lines(text, lo, hi, |ls| {
        ls.into_iter()
            .enumerate()
            .map(|(k, l)| {
                if k == 0 {
                    count_before_lo = 1;
                }
                if l.is_empty() && lo != hi { l } else { format!("{unit}{l}") }
            })
            .collect()
    });
    let _ = count_before_lo;
    if lo == hi {
        Out { sel: (lo + ulen, lo + ulen), ..r }
    } else {
        Out { sel: (a, r.sel.1), ..r }
    }
}

pub fn unindent(text: &str, lo: usize, hi: usize, tab: usize) -> Out {
    let c: Vec<char> = text.chars().collect();
    let ls = line_start(&c, lo);
    let mut removed_first = 0;
    let r = map_lines(text, lo, hi, |lines| {
        lines
            .into_iter()
            .enumerate()
            .map(|(k, l)| {
                let n = if l.starts_with('\t') {
                    1
                } else {
                    l.chars().take(tab).take_while(|ch| *ch == ' ').count()
                };
                if k == 0 {
                    removed_first = n;
                }
                l.chars().skip(n).collect()
            })
            .collect()
    });
    if lo == hi {
        let p = lo.saturating_sub(removed_first).max(ls);
        Out { sel: (p, p), ..r }
    } else {
        r
    }
}

pub fn swap_line(text: &str, lo: usize, hi: usize, up: bool) -> Option<Out> {
    let c: Vec<char> = text.chars().collect();
    let (a, b) = line_span(&c, lo, hi);
    if up {
        if a == 0 {
            return None;
        }
        let pa = line_start(&c, a - 1);
        let prev = s(&c[pa..a - 1]);
        let block = s(&c[a..b]);
        let rep = format!("{block}\n{prev}");
        let shift = a - pa;
        Some(Out { text: splice(&c, pa, b, &rep), sel: (lo - shift, hi - shift) })
    } else {
        if b >= c.len() {
            return None;
        }
        let ne = line_end(&c, b + 1);
        let next = s(&c[b + 1..ne]);
        let block = s(&c[a..b]);
        let rep = format!("{next}\n{block}");
        let shift = ne - b;
        Some(Out { text: splice(&c, a, ne, &rep), sel: (lo + shift, hi + shift) })
    }
}

pub fn duplicate(text: &str, lo: usize, hi: usize) -> Out {
    let c: Vec<char> = text.chars().collect();
    if lo != hi {
        let sel = s(&c[lo..hi]);
        let n = hi - lo;
        return Out { text: splice(&c, hi, hi, &sel), sel: (hi, hi + n) };
    }
    let (a, b) = line_span(&c, lo, hi);
    let line = s(&c[a..b]);
    let n = b - a + 1;
    Out { text: splice(&c, b, b, &format!("\n{line}")), sel: (lo + n, lo + n) }
}

pub fn delete_line(text: &str, lo: usize, hi: usize) -> Out {
    let c: Vec<char> = text.chars().collect();
    let (a, b) = line_span(&c, lo, hi);
    let (a2, b2) = if b < c.len() { (a, b + 1) } else if a > 0 { (a - 1, b) } else { (a, b) };
    let p = a2.min(c.len() - (b2 - a2));
    Out { text: splice(&c, a2, b2, ""), sel: (p, p) }
}

pub fn join_lines(text: &str, lo: usize, hi: usize) -> Out {
    let c: Vec<char> = text.chars().collect();
    let (a, mut b) = line_span(&c, lo, hi);
    if lo == hi || line_end(&c, lo) == b {
        // join with the next line
        if b < c.len() {
            b = line_end(&c, b + 1);
        }
    }
    let block = s(&c[a..b]);
    let mut parts = block.split('\n');
    let mut out = parts.next().unwrap_or("").trim_end().to_string();
    for p in parts {
        let t = p.trim();
        if !t.is_empty() {
            out.push(' ');
            out.push_str(t);
        }
    }
    let first_len = line_end(&c, a) - a;
    let p = a + first_len.min(out.chars().count());
    Out { text: splice(&c, a, b, &out), sel: (p, p) }
}

fn indent_of(line: &str) -> String {
    line.chars().take_while(|ch| *ch == ' ' || *ch == '\t').collect()
}

pub fn insert_line(text: &str, lo: usize, before: bool) -> Out {
    let c: Vec<char> = text.chars().collect();
    let a = line_start(&c, lo);
    let b = line_end(&c, lo);
    let ind = indent_of(&s(&c[a..b]));
    let n = ind.chars().count();
    if before {
        Out { text: splice(&c, a, a, &format!("{ind}\n")), sel: (a + n, a + n) }
    } else {
        Out { text: splice(&c, b, b, &format!("\n{ind}")), sel: (b + 1 + n, b + 1 + n) }
    }
}

pub fn delete_word(text: &str, lo: usize, forward: bool) -> Out {
    let c: Vec<char> = text.chars().collect();
    let (a, b) = if forward {
        let mut j = lo;
        while j < c.len() && c[j].is_whitespace() && c[j] != '\n' {
            j += 1;
        }
        if j < c.len() && is_word(c[j]) {
            while j < c.len() && is_word(c[j]) {
                j += 1;
            }
        } else if j < c.len() && j == lo {
            j += 1;
        }
        (lo, j)
    } else {
        let mut j = lo;
        while j > 0 && c[j - 1].is_whitespace() && c[j - 1] != '\n' {
            j -= 1;
        }
        if j > 0 && is_word(c[j - 1]) {
            while j > 0 && is_word(c[j - 1]) {
                j -= 1;
            }
        } else if j > 0 && j == lo {
            j -= 1;
        }
        (j, lo)
    };
    Out { text: splice(&c, a, b, ""), sel: (a, a) }
}

pub fn delete_to(text: &str, lo: usize, end: bool) -> (Out, String) {
    let c: Vec<char> = text.chars().collect();
    let (a, b) = if end {
        let e = line_end(&c, lo);
        (lo, if e == lo && e < c.len() { e + 1 } else { e })
    } else {
        (line_start(&c, lo), lo)
    };
    (Out { text: splice(&c, a, b, ""), sel: (a, a) }, s(&c[a..b]))
}

pub fn transpose(text: &str, lo: usize, hi: usize) -> Option<Out> {
    let c: Vec<char> = text.chars().collect();
    if lo != hi {
        return None;
    }
    // transpose.py: cursor at the end of a word -> swap that word with the next
    // word (or the previous one when there is no next word); otherwise swap chars.
    let (wa, wb) = word_at(&c, lo);
    if wa != wb && lo == wb {
        let next = (wb..c.len()).find(|&k| is_word(c[k]));
        let (first, second) = match next {
            Some(n) => ((wa, wb), word_at(&c, n)),
            None => {
                let prev_end = (0..wa).rev().find(|&k| is_word(c[k]))? + 1;
                (word_at(&c, prev_end - 1), (wa, wb))
            }
        };
        let (a1, b1) = first;
        let (a2, b2) = second;
        let w1: String = c[a1..b1].iter().collect();
        let w2: String = c[a2..b2].iter().collect();
        let mid: String = c[b1..a2].iter().collect();
        let rep = format!("{w2}{mid}{w1}");
        let end = a1 + rep.chars().count();
        return Some(Out { text: splice(&c, a1, b2, &rep), sel: (end, end) });
    }
    let i = if lo >= c.len() || c[lo] == '\n' { lo.checked_sub(1)? } else { lo };
    if i == 0 {
        return None;
    }
    let mut v = c.clone();
    v.swap(i - 1, i);
    Some(Out { text: s(&v), sel: (i + 1, i + 1) })
}

pub fn toggle_block_comment(text: &str, lo: usize, hi: usize, open: &str, close: &str) -> Out {
    let c: Vec<char> = text.chars().collect();
    let (a, b) = if lo == hi { line_span(&c, lo, hi) } else { (lo, hi) };
    let inner = s(&c[a..b]);
    let t = inner.trim();
    if t.starts_with(open.trim()) && t.ends_with(close.trim()) && t.len() >= open.trim().len() + close.trim().len() {
        let lead = inner.len() - inner.trim_start().len();
        let body = &t[open.trim().len()..t.len() - close.trim().len()];
        let body = body.strip_prefix(' ').unwrap_or(body);
        let body = body.strip_suffix(' ').unwrap_or(body);
        let rep = format!("{}{}", &inner[..lead], body);
        let n = rep.chars().count();
        Out { text: splice(&c, a, b, &rep), sel: (a, a + n) }
    } else {
        let rep = format!("{open}{inner}{close}");
        let n = rep.chars().count();
        Out { text: splice(&c, a, b, &rep), sel: (a, a + n) }
    }
}

// ---------------------------------------------------------------------------
// tags

const VOID: &[&str] = &["area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "source", "track", "wbr", "!doctype"];

struct TagTok {
    name: String,
    start: usize,
    end: usize, // after '>'
    closing: bool,
}

fn tags(c: &[char]) -> Vec<TagTok> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < c.len() {
        if c[i] == '<' && i + 1 < c.len() && (c[i + 1].is_alphabetic() || c[i + 1] == '/') {
            let closing = c[i + 1] == '/';
            let mut j = i + if closing { 2 } else { 1 };
            let ns = j;
            while j < c.len() && (c[j].is_alphanumeric() || c[j] == '-' || c[j] == ':' || c[j] == '!') {
                j += 1;
            }
            let name = s(&c[ns..j]).to_ascii_lowercase();
            let mut k = j;
            let mut quote: Option<char> = None;
            while k < c.len() {
                match (quote, c[k]) {
                    (None, '"') | (None, '\'') => quote = Some(c[k]),
                    (Some(q), ch) if ch == q => quote = None,
                    (None, '>') => break,
                    _ => {}
                }
                k += 1;
            }
            if k >= c.len() || name.is_empty() {
                break;
            }
            let self_closing = k > 0 && c[k - 1] == '/';
            if !self_closing && !VOID.contains(&name.as_str()) {
                out.push(TagTok { name, start: i, end: k + 1, closing });
            }
            i = k + 1;
        } else {
            i += 1;
        }
    }
    out
}

/// (open_start, open_end, close_start, close_end) pairs
fn tag_pairs(c: &[char]) -> Vec<(usize, usize, usize, usize, String)> {
    let mut stack: Vec<&TagTok> = Vec::new();
    let toks = tags(c);
    let mut pairs = Vec::new();
    for t in &toks {
        if !t.closing {
            stack.push(t);
        } else if let Some(pos) = stack.iter().rposition(|o| o.name == t.name) {
            let o = stack[pos];
            pairs.push((o.start, o.end, t.start, t.end, o.name.clone()));
            stack.truncate(pos);
        }
    }
    pairs
}

pub fn close_tag(text: &str, lo: usize) -> Option<Out> {
    let c: Vec<char> = text.chars().collect();
    let before = &c[..lo];
    let mut stack: Vec<String> = Vec::new();
    for t in tags(before) {
        if !t.closing {
            stack.push(t.name);
        } else if let Some(pos) = stack.iter().rposition(|n| *n == t.name) {
            stack.truncate(pos);
        }
    }
    let name = stack.pop()?;
    let rep = format!("</{name}>");
    let n = rep.chars().count();
    Some(Out { text: splice(&c, lo, lo, &rep), sel: (lo + n, lo + n) })
}

pub fn expand_to_tag(text: &str, lo: usize, hi: usize) -> Option<(usize, usize)> {
    let c: Vec<char> = text.chars().collect();
    let mut best: Option<(usize, usize)> = None;
    for (os, oe, cs, ce, _) in tag_pairs(&c) {
        // content first, then the whole element
        for cand in [(oe, cs), (os, ce)] {
            if cand.0 <= lo && cand.1 >= hi && (cand.1 - cand.0) > (hi - lo) {
                if best.map(|b| cand.1 - cand.0 < b.1 - b.0).unwrap_or(true) {
                    best = Some(cand);
                }
            }
        }
    }
    best
}

pub fn wrap_with_tag(text: &str, lo: usize, hi: usize, tag: &str) -> Out {
    let c: Vec<char> = text.chars().collect();
    let inner = s(&c[lo..hi]);
    let rep = format!("<{tag}>{inner}</{tag}>");
    Out { text: splice(&c, lo, hi, &rep), sel: (lo + 1, lo + 1 + tag.chars().count()) }
}

// ---------------------------------------------------------------------------
// case

fn split_words(t: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut cur = String::new();
    let chars: Vec<char> = t.chars().collect();
    for (i, &ch) in chars.iter().enumerate() {
        if ch == '_' || ch == '-' || ch.is_whitespace() {
            if !cur.is_empty() {
                words.push(std::mem::take(&mut cur));
            }
            continue;
        }
        let boundary = ch.is_uppercase()
            && !cur.is_empty()
            && (cur.chars().last().map(|p| p.is_lowercase() || p.is_ascii_digit()).unwrap_or(false)
                || chars.get(i + 1).map(|n| n.is_lowercase()).unwrap_or(false) && cur.chars().last().map(|p| p.is_uppercase()).unwrap_or(false));
        if boundary {
            words.push(std::mem::take(&mut cur));
        }
        cur.push(ch);
    }
    if !cur.is_empty() {
        words.push(cur);
    }
    words
}

fn cap(w: &str) -> String {
    let mut it = w.chars();
    match it.next() {
        Some(f) => f.to_uppercase().collect::<String>() + &it.as_str().to_lowercase(),
        None => String::new(),
    }
}

pub fn convert_case(t: &str, kind: &str) -> String {
    match kind {
        "upper" => t.to_uppercase(),
        "lower" => t.to_lowercase(),
        "swap" => t.chars().map(|ch| if ch.is_uppercase() { ch.to_lowercase().collect::<String>() } else { ch.to_uppercase().collect() }).collect(),
        "title" => {
            let mut out = String::new();
            let mut start = true;
            for ch in t.chars() {
                if start && ch.is_alphabetic() {
                    out.extend(ch.to_uppercase());
                    start = false;
                } else {
                    out.extend(ch.to_lowercase());
                }
                if ch.is_whitespace() {
                    start = true;
                }
            }
            out
        }
        "snake" => split_words(t).iter().map(|w| w.to_lowercase()).collect::<Vec<_>>().join("_"),
        "kebab" => split_words(t).iter().map(|w| w.to_lowercase()).collect::<Vec<_>>().join("-"),
        "camel" => {
            let w = split_words(t);
            w.iter().enumerate().map(|(i, x)| if i == 0 { x.to_lowercase() } else { cap(x) }).collect()
        }
        "pascal" => split_words(t).iter().map(|x| cap(x)).collect(),
        _ => t.to_string(),
    }
}

/// Convert the selection, or the word under the cursor.
pub fn case_op(text: &str, lo: usize, hi: usize, kind: &str) -> Option<Out> {
    let c: Vec<char> = text.chars().collect();
    let (a, b) = if lo == hi {
        if matches!(kind, "snake" | "kebab" | "camel" | "pascal") {
            // identifiers may contain '-'
            let (mut a, mut b) = word_at(&c, lo);
            while a > 0 && (is_word(c[a - 1]) || c[a - 1] == '-') {
                a -= 1;
            }
            while b < c.len() && (is_word(c[b]) || c[b] == '-') {
                b += 1;
            }
            (a, b)
        } else {
            word_at(&c, lo)
        }
    } else {
        (lo, hi)
    };
    if a == b {
        return None;
    }
    let rep = convert_case(&s(&c[a..b]), kind);
    let n = rep.chars().count();
    Some(Out { text: splice(&c, a, b, &rep), sel: (a, a + n) })
}

// ---------------------------------------------------------------------------
// wrap paragraph

pub fn paragraph_span(c: &[char], lo: usize, hi: usize) -> (usize, usize) {
    if lo != hi {
        return line_span(c, lo, hi);
    }
    let blank = |a: usize| s(&c[a..line_end(c, a)]).trim().is_empty();
    let mut a = line_start(c, lo);
    if blank(a) {
        return (a, line_end(c, a));
    }
    while a > 0 {
        let pa = line_start(c, a - 1);
        if blank(pa) {
            break;
        }
        a = pa;
    }
    let mut b = line_end(c, lo);
    while b < c.len() {
        let nb = line_end(c, b + 1);
        if s(&c[b + 1..nb]).trim().is_empty() {
            break;
        }
        b = nb;
    }
    (a, b)
}

pub fn wrap_paragraph(text: &str, lo: usize, hi: usize, width: usize) -> Out {
    let c: Vec<char> = text.chars().collect();
    let (a, b) = paragraph_span(&c, lo, hi);
    let block = s(&c[a..b]);
    let first = block.lines().next().unwrap_or("");
    let ind = indent_of(first);
    // keep a leading comment marker like "// " or "# "
    let rest = &first[ind.len()..];
    let prefix = ["// ", "# ", "-- ", "* ", "> "].iter().find(|p| rest.starts_with(**p)).map(|p| p.to_string()).unwrap_or_default();
    let lead = format!("{ind}{prefix}");
    let words: Vec<String> = block
        .lines()
        .map(|l| {
            let t = l.trim_start();
            let t = if !prefix.is_empty() { t.strip_prefix(prefix.trim_end()).unwrap_or(t) } else { t };
            t.to_string()
        })
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .map(String::from)
        .collect();
    let mut lines = Vec::new();
    let mut cur = String::new();
    for w in words {
        let cur_len = lead.chars().count() + cur.chars().count();
        if !cur.is_empty() && cur_len + 1 + w.chars().count() > width {
            lines.push(format!("{lead}{cur}"));
            cur.clear();
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(&w);
    }
    if !cur.is_empty() || lines.is_empty() {
        lines.push(format!("{lead}{cur}"));
    }
    let rep = lines.join("\n");
    let n = rep.chars().count();
    Out { text: splice(&c, a, b, &rep), sel: (a + n, a + n) }
}

// ---------------------------------------------------------------------------
// sorting / permuting whole lines (whole file when nothing is selected)

pub fn permute(text: &str, lo: usize, hi: usize, kind: &str, seed: u64) -> Out {
    let c: Vec<char> = text.chars().collect();
    let (a, b) = if lo == hi {
        let end = if c.last() == Some(&'\n') { c.len() - 1 } else { c.len() };
        (0, end)
    } else {
        line_span(&c, lo, hi)
    };
    let mut lines = lines_of(&c, a, b);
    match kind {
        "sort" => lines.sort_by_key(|l| l.to_lowercase()),
        "sort_case" => lines.sort(),
        "reverse" => lines.reverse(),
        "unique" => {
            let mut seen = std::collections::HashSet::new();
            lines.retain(|l| seen.insert(l.clone()));
        }
        "shuffle" => {
            let mut x = seed | 1;
            for i in (1..lines.len()).rev() {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                let j = (x % (i as u64 + 1)) as usize;
                lines.swap(i, j);
            }
        }
        _ => {}
    }
    let rep = lines.join("\n");
    let n = rep.chars().count();
    Out { text: splice(&c, a, b, &rep), sel: if lo == hi { (lo.min(c.len()), lo.min(c.len())) } else { (a, a + n) } }
}

// ---------------------------------------------------------------------------
// selection growth

pub fn expand_to_line(c: &[char], lo: usize, hi: usize) -> (usize, usize) {
    let (a, b) = line_span(c, lo, hi);
    let b = if b < c.len() { b + 1 } else { b };
    if (a, b) == (lo, hi) && b < c.len() {
        let nb = line_end(c, b);
        return (a, if nb < c.len() { nb + 1 } else { nb });
    }
    (a, b)
}

pub fn expand_to_line_up(c: &[char], lo: usize, hi: usize) -> (usize, usize) {
    let (a, b) = line_span(c, lo, hi);
    let b = if b < c.len() { b + 1 } else { b };
    if (a, b) == (lo, hi) && a > 0 {
        return (line_start(c, a - 1), b);
    }
    (a, b)
}

const PAIRS: &[(char, char)] = &[('(', ')'), ('[', ']'), ('{', '}')];

/// Innermost bracket pair around [lo, hi): returns (open index, close index).
pub fn enclosing_brackets(c: &[char], lo: usize, hi: usize) -> Option<(usize, usize)> {
    let mut depth: Vec<char> = Vec::new();
    let mut i = lo;
    while i > 0 {
        i -= 1;
        let ch = c[i];
        if let Some(&(o, _)) = PAIRS.iter().find(|p| p.1 == ch) {
            depth.push(o);
        } else if PAIRS.iter().any(|p| p.0 == ch) {
            if depth.last() == Some(&ch) {
                depth.pop();
            } else if depth.is_empty() {
                let close = PAIRS.iter().find(|p| p.0 == ch).unwrap().1;
                let mut d = 0;
                let mut j = hi;
                while j < c.len() {
                    if c[j] == ch {
                        d += 1;
                    } else if c[j] == close {
                        if d == 0 {
                            return Some((i, j));
                        }
                        d -= 1;
                    }
                    j += 1;
                }
                return None;
            }
        }
    }
    None
}

pub fn expand_to_brackets(c: &[char], lo: usize, hi: usize) -> Option<(usize, usize)> {
    let (o, cl) = enclosing_brackets(c, lo, hi)?;
    if (o + 1, cl) == (lo, hi) {
        return Some((o, cl + 1));
    }
    Some((o + 1, cl))
}

fn enclosing_quotes(c: &[char], lo: usize, hi: usize) -> Option<(usize, usize)> {
    let a = line_start(c, lo);
    let b = line_end(c, hi);
    for q in ['"', '\'', '`'] {
        let idx: Vec<usize> = (a..b).filter(|&k| c[k] == q && (k == 0 || c[k - 1] != '\\')).collect();
        for pair in idx.chunks(2) {
            if pair.len() == 2 && pair[0] < lo && pair[1] >= hi {
                let inner = (pair[0] + 1, pair[1]);
                if inner == (lo, hi) {
                    return Some((pair[0], pair[1] + 1));
                }
                if inner.0 <= lo && inner.1 >= hi {
                    return Some(inner);
                }
            }
        }
    }
    None
}

pub fn expand_to_scope(c: &[char], lo: usize, hi: usize) -> (usize, usize) {
    let q = enclosing_quotes(c, lo, hi);
    let b = expand_to_brackets(c, lo, hi);
    match (q, b) {
        (Some(q), Some(b)) => if q.1 - q.0 <= b.1 - b.0 { q } else { b },
        (Some(q), None) => q,
        (None, Some(b)) => b,
        (None, None) => expand_to_line(c, lo, hi),
    }
}

pub fn expand_smart(c: &[char], lo: usize, hi: usize) -> (usize, usize) {
    if lo == hi {
        let w = word_at(c, lo);
        if w.0 != w.1 {
            return w;
        }
    }
    let w = word_at(c, lo);
    if lo != hi && w != (lo, hi) && w.0 <= lo && w.1 >= hi && w.0 != w.1 {
        return w;
    }
    expand_to_scope(c, lo, hi)
}

pub fn expand_to_indentation(c: &[char], lo: usize, hi: usize) -> (usize, usize) {
    let lw = |a: usize| -> Option<usize> {
        let l = s(&c[a..line_end(c, a)]);
        if l.trim().is_empty() { None } else { Some(indent_of(&l).chars().count()) }
    };
    let a0 = line_start(c, lo);
    let Some(level) = lw(a0) else { return expand_to_line(c, lo, hi) };
    let mut a = a0;
    while a > 0 {
        let pa = line_start(c, a - 1);
        match lw(pa) {
            Some(l) if l >= level => a = pa,
            None => a = pa,
            _ => break,
        }
    }
    while a < lo && lw(a).is_none() {
        a = line_end(c, a) + 1;
    }
    let mut b = line_end(c, hi);
    while b < c.len() {
        let nb = line_end(c, b + 1);
        match lw(b + 1) {
            Some(l) if l >= level => b = nb,
            None => b = nb,
            _ => break,
        }
    }
    while b > hi && s(&c[line_start(c, b)..b]).trim().is_empty() {
        b = line_start(c, b).saturating_sub(1);
    }
    (a, b)
}

pub fn matching_bracket(c: &[char], pos: usize) -> Option<usize> {
    let try_at = |i: usize| -> Option<usize> {
        let ch = *c.get(i)?;
        if let Some(&(o, cl)) = PAIRS.iter().find(|p| p.0 == ch) {
            let mut d = 0;
            for (j, &x) in c.iter().enumerate().skip(i + 1) {
                if x == o {
                    d += 1;
                } else if x == cl {
                    if d == 0 {
                        return Some(j);
                    }
                    d -= 1;
                }
            }
        } else if let Some(&(o, cl)) = PAIRS.iter().find(|p| p.1 == ch) {
            let mut d = 0;
            for j in (0..i).rev() {
                if c[j] == cl {
                    d += 1;
                } else if c[j] == o {
                    if d == 0 {
                        return Some(j);
                    }
                    d -= 1;
                }
            }
        }
        None
    };
    try_at(pos).or_else(|| pos.checked_sub(1).and_then(try_at)).or_else(|| enclosing_brackets(c, pos, pos).map(|(_, e)| e))
}

/// Snippet body: `${1:default}` -> default, `$1`/`$0` -> "", `\$` -> `$`.
/// Returns the text and the char offset of the first field (or the end).
pub fn expand_snippet(body: &str) -> (String, usize) {
    let chars: Vec<char> = body.chars().collect();
    let mut out = String::new();
    let mut first: Option<(u32, usize)> = None;
    let mut i = 0;
    let mut note = |num: u32, at: usize, first: &mut Option<(u32, usize)>| {
        let num = if num == 0 { u32::MAX } else { num };
        if first.map(|f| num < f.0).unwrap_or(true) {
            *first = Some((num, at));
        }
    };
    while i < chars.len() {
        let ch = chars[i];
        if ch == '\\' && i + 1 < chars.len() && chars[i + 1] == '$' {
            out.push('$');
            i += 2;
        } else if ch == '$' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit() {
            let mut j = i + 1;
            let mut n = 0u32;
            while j < chars.len() && chars[j].is_ascii_digit() {
                n = n * 10 + chars[j].to_digit(10).unwrap();
                j += 1;
            }
            note(n, out.chars().count(), &mut first);
            i = j;
        } else if ch == '$' && i + 1 < chars.len() && chars[i + 1] == '{' {
            let mut j = i + 2;
            let mut n = 0u32;
            while j < chars.len() && chars[j].is_ascii_digit() {
                n = n * 10 + chars[j].to_digit(10).unwrap();
                j += 1;
            }
            note(n, out.chars().count(), &mut first);
            if j < chars.len() && chars[j] == ':' {
                j += 1;
            }
            let mut depth = 1;
            let mut inner = String::new();
            while j < chars.len() {
                if chars[j] == '{' {
                    depth += 1;
                } else if chars[j] == '}' {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                inner.push(chars[j]);
                j += 1;
            }
            let (expanded, _) = expand_snippet(&inner);
            out.push_str(&expanded);
            i = j + 1;
        } else {
            out.push(ch);
            i += 1;
        }
    }
    let end = out.chars().count();
    (out, first.map(|f| f.1).unwrap_or(end))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cases() {
        assert_eq!(convert_case("helloWorld", "snake"), "hello_world");
        assert_eq!(convert_case("hello_world", "camel"), "helloWorld");
        assert_eq!(convert_case("hello-world", "pascal"), "HelloWorld");
        assert_eq!(convert_case("HTTPServer", "kebab"), "http-server");
    }
    #[test]
    fn lines() {
        let o = swap_line("a\nb\nc", 2, 2, true).unwrap();
        assert_eq!(o.text, "b\na\nc");
        let o = join_lines("a\n   b\nc", 0, 0);
        assert_eq!(o.text, "a b\nc");
        let o = permute("b\na\nc\n", 0, 0, "sort", 1);
        assert_eq!(o.text, "a\nb\nc\n");
        let o = duplicate("x\ny", 0, 0);
        assert_eq!(o.text, "x\nx\ny");
    }
    #[test]
    fn tags_and_brackets() {
        let t = "<div><p>hi</p>";
        let o = close_tag(t, t.chars().count()).unwrap();
        assert!(o.text.ends_with("</div>"));
        let c: Vec<char> = "f(a, [b])".chars().collect();
        assert_eq!(expand_to_brackets(&c, 6, 6), Some((6, 7)));
        assert_eq!(matching_bracket(&c, 1), Some(8));
        let (s, at) = expand_snippet("for ${1:i} in $2 {\n\t$0\n}");
        assert_eq!(s, "for i in  {\n\t\n}");
        assert_eq!(at, 4);
    }
    #[test]
    fn transpose_rules() {
        // inside a word: characters swap
        assert_eq!(transpose("abc", 1, 1).unwrap().text, "bac");
        // at the end of a word: words swap
        assert_eq!(transpose("one two", 3, 3).unwrap().text, "two one");
    }
    #[test]
    fn wrap() {
        let o = wrap_paragraph("one two three four five", 0, 0, 10);
        assert_eq!(o.text, "one two\nthree four\nfive");
    }
}
