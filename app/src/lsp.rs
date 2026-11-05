//! A small Language Server Protocol client: diagnostics, hover and go-to-definition.
//! Server list = well-known servers found on PATH + clients configured for the
//! Sublime "LSP" package (Packages/User/LSP.sublime-settings).

use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
pub struct ServerSpec {
    pub name: String,
    pub command: Vec<String>,
    pub exts: Vec<String>,
    pub source: String,
    pub found: Option<PathBuf>,
}

pub fn language_id(ext: &str) -> &'static str {
    match ext.to_ascii_lowercase().as_str() {
        "rs" => "rust",
        "py" | "pyw" => "python",
        "js" | "mjs" | "cjs" => "javascript",
        "jsx" => "javascriptreact",
        "ts" | "mts" | "cts" => "typescript",
        "tsx" => "typescriptreact",
        "c" => "c",
        "cpp" | "cc" | "cxx" | "hpp" | "hh" | "h" => "cpp",
        "go" => "go",
        "css" => "css",
        "scss" => "scss",
        "less" => "less",
        "html" | "htm" => "html",
        "json" | "jsonc" => "json",
        "md" | "markdown" => "markdown",
        "lua" => "lua",
        "java" => "java",
        "cs" => "csharp",
        "sh" | "bash" => "shellscript",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "php" => "php",
        "rb" => "ruby",
        "zig" => "zig",
        _ => "plaintext",
    }
}

fn exts_for_selector(sel: &str) -> Vec<String> {
    // Sublime LSP uses scope selectors like "source.python | source.js"
    let map: &[(&str, &[&str])] = &[
        ("source.python", &["py"]), ("source.rust", &["rs"]), ("source.js", &["js", "mjs", "cjs", "jsx"]),
        ("source.ts", &["ts", "mts"]), ("source.tsx", &["tsx"]), ("source.c++", &["cpp", "cc", "cxx", "hpp", "h"]),
        ("source.c", &["c", "h"]), ("source.go", &["go"]), ("source.css", &["css"]), ("source.scss", &["scss"]),
        ("text.html", &["html", "htm"]), ("source.json", &["json"]), ("text.html.markdown", &["md"]),
        ("source.lua", &["lua"]), ("source.java", &["java"]), ("source.cs", &["cs"]), ("source.shell", &["sh"]),
        ("source.toml", &["toml"]), ("source.yaml", &["yaml", "yml"]), ("source.php", &["php"]), ("source.ruby", &["rb"]),
    ];
    let mut out = Vec::new();
    for (scope, exts) in map {
        if sel.contains(scope) {
            out.extend(exts.iter().map(|e| e.to_string()));
        }
    }
    out
}

pub fn which(cmd: &str) -> Option<PathBuf> {
    let p = Path::new(cmd);
    if p.is_absolute() {
        return p.exists().then(|| p.to_path_buf());
    }
    let path = std::env::var_os("PATH")?;
    let exts: Vec<String> = if cfg!(windows) {
        vec!["".into(), ".exe".into(), ".cmd".into(), ".bat".into()]
    } else {
        vec!["".into()]
    };
    for dir in std::env::split_paths(&path) {
        for e in &exts {
            let cand = dir.join(format!("{cmd}{e}"));
            if cand.is_file() {
                return Some(cand);
            }
        }
    }
    None
}

pub fn discover_servers() -> Vec<ServerSpec> {
    let mut v: Vec<ServerSpec> = Vec::new();
    let known: &[(&str, &[&str], &[&str])] = &[
        ("rust-analyzer", &["rust-analyzer"], &["rs"]),
        ("pyright", &["pyright-langserver", "--stdio"], &["py", "pyw"]),
        ("pylsp", &["pylsp"], &["py", "pyw"]),
        ("typescript-language-server", &["typescript-language-server", "--stdio"], &["js", "jsx", "ts", "tsx", "mjs", "cjs"]),
        ("clangd", &["clangd"], &["c", "cpp", "cc", "cxx", "h", "hpp"]),
        ("gopls", &["gopls"], &["go"]),
        ("css-languageserver", &["vscode-css-language-server", "--stdio"], &["css", "scss", "less"]),
        ("html-languageserver", &["vscode-html-language-server", "--stdio"], &["html", "htm"]),
        ("json-languageserver", &["vscode-json-language-server", "--stdio"], &["json", "jsonc"]),
        ("marksman", &["marksman", "server"], &["md", "markdown"]),
        ("lua-language-server", &["lua-language-server"], &["lua"]),
        ("taplo", &["taplo", "lsp", "stdio"], &["toml"]),
        ("yaml-language-server", &["yaml-language-server", "--stdio"], &["yaml", "yml"]),
    ];
    // Sublime LSP package client settings come first so they win.
    if let Ok(appdata) = std::env::var("APPDATA") {
        let f = Path::new(&appdata).join("Sublime Text").join("Packages").join("User").join("LSP.sublime-settings");
        if let Ok(text) = std::fs::read_to_string(&f) {
            if let Some(val) = crate::highlight::lenient_json(&text) {
                if let Some(clients) = val.get("clients").and_then(|c| c.as_object()) {
                    for (name, c) in clients {
                        if c.get("enabled").and_then(|e| e.as_bool()) == Some(false) {
                            continue;
                        }
                        let command: Vec<String> = c.get("command").and_then(|x| x.as_array())
                            .map(|a| a.iter().filter_map(|s| s.as_str().map(String::from)).collect()).unwrap_or_default();
                        if command.is_empty() {
                            continue;
                        }
                        let sel = c.get("selector").and_then(|s| s.as_str()).unwrap_or("");
                        let exts = exts_for_selector(sel);
                        let found = which(&command[0]);
                        v.push(ServerSpec { name: name.clone(), command, exts, source: "Sublime LSP settings".into(), found });
                    }
                }
            }
        }
    }
    for (name, cmd, exts) in known {
        if v.iter().any(|s| s.name == *name) {
            continue;
        }
        v.push(ServerSpec {
            name: name.to_string(),
            command: cmd.iter().map(|s| s.to_string()).collect(),
            exts: exts.iter().map(|s| s.to_string()).collect(),
            source: "Built-in list".into(),
            found: which(cmd[0]),
        });
    }
    v
}

pub fn path_to_uri(p: &Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/");
    let mut out = String::from("file:///");
    for ch in s.trim_start_matches('/').chars() {
        match ch {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '/' | '-' | '_' | '.' | '~' | ':' => out.push(ch),
            c => {
                let mut b = [0u8; 4];
                for x in c.encode_utf8(&mut b).bytes() {
                    out.push_str(&format!("%{x:02X}"));
                }
            }
        }
    }
    out
}

pub fn uri_to_path(uri: &str) -> PathBuf {
    let s = uri.strip_prefix("file:///").or_else(|| uri.strip_prefix("file://")).unwrap_or(uri);
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    let s = String::from_utf8_lossy(&out).to_string();
    if cfg!(windows) { PathBuf::from(s.replace('/', "\\")) } else { PathBuf::from(format!("/{s}")) }
}

pub fn norm_key(p: &Path) -> String {
    p.to_string_lossy().replace('/', "\\").to_ascii_lowercase()
}

#[derive(Clone, Debug)]
pub struct Diag {
    pub line: usize,
    pub col: usize, // utf16 units
    pub end_line: usize,
    pub end_col: usize,
    pub severity: u8,
    pub message: String,
    pub source: String,
}

pub enum LspEvent {
    Diagnostics(String, Vec<Diag>),
    Hover(String),
    Definition(PathBuf, usize, usize),
    References(Vec<(PathBuf, usize, usize)>),
    Status(String),
}

enum Pending {
    Init,
    Hover,
    Definition,
    References,
    Other,
}

pub struct LspClient {
    pub name: String,
    pub status: String,
    stdin: Arc<Mutex<ChildStdin>>,
    child: Child,
    rx: Receiver<Value>,
    next_id: i64,
    pending: HashMap<i64, Pending>,
    initialized: bool,
    queued: Vec<Value>,
    pub opened: HashSet<String>,
    pub versions: HashMap<String, (i32, u64)>,
    pub log: Vec<String>,
}

fn write_msg(stdin: &Arc<Mutex<ChildStdin>>, v: &Value) {
    let body = v.to_string();
    let mut s = stdin.lock().unwrap();
    let _ = write!(s, "Content-Length: {}\r\n\r\n{}", body.len(), body);
    let _ = s.flush();
}

impl LspClient {
    pub fn start(spec: &ServerSpec, root: Option<&Path>) -> Result<Self, String> {
        let exe = spec.found.clone().ok_or_else(|| format!("{} is not installed (not on PATH)", spec.command[0]))?;
        let mut cmd = Command::new(&exe);
        cmd.args(&spec.command[1..]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
        if let Some(r) = root {
            cmd.current_dir(r);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000);
        }
        let mut child = cmd.spawn().map_err(|e| format!("could not start {}: {e}", spec.name))?;
        let stdin = Arc::new(Mutex::new(child.stdin.take().unwrap()));
        let stdout = child.stdout.take().unwrap();
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let mut r = BufReader::new(stdout);
            loop {
                let mut len = 0usize;
                loop {
                    let mut line = String::new();
                    match r.read_line(&mut line) {
                        Ok(0) | Err(_) => return,
                        _ => {}
                    }
                    let l = line.trim();
                    if l.is_empty() {
                        break;
                    }
                    if let Some(v) = l.strip_prefix("Content-Length:") {
                        len = v.trim().parse().unwrap_or(0);
                    }
                }
                let mut buf = vec![0u8; len];
                if r.read_exact(&mut buf).is_err() {
                    return;
                }
                if let Ok(v) = serde_json::from_slice::<Value>(&buf) {
                    if tx.send(v).is_err() {
                        return;
                    }
                }
            }
        });
        let mut c = Self {
            name: spec.name.clone(),
            status: "starting".into(),
            stdin,
            child,
            rx,
            next_id: 1,
            pending: HashMap::new(),
            initialized: false,
            queued: Vec::new(),
            opened: HashSet::new(),
            versions: HashMap::new(),
            log: Vec::new(),
        };
        let root_uri = root.map(path_to_uri);
        let params = json!({
            "processId": std::process::id(),
            "rootUri": root_uri,
            "workspaceFolders": root.map(|r| json!([{"uri": path_to_uri(r), "name": r.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()}])),
            "clientInfo": {"name": "Bojan Text", "version": env!("CARGO_PKG_VERSION")},
            "capabilities": {
                "textDocument": {
                    "synchronization": {"didSave": true},
                    "hover": {"contentFormat": ["markdown", "plaintext"]},
                    "definition": {},
                    "publishDiagnostics": {"relatedInformation": false}
                },
                "workspace": {"workspaceFolders": true, "configuration": true}
            }
        });
        let id = c.next_id;
        c.next_id += 1;
        c.pending.insert(id, Pending::Init);
        write_msg(&c.stdin, &json!({"jsonrpc": "2.0", "id": id, "method": "initialize", "params": params}));
        Ok(c)
    }

    fn send(&mut self, v: Value) {
        if self.initialized {
            write_msg(&self.stdin, &v);
        } else {
            self.queued.push(v);
        }
    }

    fn notify(&mut self, method: &str, params: Value) {
        self.send(json!({"jsonrpc": "2.0", "method": method, "params": params}));
    }

    fn request(&mut self, method: &str, params: Value, kind: Pending) {
        let id = self.next_id;
        self.next_id += 1;
        self.pending.insert(id, kind);
        self.send(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
    }

    /// Open or update a document (full-text sync).
    pub fn sync(&mut self, path: &Path, text: &str, hash: u64) {
        let uri = path_to_uri(path);
        let key = norm_key(path);
        if !self.opened.contains(&key) {
            let ext = path.extension().map(|e| e.to_string_lossy().to_string()).unwrap_or_default();
            self.notify("textDocument/didOpen", json!({"textDocument": {"uri": uri, "languageId": language_id(&ext), "version": 1, "text": text}}));
            self.opened.insert(key.clone());
            self.versions.insert(key, (1, hash));
            return;
        }
        let (ver, h) = self.versions.get(&key).cloned().unwrap_or((1, 0));
        if h != hash {
            let nv = ver + 1;
            self.notify("textDocument/didChange", json!({"textDocument": {"uri": uri, "version": nv}, "contentChanges": [{"text": text}]}));
            self.versions.insert(key, (nv, hash));
        }
    }

    pub fn did_save(&mut self, path: &Path) {
        self.notify("textDocument/didSave", json!({"textDocument": {"uri": path_to_uri(path)}}));
    }

    pub fn hover(&mut self, path: &Path, line: usize, col16: usize) {
        self.request("textDocument/hover", json!({"textDocument": {"uri": path_to_uri(path)}, "position": {"line": line, "character": col16}}), Pending::Hover);
    }

    pub fn references(&mut self, path: &Path, line: usize, col16: usize) {
        self.request(
            "textDocument/references",
            json!({"textDocument": {"uri": path_to_uri(path)}, "position": {"line": line, "character": col16}, "context": {"includeDeclaration": true}}),
            Pending::References,
        );
    }

    pub fn definition(&mut self, path: &Path, line: usize, col16: usize) {
        self.request("textDocument/definition", json!({"textDocument": {"uri": path_to_uri(path)}, "position": {"line": line, "character": col16}}), Pending::Definition);
    }

    pub fn poll(&mut self) -> Vec<LspEvent> {
        let mut out = Vec::new();
        while let Ok(msg) = self.rx.try_recv() {
            let id = msg.get("id").and_then(|i| i.as_i64());
            let method = msg.get("method").and_then(|m| m.as_str()).map(String::from);
            match (id, method) {
                (Some(id), Some(m)) => {
                    // server -> client request; answer politely
                    let result = match m.as_str() {
                        "workspace/configuration" => {
                            let n = msg.pointer("/params/items").and_then(|i| i.as_array()).map(|a| a.len()).unwrap_or(1);
                            Value::Array(vec![Value::Null; n])
                        }
                        _ => Value::Null,
                    };
                    write_msg(&self.stdin, &json!({"jsonrpc": "2.0", "id": id, "result": result}));
                }
                (Some(id), None) => {
                    let kind = self.pending.remove(&id).unwrap_or(Pending::Other);
                    if let Some(err) = msg.get("error") {
                        self.log.push(format!("error: {}", err));
                        if matches!(kind, Pending::Init) {
                            self.status = "failed to start".into();
                        }
                        continue;
                    }
                    let res = msg.get("result").cloned().unwrap_or(Value::Null);
                    match kind {
                        Pending::Init => {
                            self.initialized = true;
                            self.status = "ready".into();
                            write_msg(&self.stdin, &json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}));
                            for q in std::mem::take(&mut self.queued) {
                                write_msg(&self.stdin, &q);
                            }
                            out.push(LspEvent::Status(format!("{} ready", self.name)));
                        }
                        Pending::Hover => {
                            let text = hover_text(res.get("contents").unwrap_or(&Value::Null));
                            if !text.trim().is_empty() {
                                out.push(LspEvent::Hover(text));
                            }
                        }
                        Pending::References => {
                            let mut v = Vec::new();
                            for l in res.as_array().cloned().unwrap_or_default() {
                                let uri = l.get("uri").and_then(|u| u.as_str()).unwrap_or("");
                                let line = l.pointer("/range/start/line").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
                                let ch = l.pointer("/range/start/character").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
                                if !uri.is_empty() {
                                    v.push((uri_to_path(uri), line, ch));
                                }
                            }
                            out.push(LspEvent::References(v));
                        }
                        Pending::Definition => {
                            let loc = if res.is_array() { res.get(0).cloned() } else if res.is_null() { None } else { Some(res) };
                            if let Some(l) = loc {
                                let uri = l.get("uri").or_else(|| l.get("targetUri")).and_then(|u| u.as_str()).unwrap_or("");
                                let range = l.get("range").or_else(|| l.get("targetSelectionRange"));
                                let line = range.and_then(|r| r.pointer("/start/line")).and_then(|x| x.as_u64()).unwrap_or(0) as usize;
                                let ch = range.and_then(|r| r.pointer("/start/character")).and_then(|x| x.as_u64()).unwrap_or(0) as usize;
                                if !uri.is_empty() {
                                    out.push(LspEvent::Definition(uri_to_path(uri), line, ch));
                                }
                            }
                        }
                        Pending::Other => {}
                    }
                }
                (None, Some(m)) => match m.as_str() {
                    "textDocument/publishDiagnostics" => {
                        let uri = msg.pointer("/params/uri").and_then(|u| u.as_str()).unwrap_or("");
                        let mut diags = Vec::new();
                        if let Some(arr) = msg.pointer("/params/diagnostics").and_then(|d| d.as_array()) {
                            for d in arr {
                                let g = |p: &str| d.pointer(p).and_then(|x| x.as_u64()).unwrap_or(0) as usize;
                                diags.push(Diag {
                                    line: g("/range/start/line"),
                                    col: g("/range/start/character"),
                                    end_line: g("/range/end/line"),
                                    end_col: g("/range/end/character"),
                                    severity: d.get("severity").and_then(|s| s.as_u64()).unwrap_or(1) as u8,
                                    message: d.get("message").and_then(|s| s.as_str()).unwrap_or("").to_string(),
                                    source: d.get("source").and_then(|s| s.as_str()).unwrap_or("").to_string(),
                                });
                            }
                        }
                        out.push(LspEvent::Diagnostics(norm_key(&uri_to_path(uri)), diags));
                    }
                    "window/logMessage" | "window/showMessage" => {
                        if let Some(t) = msg.pointer("/params/message").and_then(|s| s.as_str()) {
                            self.log.push(t.to_string());
                            if self.log.len() > 200 {
                                self.log.remove(0);
                            }
                        }
                    }
                    "$/progress" => {
                        if let Some(t) = msg.pointer("/params/value/title").and_then(|s| s.as_str()) {
                            self.status = t.to_string();
                        } else if msg.pointer("/params/value/kind").and_then(|s| s.as_str()) == Some("end") {
                            self.status = "ready".into();
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        if let Ok(Some(_)) = self.child.try_wait() {
            if self.status != "stopped" {
                self.status = "stopped".into();
                out.push(LspEvent::Status(format!("{} stopped", self.name)));
            }
        }
        out
    }
}

impl Drop for LspClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

fn hover_text(c: &Value) -> String {
    match c {
        Value::String(s) => s.clone(),
        Value::Array(a) => a.iter().map(hover_text).collect::<Vec<_>>().join("\n\n"),
        Value::Object(o) => o.get("value").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        _ => String::new(),
    }
}

/// UTF-16 column of a char column in a line.
pub fn utf16_col(line: &str, char_col: usize) -> usize {
    line.chars().take(char_col).map(|c| c.len_utf16()).sum()
}

/// Char column from a UTF-16 column.
pub fn char_col_from_utf16(line: &str, col16: usize) -> usize {
    let mut acc = 0;
    for (i, c) in line.chars().enumerate() {
        if acc >= col16 {
            return i;
        }
        acc += c.len_utf16();
    }
    line.chars().count()
}
