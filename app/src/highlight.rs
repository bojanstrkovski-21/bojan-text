//! Syntax highlighting (syntect) + loading languages and color schemes straight
//! from Sublime Text package folders (.sublime-package zips and loose folders).

use crate::colors::{eval_scheme_color, Rgba};
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use syntect::highlighting::{
    Color, FontStyle, HighlightIterator, HighlightState, Highlighter, ScopeSelectors, Style, StyleModifier, Theme,
    ThemeItem, ThemeSet, ThemeSettings,
};
use syntect::parsing::{ParseState, ScopeStack, SyntaxDefinition, SyntaxReference, SyntaxSet};

// ---------------------------------------------------------------------------
// Color schemes

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Scheme {
    pub name: String,
    pub source: String,
    pub theme: Theme,
}

pub fn strip_json_comments(src: &str) -> String {
    // Sublime's JSON allows // and /* */ comments and trailing commas.
    let mut out = String::with_capacity(src.len());
    let b: Vec<char> = src.chars().collect();
    let mut i = 0;
    let mut in_str = false;
    while i < b.len() {
        let c = b[i];
        if in_str {
            out.push(c);
            if c == '\\' && i + 1 < b.len() {
                out.push(b[i + 1]);
                i += 2;
                continue;
            }
            if c == '"' {
                in_str = false;
            }
            i += 1;
            continue;
        }
        if c == '"' {
            in_str = true;
            out.push(c);
            i += 1;
        } else if c == '/' && i + 1 < b.len() && b[i + 1] == '/' {
            while i < b.len() && b[i] != '\n' {
                i += 1;
            }
        } else if c == '/' && i + 1 < b.len() && b[i + 1] == '*' {
            i += 2;
            while i + 1 < b.len() && !(b[i] == '*' && b[i + 1] == '/') {
                i += 1;
            }
            i += 2;
        } else {
            out.push(c);
            i += 1;
        }
    }
    // trailing commas
    let chars: Vec<char> = out.chars().collect();
    let mut res = String::with_capacity(chars.len());
    let mut in_str = false;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if in_str {
            res.push(c);
            if c == '\\' && i + 1 < chars.len() {
                res.push(chars[i + 1]);
                i += 2;
                continue;
            }
            if c == '"' {
                in_str = false;
            }
        } else if c == '"' {
            in_str = true;
            res.push(c);
        } else if c == ',' {
            let mut j = i + 1;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            if j < chars.len() && (chars[j] == '}' || chars[j] == ']') {
                // drop it
            } else {
                res.push(c);
            }
        } else {
            res.push(c);
        }
        i += 1;
    }
    res
}

pub fn lenient_json(src: &str) -> Option<serde_json::Value> {
    serde_json::from_str(&strip_json_comments(src)).ok()
}

fn to_syn(c: Rgba) -> Color {
    let [r, g, b, a] = c.to_u8();
    Color { r, g, b, a }
}

/// Convert a `.sublime-color-scheme` file into a syntect theme.
pub fn load_sublime_scheme(text: &str, fallback_name: &str, source: &str) -> Option<Scheme> {
    let v = lenient_json(text)?;
    let mut vars: HashMap<String, String> = HashMap::new();
    if let Some(obj) = v.get("variables").and_then(|x| x.as_object()) {
        for (k, val) in obj {
            if let Some(s) = val.as_str() {
                vars.insert(k.clone(), s.to_string());
            }
        }
    }
    let col = |val: Option<&serde_json::Value>| -> Option<Color> {
        let s = val?.as_str()?;
        eval_scheme_color(s, &vars, 0).map(to_syn)
    };
    let g = v.get("globals");
    let gg = |k: &str| col(g.and_then(|g| g.get(k)));
    let settings = ThemeSettings {
        foreground: gg("foreground"),
        background: gg("background"),
        caret: gg("caret"),
        line_highlight: gg("line_highlight"),
        misspelling: gg("misspelling"),
        accent: gg("accent"),
        selection: gg("selection"),
        selection_border: gg("selection_border"),
        inactive_selection: gg("inactive_selection"),
        gutter: gg("gutter"),
        gutter_foreground: gg("gutter_foreground"),
        find_highlight: gg("find_highlight"),
        find_highlight_foreground: gg("find_highlight_foreground"),
        guide: gg("guide"),
        active_guide: gg("active_guide"),
        highlight: gg("highlight"),
        shadow: gg("shadow"),
        brackets_foreground: gg("brackets_foreground"),
        ..Default::default()
    };
    let mut scopes = Vec::new();
    if let Some(rules) = v.get("rules").and_then(|r| r.as_array()) {
        for r in rules {
            let Some(scope) = r.get("scope").and_then(|s| s.as_str()) else { continue };
            let Ok(sel) = ScopeSelectors::from_str(scope) else { continue };
            // foreground may be an array (hashed gradient) — take the first.
            let fgv = r.get("foreground").map(|f| if let Some(a) = f.as_array() { a.first().cloned().unwrap_or_default() } else { f.clone() });
            let foreground = col(fgv.as_ref());
            let background = col(r.get("background"));
            let font_style = r.get("font_style").and_then(|f| f.as_str()).map(|fs| {
                let mut s = FontStyle::empty();
                for w in fs.split_whitespace() {
                    match w {
                        "bold" => s |= FontStyle::BOLD,
                        "italic" => s |= FontStyle::ITALIC,
                        "underline" => s |= FontStyle::UNDERLINE,
                        _ => {}
                    }
                }
                s
            });
            scopes.push(ThemeItem { scope: sel, style: StyleModifier { foreground, background, font_style } });
        }
    }
    let name = v.get("name").and_then(|n| n.as_str()).unwrap_or(fallback_name).to_string();
    Some(Scheme {
        name: name.clone(),
        source: source.to_string(),
        theme: Theme { name: Some(name), author: None, settings, scopes },
    })
}

pub fn builtin_schemes() -> Vec<Scheme> {
    let mut out = Vec::new();
    for (name, src) in [
        ("Bojan Night", include_str!("../assets/schemes/Bojan Night.sublime-color-scheme")),
        ("Bojan Day", include_str!("../assets/schemes/Bojan Day.sublime-color-scheme")),
        ("Ember", include_str!("../assets/schemes/Ember.sublime-color-scheme")),
        ("Aurora", include_str!("../assets/schemes/Aurora.sublime-color-scheme")),
    ] {
        if let Some(s) = load_sublime_scheme(src, name, "Built in") {
            out.push(s);
        }
    }
    let ts = ThemeSet::load_defaults();
    let mut names: Vec<_> = ts.themes.keys().cloned().collect();
    names.sort();
    for n in names {
        out.push(Scheme { name: n.clone(), source: "Built in (classic)".into(), theme: ts.themes[&n].clone() });
    }
    out
}

// ---------------------------------------------------------------------------
// Sublime package discovery

#[derive(Clone, Default, Debug, serde::Serialize, serde::Deserialize)]
pub struct PackageInfo {
    pub name: String,
    pub location: String,
    pub syntaxes_loaded: Vec<String>,
    pub syntaxes_failed: Vec<(String, String)>,
    pub tm_languages: usize,
    pub schemes: Vec<String>,
    pub python_files: usize,
    pub snippets: usize,
    pub completions: usize,
    pub lsp_settings: bool,
}

#[derive(Clone, Default, Debug, serde::Serialize, serde::Deserialize)]
pub struct PackageReport {
    pub folders: Vec<(String, bool)>,
    pub packages: Vec<PackageInfo>,
    pub error: Option<String>,
}

impl PackageReport {
    pub fn total_loaded(&self) -> usize {
        self.packages.iter().map(|p| p.syntaxes_loaded.len()).sum()
    }
    pub fn total_failed(&self) -> usize {
        self.packages.iter().map(|p| p.syntaxes_failed.len()).sum()
    }
    pub fn total_python(&self) -> usize {
        self.packages.iter().map(|p| p.python_files).sum()
    }
}

pub struct ScanResult {
    pub set: SyntaxSet,
    pub schemes: Vec<Scheme>,
    pub report: PackageReport,
    pub extras: Extras,
}

/// Snippets and build systems read from Sublime packages.
#[derive(Clone, Default, Debug, serde::Serialize, serde::Deserialize)]
pub struct Extras {
    pub snippets: Vec<Snippet>,
    pub builds: Vec<BuildSystem>,
    /// (package, macro name) from *.sublime-macro files
    pub macros: Vec<(String, String)>,
}

#[derive(Clone, Default, Debug, serde::Serialize, serde::Deserialize)]
pub struct Snippet {
    pub description: String,
    pub trigger: String,
    pub scope: String,
    pub content: String,
    pub package: String,
}

#[derive(Clone, Default, Debug, serde::Serialize, serde::Deserialize)]
pub struct BuildSystem {
    pub name: String,
    pub cmd: Vec<String>,
    pub shell_cmd: Option<String>,
    pub selector: String,
    pub file_regex: String,
    pub working_dir: String,
    pub keyfiles: Vec<String>,
    /// (variant name, cmd, shell_cmd)
    pub variants: Vec<(String, Vec<String>, Option<String>)>,
    pub package: String,
}

fn xml_unescape(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

fn xml_tag(text: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let a = text.find(&open)? + open.len();
    let b = a + text[a..].find(&close)?;
    let inner = &text[a..b];
    Some(match (inner.find("<![CDATA["), inner.rfind("]]>")) {
        (Some(x), Some(y)) if y > x => inner[x + 9..y].to_string(),
        _ => xml_unescape(inner.trim()),
    })
}

pub fn parse_snippet(text: &str, package: &str, file: &str) -> Option<Snippet> {
    let content = xml_tag(text, "content")?;
    Some(Snippet {
        description: xml_tag(text, "description").unwrap_or_else(|| file.trim_end_matches(".sublime-snippet").to_string()),
        trigger: xml_tag(text, "tabTrigger").unwrap_or_default(),
        scope: xml_tag(text, "scope").unwrap_or_default(),
        content: content.trim_matches('\n').to_string(),
        package: package.to_string(),
    })
}

fn json_cmd(v: Option<&serde_json::Value>) -> Vec<String> {
    match v {
        Some(serde_json::Value::Array(a)) => a.iter().filter_map(|x| x.as_str().map(String::from)).collect(),
        Some(serde_json::Value::String(s)) => s.split_whitespace().map(String::from).collect(),
        _ => Vec::new(),
    }
}

pub fn parse_build(text: &str, package: &str, file: &str) -> Option<BuildSystem> {
    let mut v = lenient_json(text)?;
    // platform overrides
    if let Some(w) = v.get("windows").cloned() {
        if let (Some(obj), Some(wo)) = (v.as_object_mut(), w.as_object()) {
            for (k, val) in wo {
                obj.insert(k.clone(), val.clone());
            }
        }
    }
    let target = v.get("target").and_then(|t| t.as_str()).unwrap_or("exec");
    let cmd = json_cmd(v.get("cmd"));
    let shell_cmd = v.get("shell_cmd").and_then(|s| s.as_str()).map(String::from);
    if target != "exec" && cmd.is_empty() && shell_cmd.is_none() {
        return None;
    }
    let mut variants = Vec::new();
    if let Some(arr) = v.get("variants").and_then(|a| a.as_array()) {
        for var in arr {
            let name = var.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
            let vc = json_cmd(var.get("cmd"));
            let vs = var.get("shell_cmd").and_then(|s| s.as_str()).map(String::from);
            if !name.is_empty() && (!vc.is_empty() || vs.is_some()) {
                variants.push((name, vc, vs));
            }
        }
    }
    if cmd.is_empty() && shell_cmd.is_none() && variants.is_empty() {
        return None;
    }
    Some(BuildSystem {
        name: file.trim_end_matches(".sublime-build").to_string(),
        cmd,
        shell_cmd,
        selector: v.get("selector").and_then(|s| s.as_str()).unwrap_or("").to_string(),
        file_regex: v.get("file_regex").and_then(|s| s.as_str()).unwrap_or("").to_string(),
        working_dir: v.get("working_dir").and_then(|s| s.as_str()).unwrap_or("").to_string(),
        keyfiles: json_cmd(v.get("keyfiles")),
        variants,
        package: package.to_string(),
    })
}

/// Build systems available even without Sublime installed.
pub fn builtin_builds() -> Vec<BuildSystem> {
    let b = |name: &str, cmd: &[&str], sel: &str, regex: &str, keyfiles: &[&str], variants: &[(&str, &[&str])]| BuildSystem {
        name: name.into(),
        cmd: cmd.iter().map(|s| s.to_string()).collect(),
        shell_cmd: None,
        selector: sel.into(),
        file_regex: regex.into(),
        working_dir: if keyfiles.is_empty() { "$file_path".into() } else { "$folder".into() },
        keyfiles: keyfiles.iter().map(|s| s.to_string()).collect(),
        variants: variants.iter().map(|(n, c)| (n.to_string(), c.iter().map(|s| s.to_string()).collect(), None)).collect(),
        package: "Bojan Text".into(),
    };
    vec![
        b("Cargo", &["cargo", "build"], "source.rust", r"^ *--> (.+?):(\d+):(\d+)()", &["Cargo.toml"], &[("Run", &["cargo", "run"]), ("Test", &["cargo", "test"]), ("Check", &["cargo", "check"])]),
        b("Python", &["python", "-u", "$file"], "source.python", r#"^\s*File "(...*?)", line ([0-9]*)"#, &[], &[]),
        b("Node", &["node", "$file"], "source.js", r"^\s*at .*?\(?(.+?):(\d+):(\d+)\)?$", &[], &[]),
        b("Go", &["go", "run", "$file"], "source.go", r"^(.+?\.go):(\d+):(\d+):", &[], &[]),
    ]
}

/// Folders Sublime Text keeps packages in on this machine.
pub fn default_package_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    for pf in [std::env::var("ProgramFiles").ok(), Some("C:\\Program Files".into())].into_iter().flatten() {
        let p = Path::new(&pf).join("Sublime Text").join("Packages");
        if !dirs.contains(&p) {
            dirs.push(p);
        }
    }
    if let Ok(appdata) = std::env::var("APPDATA") {
        dirs.push(Path::new(&appdata).join("Sublime Text").join("Installed Packages"));
        dirs.push(Path::new(&appdata).join("Sublime Text").join("Packages"));
    }
    dirs
}

struct FileEntry {
    name: String,
    data: Vec<u8>,
}

fn collect_files(pkg_path: &Path) -> Vec<FileEntry> {
    let interesting = |n: &str| {
        let l = n.to_ascii_lowercase();
        l.ends_with(".sublime-syntax") || l.ends_with(".sublime-color-scheme") || l.ends_with(".tmtheme")
            || l.ends_with(".tmlanguage") || l.ends_with(".py") || l.ends_with(".sublime-snippet")
            || l.ends_with(".sublime-completions") || l.ends_with("lsp.sublime-settings") || l.ends_with(".sublime-build") || l.ends_with(".sublime-macro")
    };
    let mut out = Vec::new();
    if pkg_path.is_file() {
        let Ok(f) = std::fs::File::open(pkg_path) else { return out };
        let Ok(mut z) = zip::ZipArchive::new(f) else { return out };
        for i in 0..z.len() {
            let Ok(mut e) = z.by_index(i) else { continue };
            let Ok(name) = e.name().map(|n| n.to_string()) else { continue };
            if !interesting(&name) {
                continue;
            }
            let mut data = Vec::new();
            let big = name.ends_with(".py") || name.to_ascii_lowercase().ends_with(".tmlanguage");
            if !big && e.read_to_end(&mut data).is_err() {
                continue;
            }
            out.push(FileEntry { name, data });
        }
    } else if pkg_path.is_dir() {
        let mut stack = vec![pkg_path.to_path_buf()];
        while let Some(d) = stack.pop() {
            let Ok(rd) = std::fs::read_dir(&d) else { continue };
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    if stack.len() < 200 {
                        stack.push(p);
                    }
                } else {
                    let name = p.strip_prefix(pkg_path).unwrap_or(&p).to_string_lossy().replace('\\', "/");
                    if interesting(&name) {
                        let big = name.ends_with(".py") || name.to_ascii_lowercase().ends_with(".tmlanguage");
                        let data = if big { Vec::new() } else { std::fs::read(&p).unwrap_or_default() };
                        out.push(FileEntry { name, data });
                    }
                }
            }
        }
    }
    out
}

/// Same as [`scan_sublime_packages`], but reuses a saved result while the
/// package folders are unchanged (the full scan takes tens of seconds).
pub fn scan_sublime_packages_cached(dirs: &[PathBuf]) -> ScanResult {
    let mut fp = String::from("v5|");
    for d in dirs {
        fp.push_str(&d.display().to_string());
        if let Ok(rd) = std::fs::read_dir(d) {
            let mut v: Vec<String> = rd
                .flatten()
                .map(|e| {
                    let m = e.metadata().ok();
                    let len = m.as_ref().map(|m| m.len()).unwrap_or(0);
                    let t = m.and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
                    format!("{}:{len}:{t}", e.file_name().to_string_lossy())
                })
                .collect();
            v.sort();
            fp.push_str(&v.join(","));
        }
        fp.push('|');
    }
    let key = {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        fp.hash(&mut h);
        h.finish()
    };
    let dir = std::env::var("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(|_| std::env::temp_dir()).join("BojanText");
    let file = dir.join(format!("sublime-packages-{key:016x}.bin"));
    if let Ok(cached) = syntect::dumps::from_dump_file::<(SyntaxSet, Vec<Scheme>, PackageReport, Extras), _>(&file) {
        let (set, schemes, report, extras) = cached;
        return ScanResult { set, schemes, report, extras };
    }
    let r = scan_sublime_packages(dirs, true);
    if r.report.error.is_none() {
        let _ = std::fs::create_dir_all(&dir);
        // drop older caches
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for e in rd.flatten() {
                let n = e.file_name().to_string_lossy().to_string();
                if n.starts_with("sublime-packages-") && e.path() != file {
                    let _ = std::fs::remove_file(e.path());
                }
            }
        }
        let _ = syntect::dumps::dump_to_file(&(&r.set, &r.schemes, &r.report, &r.extras), &file);
    }
    r
}

/// Scan Sublime package folders and build a syntax set that combines the
/// editor's own languages with every Sublime syntax that loads.
pub fn scan_sublime_packages(dirs: &[PathBuf], include_sublime_syntaxes: bool) -> ScanResult {
    let mut report = PackageReport::default();
    let mut defs: Vec<SyntaxDefinition> = Vec::new();
    let mut schemes = Vec::new();
    let mut infos: Vec<PackageInfo> = Vec::new();
    let mut extras = Extras::default();
    // "Packages/<pkg>/<path>" -> (info index, short file name, text); later folders override earlier ones
    let mut syntax_files: HashMap<String, (usize, String, String)> = HashMap::new();
    let mut syntax_order: Vec<String> = Vec::new();
    for dir in dirs {
        let exists = dir.exists();
        report.folders.push((dir.display().to_string(), exists));
        if !exists {
            continue;
        }
        let Ok(rd) = std::fs::read_dir(dir) else { continue };
        let mut entries: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
        entries.sort();
        for p in entries {
            let is_pkg = p.extension().map(|e| e == "sublime-package").unwrap_or(false) || p.is_dir();
            if !is_pkg {
                continue;
            }
            let name = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            let mut info = PackageInfo { name: name.clone(), location: p.display().to_string(), ..Default::default() };
            for f in collect_files(&p) {
                let lower = f.name.to_ascii_lowercase();
                let short = f.name.rsplit('/').next().unwrap_or(&f.name).to_string();
                if lower.ends_with(".sublime-syntax") {
                    if !include_sublime_syntaxes {
                        continue;
                    }
                    let key = format!("Packages/{}/{}", name, f.name);
                    if !syntax_files.contains_key(&key) {
                        syntax_order.push(key.clone());
                    }
                    syntax_files.insert(key, (infos.len(), short, String::from_utf8_lossy(&f.data).to_string()));
                } else if lower.ends_with(".sublime-color-scheme") {
                    let text = String::from_utf8_lossy(&f.data);
                    let stem = short.trim_end_matches(".sublime-color-scheme");
                    if let Some(s) = load_sublime_scheme(&text, stem, &format!("Sublime: {name}")) {
                        info.schemes.push(s.name.clone());
                        schemes.push(s);
                    }
                } else if lower.ends_with(".tmtheme") {
                    let mut cur = std::io::Cursor::new(f.data.clone());
                    if let Ok(t) = ThemeSet::load_from_reader(&mut cur) {
                        let n = t.name.clone().unwrap_or_else(|| short.trim_end_matches(".tmTheme").to_string());
                        info.schemes.push(n.clone());
                        schemes.push(Scheme { name: n, source: format!("Sublime: {name}"), theme: t });
                    }
                } else if lower.ends_with(".tmlanguage") {
                    info.tm_languages += 1;
                } else if lower.ends_with(".py") {
                    info.python_files += 1;
                } else if lower.ends_with(".sublime-snippet") {
                    info.snippets += 1;
                    if let Some(sn) = parse_snippet(&String::from_utf8_lossy(&f.data), &name, &short) {
                        extras.snippets.push(sn);
                    }
                } else if lower.ends_with(".sublime-macro") {
                    extras.macros.push((name.clone(), short.trim_end_matches(".sublime-macro").to_string()));
                } else if lower.ends_with(".sublime-build") {
                    if let Some(b) = parse_build(&String::from_utf8_lossy(&f.data), &name, &short) {
                        extras.builds.push(b);
                    }
                } else if lower.ends_with(".sublime-completions") {
                    info.completions += 1;
                } else if lower.ends_with("lsp.sublime-settings") {
                    info.lsp_settings = true;
                }
            }
            infos.push(info);
        }
    }

    // Resolve Sublime Text 4 syntax inheritance ("extends") and load each syntax.
    let mut memo: HashMap<String, Option<yaml_rust::Yaml>> = HashMap::new();
    for key in &syntax_order {
        let (idx, short, _) = syntax_files[key].clone();
        let merged = resolve_syntax(key, &syntax_files, &mut memo, 0);
        let stem = short.trim_end_matches(".sublime-syntax").to_string();
        let res = match merged {
            Some(y) => {
                let mut out = String::new();
                let ok = yaml_rust::YamlEmitter::new(&mut out).dump(&y).is_ok();
                if ok {
                    match std::panic::catch_unwind(|| SyntaxDefinition::load_from_str(&out, true, Some(&stem))) {
                        Ok(Ok(d)) => Ok(d),
                        Ok(Err(e)) => Err(e.to_string()),
                        Err(_) => Err("crashed while reading".to_string()),
                    }
                } else {
                    Err("could not rebuild the file".to_string())
                }
            }
            None => Err("could not read the file or the syntax it extends".to_string()),
        };
        match res {
            Ok(d) => {
                infos[idx].syntaxes_loaded.push(d.name.clone());
                defs.push(d);
            }
            Err(e) => infos[idx].syntaxes_failed.push((short, e)),
        }
    }
    for info in infos {
        let useful = !info.syntaxes_loaded.is_empty() || !info.syntaxes_failed.is_empty() || !info.schemes.is_empty()
            || info.python_files > 0 || info.tm_languages > 0 || info.lsp_settings;
        if useful {
            report.packages.push(info);
        }
    }

    // Combine: editor defaults first (their internal links must keep their
    // positions), Sublime definitions after; lookups prefer the newest entry.
    let built = std::panic::catch_unwind(move || {
        let mut b = SyntaxSet::load_defaults_newlines().into_builder();
        for d in defs {
            b.add(d);
        }
        b.build()
    });
    let set = match built {
        Ok(s) => s,
        Err(_) => {
            report.error = Some("Combining Sublime syntaxes failed; using the built-in languages only.".into());
            SyntaxSet::load_defaults_newlines()
        }
    };
    ScanResult { set, schemes, report, extras }
}

// ---------------------------------------------------------------------------
// Symbols (Goto Symbol) and scope names

fn scope_strings(stack: &ScopeStack) -> Vec<String> {
    stack.as_slice().iter().map(|s| s.build_string()).collect()
}

/// Walk the document and call `f(line, byte_start, byte_end, scopes)` for every token.
fn walk_scopes(text: &str, syntax: &SyntaxReference, ss: &SyntaxSet, mut f: impl FnMut(usize, usize, usize, &ScopeStack) -> bool) {
    let mut ps = ParseState::new(syntax);
    let mut stack = ScopeStack::new();
    let mut line_byte = 0;
    for (ln, line) in text.split_inclusive('\n').enumerate() {
        let ops = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| ps.parse_line(line, ss))) {
            Ok(Ok(o)) => o,
            _ => return,
        };
        let mut pos = 0;
        for (at, op) in ops {
            if at > pos && !f(ln, line_byte + pos, line_byte + at, &stack) {
                return;
            }
            pos = pos.max(at);
            let _ = stack.apply(&op);
        }
        if line.len() > pos && !f(ln, line_byte + pos, line_byte + line.len(), &stack) {
            return;
        }
        line_byte += line.len();
    }
}

/// (name, byte offset) of functions, types, classes and headings.
pub fn symbols(text: &str, syntax: &SyntaxReference, ss: &SyntaxSet) -> Vec<(String, usize)> {
    let mut out: Vec<(String, usize)> = Vec::new();
    let mut last_end = usize::MAX;
    walk_scopes(text, syntax, ss, |_, a, b, stack| {
        let hit = scope_strings(stack).iter().any(|s| {
            (s.starts_with("entity.name.") && !s.starts_with("entity.name.tag") && !s.starts_with("entity.name.label")) || s.starts_with("meta.toc-list")
        });
        if hit {
            let piece = text[a..b].trim_end_matches('\n');
            if a == last_end {
                if let Some(l) = out.last_mut() {
                    l.0.push_str(piece);
                }
            } else if !piece.trim().is_empty() {
                out.push((piece.trim().to_string(), a));
            }
            last_end = b;
        }
        out.len() < 5000
    });
    out
}

/// Scope names at a byte offset (Tools ▸ Developer ▸ Show Scope Name).
pub fn scope_at(text: &str, syntax: &SyntaxReference, ss: &SyntaxSet, byte: usize) -> String {
    let mut found = String::new();
    walk_scopes(text, syntax, ss, |_, a, b, stack| {
        if byte >= a && byte < b.max(a + 1) {
            found = scope_strings(stack).join(" ");
            return false;
        }
        a <= byte
    });
    if found.is_empty() {
        syntax.scope.build_string()
    } else {
        found
    }
}

// ---------------------------------------------------------------------------
// Sublime Text 4 syntax inheritance. syntect reads ST3-style files only, so
// files that say `extends: Parent.sublime-syntax` are merged with their parent
// here first: variables are overridden key by key, contexts are replaced, or
// extended when they start with `meta_prepend: true` / `meta_append: true`.
// ST4 backtracking (`branch_point` / `branch` / `fail`) is approximated by
// taking the first branch and dropping `fail` rules.

use yaml_rust::Yaml;

fn ystr(s: &str) -> Yaml {
    Yaml::String(s.to_string())
}

fn resolve_syntax(key: &str, files: &HashMap<String, (usize, String, String)>, memo: &mut HashMap<String, Option<Yaml>>, depth: u32) -> Option<Yaml> {
    if let Some(m) = memo.get(key) {
        return m.clone();
    }
    if depth > 12 {
        return None;
    }
    let (_, _, text) = files.get(key)?;
    let doc = yaml_rust::YamlLoader::load_from_str(text).ok()?.into_iter().next()?;
    let Yaml::Hash(mut child) = doc else { return None };
    let parents: Vec<String> = match child.remove(&ystr("extends")) {
        Some(Yaml::String(s)) => vec![s],
        Some(Yaml::Array(a)) => a.into_iter().filter_map(|x| x.into_string()).collect(),
        _ => Vec::new(),
    };
    let dir = key.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_default();
    let mut result = if parents.is_empty() {
        Yaml::Hash(child)
    } else {
        let mut base: Option<yaml_rust::yaml::Hash> = None;
        for p in parents {
            let pkey = if p.starts_with("Packages/") { p.clone() } else { format!("{dir}/{p}") };
            let parent = resolve_syntax(&pkey, files, memo, depth + 1)?;
            let Yaml::Hash(ph) = parent else { return None };
            base = Some(match base {
                None => ph,
                Some(b) => merge_hash(b, ph),
            });
        }
        Yaml::Hash(merge_hash(base?, child))
    };
    fix_branches(&mut result);
    memo.insert(key.to_string(), Some(result.clone()));
    Some(result)
}

fn merge_hash(parent: yaml_rust::yaml::Hash, child: yaml_rust::yaml::Hash) -> yaml_rust::yaml::Hash {
    let mut out = parent;
    // these describe the file itself and are not inherited
    for k in ["file_extensions", "hidden_file_extensions", "first_line_match", "hidden"] {
        out.remove(&ystr(k));
    }
    for (k, v) in child {
        let ks = k.as_str().unwrap_or("").to_string();
        match (ks.as_str(), v) {
            ("variables", Yaml::Hash(cv)) => {
                let mut vars = match out.remove(&k) {
                    Some(Yaml::Hash(h)) => h,
                    _ => Default::default(),
                };
                for (vk, vv) in cv {
                    vars.insert(vk, vv);
                }
                out.insert(k, Yaml::Hash(vars));
            }
            ("contexts", Yaml::Hash(cc)) => {
                let mut ctxs = match out.remove(&k) {
                    Some(Yaml::Hash(h)) => h,
                    _ => Default::default(),
                };
                for (ck, cv) in cc {
                    let Yaml::Array(items) = cv else {
                        ctxs.insert(ck, cv);
                        continue;
                    };
                    let mut prepend = false;
                    let mut append = false;
                    let mut kept = Vec::new();
                    for it in items {
                        if let Yaml::Hash(mut h) = it {
                            if h.remove(&ystr("meta_prepend")).and_then(|v| v.as_bool()) == Some(true) {
                                prepend = true;
                            }
                            if h.remove(&ystr("meta_append")).and_then(|v| v.as_bool()) == Some(true) {
                                append = true;
                            }
                            if !h.is_empty() {
                                kept.push(Yaml::Hash(h));
                            }
                        } else {
                            kept.push(it);
                        }
                    }
                    let parent_items = match ctxs.get(&ck) {
                        Some(Yaml::Array(a)) => a.clone(),
                        _ => Vec::new(),
                    };
                    let merged = if prepend {
                        // meta_* keys of the parent stay first
                        let (meta, rest): (Vec<Yaml>, Vec<Yaml>) = parent_items.into_iter().partition(is_meta_item);
                        meta.into_iter().chain(kept).chain(rest).collect()
                    } else if append {
                        parent_items.into_iter().chain(kept).collect()
                    } else {
                        kept
                    };
                    ctxs.insert(ck, Yaml::Array(merged));
                }
                out.insert(k, Yaml::Hash(ctxs));
            }
            (_, v) => {
                out.insert(k, v);
            }
        }
    }
    out
}

fn is_meta_item(y: &Yaml) -> bool {
    match y {
        Yaml::Hash(h) => h.keys().all(|k| k.as_str().map(|s| s.starts_with("meta_") || s == "clear_scopes").unwrap_or(false)),
        _ => false,
    }
}

fn fix_branches(y: &mut Yaml) {
    let Yaml::Hash(h) = y else { return };
    let Some(Yaml::Hash(ctxs)) = h.get_mut(&ystr("contexts")) else { return };
    for (_, v) in ctxs.iter_mut() {
        let Yaml::Array(items) = v else { continue };
        items.retain(|it| match it {
            Yaml::Hash(m) => !m.contains_key(&ystr("fail")),
            _ => true,
        });
        for it in items.iter_mut() {
            if let Yaml::Hash(m) = it {
                m.remove(&ystr("branch_point"));
                if let Some(b) = m.remove(&ystr("branch")) {
                    let first = match b {
                        Yaml::Array(a) => a.into_iter().next(),
                        other => Some(other),
                    };
                    if let Some(f) = first {
                        m.insert(ystr("push"), f);
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Incremental per-line highlighting cache

pub type Span = (usize, usize, Style); // byte range inside the line

#[derive(Default)]
pub struct HlCache {
    key: String,
    hashes: Vec<u64>,
    states: Vec<(ParseState, HighlightState)>, // start state of each line, plus a final one
    pub spans: Vec<Vec<Span>>,
    pub line_starts: Vec<usize>,
    pub broken: bool,
}

fn hash_str(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

impl HlCache {
    pub fn invalidate(&mut self) {
        self.key.clear();
    }

    pub fn update(&mut self, text: &str, syntax: &SyntaxReference, ss: &SyntaxSet, theme: &Theme, key: &str) {
        let lines: Vec<&str> = text.split_inclusive('\n').collect();
        let hashes: Vec<u64> = lines.iter().map(|l| hash_str(l)).collect();
        let mut starts = Vec::with_capacity(lines.len());
        let mut acc = 0;
        for l in &lines {
            starts.push(acc);
            acc += l.len();
        }
        if self.key != key {
            self.key = key.to_string();
            self.hashes.clear();
            self.states.clear();
            self.spans.clear();
            self.broken = false;
        }
        if self.hashes == hashes && self.states.len() == lines.len() + 1 {
            self.line_starts = starts;
            return;
        }
        let highlighter = Highlighter::new(theme);
        let old_n = self.hashes.len();
        let new_n = hashes.len();
        let mut d = 0;
        while d < old_n && d < new_n && self.hashes[d] == hashes[d] && d + 1 < self.states.len() {
            d += 1;
        }
        let mut new_states: Vec<(ParseState, HighlightState)> = Vec::with_capacity(new_n + 1);
        let mut new_spans: Vec<Vec<Span>> = Vec::with_capacity(new_n);
        if d > 0 && self.states.len() > d {
            new_states.extend(self.states[..=d].iter().cloned());
            new_spans.extend(self.spans[..d].iter().cloned());
        } else {
            d = 0;
            new_states.push((ParseState::new(syntax), HighlightState::new(&highlighter, ScopeStack::new())));
        }
        let delta = new_n as isize - old_n as isize;
        let (mut ps, mut hs) = new_states[d].clone();
        let mut j = d;
        let broken = &mut self.broken;
        while j < new_n {
            let line = lines[j];
            let mut spans = Vec::new();
            if !*broken {
                let ops = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| ps.parse_line(line, ss)));
                match ops {
                    Ok(Ok(ops)) => {
                        let mut off = 0;
                        for (style, piece) in HighlightIterator::new(&mut hs, &ops, line, &highlighter) {
                            spans.push((off, off + piece.len(), style));
                            off += piece.len();
                        }
                    }
                    _ => *broken = true,
                }
            }
            new_spans.push(spans);
            new_states.push((ps.clone(), hs.clone()));
            j += 1;
            // try to resync with the old cache in the unchanged tail
            let oj = j as isize - delta;
            if j < new_n && oj > d as isize && (oj as usize) < old_n && self.hashes[oj as usize] == hashes[j] {
                let tail_equal = (j..new_n).all(|k| {
                    let ok = k as isize - delta;
                    ok >= 0 && (ok as usize) < old_n && self.hashes[ok as usize] == hashes[k]
                });
                if tail_equal && self.states.len() > oj as usize && self.states[oj as usize] == (ps.clone(), hs.clone()) {
                    let o = oj as usize;
                    new_spans.extend(self.spans[o..].iter().cloned());
                    new_states.extend(self.states[o + 1..].iter().cloned());
                    break;
                }
            }
        }
        self.hashes = hashes;
        self.states = new_states;
        self.spans = new_spans;
        self.line_starts = starts;
    }
}

/// Newest syntax with this name (Sublime's own versions are added after the built-in ones).
pub fn syntax_by_name<'a>(ss: &'a SyntaxSet, name: &str) -> Option<&'a SyntaxReference> {
    // prefer the visible language over hidden embedded copies with the same name
    ss.syntaxes().iter().rev().find(|s| s.name == name && !s.hidden).or_else(|| ss.syntaxes().iter().rev().find(|s| s.name == name))
}

pub fn find_syntax<'a>(ss: &'a SyntaxSet, path: Option<&Path>, first_line: &str) -> &'a SyntaxReference {
    if let Some(p) = path {
        if let Some(name) = p.file_name().map(|n| n.to_string_lossy().to_string()) {
            // full file name (e.g. "Makefile") then extension
            if let Some(s) = ss.find_syntax_by_extension(&name) {
                return s;
            }
            if let Some(ext) = p.extension() {
                if let Some(s) = ss.find_syntax_by_extension(&ext.to_string_lossy()) {
                    return s;
                }
            }
        }
    }
    ss.find_syntax_by_first_line(first_line).unwrap_or_else(|| ss.find_syntax_plain_text())
}

pub fn color_from_syn(c: Color) -> eframe::egui::Color32 {
    eframe::egui::Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_schemes_load() {
        let s = builtin_schemes();
        assert!(s.iter().any(|x| x.name == "Bojan Night"));
        assert!(s.iter().find(|x| x.name == "Bojan Night").unwrap().theme.settings.background.is_some());
    }

    /// Reads the real Sublime Text install on this PC (run with --ignored --nocapture).
    #[test]
    #[ignore]
    fn scan_real_sublime_packages() {
        let t = std::time::Instant::now();
        let r = scan_sublime_packages(&default_package_dirs(), true);
        println!("folders: {:?}", r.report.folders);
        println!("syntaxes loaded {} failed {} schemes {} python files {} in {:?}",
            r.report.total_loaded(), r.report.total_failed(), r.schemes.len(), r.report.total_python(), t.elapsed());
        for p in &r.report.packages {
            for (f, e) in &p.syntaxes_failed {
                println!("  FAIL {} / {}: {}", p.name, f, e.lines().next().unwrap_or(""));
            }
        }
        for p in &r.report.packages {
            if p.name == "JavaScript" {
                println!("  JS package loaded: {:?}", p.syntaxes_loaded);
            }
        }
        for s in r.set.syntaxes() {
            if s.name.contains("Script") || s.name.contains("TSX") {
                println!("  syntax {:?} exts {:?} hidden {}", s.name, s.file_extensions, s.hidden);
            }
        }
        // highlight some text with a couple of the loaded syntaxes
        let theme = &builtin_schemes()[0].theme;
        for (ext, sample) in [("rs", "fn main() { let x = 1; }\n"), ("py", "def f(x):\n    return x\n"), ("css", "a { color: red; }\n"), ("md", "# Hi\n*x*\n"), ("ts", "const a: number = 1;\n"), ("js", "const a = `x${1}`; function f(b) { return b => b * 2; }\n"), ("html", "<div class=\"a\"><script>let x = 1;</script></div>\n"), ("php", "<?php echo $x; ?>\n"), ("sql", "SELECT * FROM t WHERE a = 1;\n")] {
            let syn = find_syntax(&r.set, Some(Path::new(&format!("x.{ext}"))), "");
            let mut c = HlCache::default();
            let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| c.update(sample, syn, &r.set, theme, "k")));
            println!("  {ext}: syntax {:?} ok={} broken={} spans={}", syn.name, res.is_ok(), c.broken, c.spans.iter().map(|s| s.len()).sum::<usize>());
        }
    }
}
