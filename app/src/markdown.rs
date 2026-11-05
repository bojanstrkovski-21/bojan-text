//! Markdown: live in-app preview (rendered with egui) and browser preview
//! (HTML served from a tiny local server so the page refreshes as you type).

use crate::colors::find_colors;
use eframe::egui::{self, Color32, RichText};
use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

fn options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_HEADING_ATTRIBUTES
}

// ---------------------------------------------------------------------------
// HTML

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

pub fn to_html(md: &str, title: &str, dark: bool, ss: &syntect::parsing::SyntaxSet, theme: &syntect::highlighting::Theme, live: bool) -> String {
    let mut events: Vec<Event> = Vec::new();
    let mut in_code: Option<String> = None;
    let mut code_buf = String::new();
    for ev in Parser::new_ext(md, options()) {
        match ev {
            Event::Start(Tag::CodeBlock(kind)) => {
                in_code = Some(match kind {
                    CodeBlockKind::Fenced(l) => l.to_string(),
                    CodeBlockKind::Indented => String::new(),
                });
                code_buf.clear();
            }
            Event::End(TagEnd::CodeBlock) => {
                let lang = in_code.take().unwrap_or_default();
                let token = lang.split_whitespace().next().unwrap_or("");
                let syn = ss.find_syntax_by_token(token).unwrap_or_else(|| ss.find_syntax_plain_text());
                let html = syntect::html::highlighted_html_for_string(&code_buf, ss, syn, theme)
                    .unwrap_or_else(|_| format!("<pre><code>{}</code></pre>", esc(&code_buf)));
                events.push(Event::Html(html.into()));
            }
            Event::Text(t) if in_code.is_some() => code_buf.push_str(&t),
            Event::Code(c) => {
                // show a swatch next to inline color codes like `#ff8800`
                let found = find_colors(&c, true);
                if found.len() == 1 && found[0].start == 0 && found[0].end == c.len() {
                    let col = found[0].color;
                    let [r, g, b, a] = col.to_u8();
                    events.push(Event::InlineHtml(
                        format!("<code><span class=\"sw\" style=\"background:rgba({r},{g},{b},{:.2})\"></span>{}</code>", a as f32 / 255.0, esc(&c)).into(),
                    ));
                } else {
                    events.push(Event::Code(c));
                }
            }
            e => events.push(e),
        }
    }
    let mut body = String::new();
    pulldown_cmark::html::push_html(&mut body, events.into_iter());
    let (bg, fg, muted, border, codebg, link) = if dark {
        ("#161b24", "#d6deeb", "#8b98ab", "#2a3342", "#1f2735", "#6cb6ff")
    } else {
        ("#ffffff", "#24292f", "#57606a", "#d0d7de", "#f6f8fa", "#0969da")
    };
    let script = if live {
        r#"<script>
let v=null;const k='bt-scroll';window.addEventListener('scroll',()=>sessionStorage.setItem(k,window.scrollY));
window.addEventListener('load',()=>{const y=sessionStorage.getItem(k);if(y)window.scrollTo(0,+y);});
setInterval(async()=>{try{const r=await fetch('/__version');const t=await r.text();if(v===null)v=t;else if(t!==v)location.reload();}catch(e){}},400);
</script>"#
    } else {
        ""
    };
    format!(
        r#"<!doctype html><html><head><meta charset="utf-8"><title>{title}</title>
<style>
body{{background:{bg};color:{fg};font:16px/1.65 "Segoe UI",system-ui,sans-serif;max-width:880px;margin:0 auto;padding:40px 32px}}
h1,h2{{border-bottom:1px solid {border};padding-bottom:.3em}} h1,h2,h3,h4{{line-height:1.25;margin-top:1.6em}}
a{{color:{link}}} code{{background:{codebg};padding:.15em .4em;border-radius:5px;font:0.9em Consolas,monospace}}
pre{{background:{codebg}!important;padding:14px 16px;border-radius:8px;overflow:auto;border:1px solid {border}}} pre code{{background:none;padding:0}}
blockquote{{margin:0;padding:.2em 1em;color:{muted};border-left:4px solid {border}}}
table{{border-collapse:collapse}} td,th{{border:1px solid {border};padding:6px 13px}} tr:nth-child(2n){{background:{codebg}}}
img{{max-width:100%}} hr{{border:0;border-top:1px solid {border};margin:2em 0}}
.sw{{display:inline-block;width:.8em;height:.8em;border-radius:3px;margin-right:.35em;vertical-align:-1px;border:1px solid #8886}}
li input{{margin-right:.4em}}
</style>{script}</head><body>
{body}
</body></html>"#,
        title = esc(title)
    )
}

/// Open a URL or file with the system's default browser.
pub fn open_url(url: &str) {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("rundll32").args(["url.dll,FileProtocolHandler", url]).spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
}

// ---------------------------------------------------------------------------
// Live browser preview server

#[derive(Default)]
struct Shared {
    version: u64,
    html: String,
    base_dir: Option<PathBuf>,
}

pub struct PreviewServer {
    shared: Arc<Mutex<Shared>>,
    pub port: u16,
}

impl PreviewServer {
    pub fn start() -> Option<Self> {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").ok()?;
        let port = listener.local_addr().ok()?.port();
        let shared = Arc::new(Mutex::new(Shared::default()));
        let sh = shared.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let sh = sh.clone();
                std::thread::spawn(move || handle(stream, sh));
            }
        });
        Some(Self { shared, port })
    }
    pub fn publish(&self, html: String, base_dir: Option<PathBuf>) {
        let mut s = self.shared.lock().unwrap();
        if s.html != html {
            s.html = html;
            s.version += 1;
        }
        s.base_dir = base_dir;
    }
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/", self.port)
    }
}

fn handle(mut stream: std::net::TcpStream, sh: Arc<Mutex<Shared>>) {
    let mut buf = [0u8; 4096];
    let n = stream.read(&mut buf).unwrap_or(0);
    let req = String::from_utf8_lossy(&buf[..n]);
    let path = req.split_whitespace().nth(1).unwrap_or("/").to_string();
    let (status, ctype, body): (&str, &str, Vec<u8>) = if path == "/" || path.starts_with("/?") {
        ("200 OK", "text/html; charset=utf-8", sh.lock().unwrap().html.clone().into_bytes())
    } else if path == "/__version" {
        ("200 OK", "text/plain", sh.lock().unwrap().version.to_string().into_bytes())
    } else {
        // relative files (images) next to the markdown file
        let base = sh.lock().unwrap().base_dir.clone();
        let rel = percent_decode(path.trim_start_matches('/'));
        match base.and_then(|b| {
            let p = b.join(&rel).canonicalize().ok()?;
            let bb = b.canonicalize().ok()?;
            if p.starts_with(&bb) { std::fs::read(&p).ok().map(|d| (p, d)) } else { None }
        }) {
            Some((p, d)) => {
                let ext = p.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
                let ct = match ext.as_str() {
                    "png" => "image/png",
                    "jpg" | "jpeg" => "image/jpeg",
                    "gif" => "image/gif",
                    "svg" => "image/svg+xml",
                    "webp" => "image/webp",
                    _ => "application/octet-stream",
                };
                ("200 OK", ct, d)
            }
            None => ("404 Not Found", "text/plain", b"not found".to_vec()),
        }
    };
    let head = format!("HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n", body.len());
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(&body);
}

fn percent_decode(s: &str) -> String {
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
    String::from_utf8_lossy(&out).to_string()
}

// ---------------------------------------------------------------------------
// In-app renderer

#[derive(Clone, Default)]
struct Run {
    text: String,
    bold: bool,
    italic: bool,
    strike: bool,
    code: bool,
    link: Option<String>,
}

enum Block {
    Heading(u8, Vec<Run>),
    Para(Vec<Run>),
    Code(String, String),
    Quote(Vec<Block>),
    List(Option<u64>, Vec<(Option<bool>, Vec<Block>)>),
    Rule,
    Table(Vec<Vec<Vec<Run>>>),
    Html(String),
}

enum Frame {
    Doc(Vec<Block>),
    Quote(Vec<Block>),
    List(Option<u64>, Vec<(Option<bool>, Vec<Block>)>),
    Item(Option<bool>, Vec<Block>, Vec<Run>),
}

fn push_block(stack: &mut [Frame], b: Block) {
    match stack.last_mut().unwrap() {
        Frame::Doc(v) | Frame::Quote(v) => v.push(b),
        Frame::Item(_, v, runs) => {
            if !runs.is_empty() {
                v.push(Block::Para(std::mem::take(runs)));
            }
            v.push(b)
        }
        Frame::List(..) => {}
    }
}

fn parse_blocks(md: &str) -> Vec<Block> {
    let mut stack = vec![Frame::Doc(Vec::new())];
    let mut runs: Vec<Run> = Vec::new();
    let mut style = Run::default();
    let mut heading: Option<u8> = None;
    let mut code: Option<(String, String)> = None;
    let mut table: Option<Vec<Vec<Vec<Run>>>> = None;
    let mut in_para = false;
    for ev in Parser::new_ext(md, options()) {
        match ev {
            Event::Start(t) => match t {
                Tag::Paragraph => { runs.clear(); in_para = true; }
                Tag::Heading { level, .. } => {
                    runs.clear();
                    heading = Some(match level {
                        HeadingLevel::H1 => 1, HeadingLevel::H2 => 2, HeadingLevel::H3 => 3,
                        HeadingLevel::H4 => 4, HeadingLevel::H5 => 5, HeadingLevel::H6 => 6,
                    });
                }
                Tag::BlockQuote(_) => stack.push(Frame::Quote(Vec::new())),
                Tag::CodeBlock(k) => {
                    let lang = match k { CodeBlockKind::Fenced(l) => l.to_string(), _ => String::new() };
                    code = Some((lang, String::new()));
                }
                Tag::List(start) => {
                    if let Some(Frame::Item(_, v, _)) = stack.last_mut() {
                        if !runs.is_empty() {
                            v.push(Block::Para(std::mem::take(&mut runs)));
                        }
                    }
                    stack.push(Frame::List(start, Vec::new()))
                }
                Tag::Item => { runs.clear(); stack.push(Frame::Item(None, Vec::new(), Vec::new())); }
                Tag::Table(_) => table = Some(Vec::new()),
                Tag::TableHead | Tag::TableRow => { if let Some(t) = table.as_mut() { t.push(Vec::new()); } }
                Tag::TableCell => runs.clear(),
                Tag::Emphasis => style.italic = true,
                Tag::Strong => style.bold = true,
                Tag::Strikethrough => style.strike = true,
                Tag::Link { dest_url, .. } => style.link = Some(dest_url.to_string()),
                Tag::Image { dest_url, .. } => {
                    runs.push(Run { text: format!("🖼 {}", dest_url), italic: true, ..Default::default() });
                }
                _ => {}
            },
            Event::End(t) => match t {
                TagEnd::Paragraph => {
                    in_para = false;
                    let r = std::mem::take(&mut runs);
                    if let Some(Frame::Item(_, _, ir)) = stack.last_mut() {
                        ir.extend(r);
                        let Some(Frame::Item(_, v, ir)) = stack.last_mut() else { unreachable!() };
                        v.push(Block::Para(std::mem::take(ir)));
                    } else {
                        push_block(&mut stack, Block::Para(r));
                    }
                }
                TagEnd::Heading(_) => {
                    let r = std::mem::take(&mut runs);
                    push_block(&mut stack, Block::Heading(heading.take().unwrap_or(1), r));
                }
                TagEnd::BlockQuote(_) => {
                    if let Some(Frame::Quote(v)) = stack.pop() { push_block(&mut stack, Block::Quote(v)); }
                }
                TagEnd::CodeBlock => {
                    if let Some((l, c)) = code.take() { push_block(&mut stack, Block::Code(l, c)); }
                }
                TagEnd::List(_) => {
                    if let Some(Frame::List(s, items)) = stack.pop() { push_block(&mut stack, Block::List(s, items)); }
                }
                TagEnd::Item => {
                    if let Some(Frame::Item(task, mut v, mut ir)) = stack.pop() {
                        ir.extend(std::mem::take(&mut runs));
                        if !ir.is_empty() { v.push(Block::Para(ir)); }
                        if let Some(Frame::List(_, items)) = stack.last_mut() { items.push((task, v)); }
                    }
                }
                TagEnd::TableCell => {
                    let r = std::mem::take(&mut runs);
                    if let Some(row) = table.as_mut().and_then(|t| t.last_mut()) { row.push(r); }
                }
                TagEnd::Table => {
                    if let Some(t) = table.take() { push_block(&mut stack, Block::Table(t)); }
                }
                TagEnd::Emphasis => style.italic = false,
                TagEnd::Strong => style.bold = false,
                TagEnd::Strikethrough => style.strike = false,
                TagEnd::Link => style.link = None,
                _ => {}
            },
            Event::Text(t) => {
                if let Some((_, c)) = code.as_mut() {
                    c.push_str(&t);
                } else {
                    runs.push(Run { text: t.to_string(), ..style.clone() });
                }
            }
            Event::Code(t) => runs.push(Run { text: t.to_string(), code: true, ..style.clone() }),
            Event::SoftBreak => runs.push(Run { text: " ".into(), ..style.clone() }),
            Event::HardBreak => runs.push(Run { text: "\n".into(), ..style.clone() }),
            Event::Rule => push_block(&mut stack, Block::Rule),
            Event::TaskListMarker(done) => {
                if let Some(Frame::Item(task, ..)) = stack.last_mut() { *task = Some(done); }
            }
            Event::Html(h) => push_block(&mut stack, Block::Html(h.to_string())),
            Event::InlineHtml(h) => runs.push(Run { text: h.to_string(), code: true, ..Default::default() }),
            _ => {}
        }
        let _ = in_para;
    }
    // tight list items carry text without a paragraph
    match stack.into_iter().next() {
        Some(Frame::Doc(v)) => v,
        _ => Vec::new(),
    }
}

pub struct MdStyle {
    pub fg: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub code_bg: Color32,
    pub border: Color32,
    pub font_size: f32,
}

pub struct MdView {
    cache_src: u64,
    blocks: Vec<Block>,
}

impl Default for MdView {
    fn default() -> Self {
        Self { cache_src: u64::MAX, blocks: Vec::new() }
    }
}

impl MdView {
    pub fn show(&mut self, ui: &mut egui::Ui, md: &str, st: &MdStyle, code_hl: &mut dyn FnMut(&mut egui::Ui, &str, &str)) {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        md.hash(&mut h);
        let key = h.finish();
        if key != self.cache_src {
            self.blocks = parse_blocks(md);
            self.cache_src = key;
        }
        let blocks = std::mem::take(&mut self.blocks);
        for b in &blocks {
            show_block(ui, b, st, code_hl, 0);
        }
        self.blocks = blocks;
    }
}

fn runs_ui(ui: &mut egui::Ui, runs: &[Run], st: &MdStyle, size: f32, strong: bool, color: Color32) {
    runs_ui_ex(ui, runs, st, size, strong, color, true)
}

fn runs_ui_ex(ui: &mut egui::Ui, runs: &[Run], st: &MdStyle, size: f32, strong: bool, color: Color32, wrap: bool) {
    let body = |ui: &mut egui::Ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for r in runs {
            let mut t = RichText::new(&r.text).size(if r.code { size * 0.92 } else { size }).color(color);
            if r.bold || strong { t = t.strong(); }
            if r.italic { t = t.italics(); }
            if r.strike { t = t.strikethrough(); }
            if r.code {
                t = t.monospace().background_color(st.code_bg);
                let cols = find_colors(&r.text, true);
                if cols.len() == 1 && cols[0].end - cols[0].start == r.text.len() {
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(size * 0.8, size * 0.8), egui::Sense::hover());
                    ui.painter().rect_filled(rect.shrink(1.0), 3.0, cols[0].color.to_color32());
                    ui.add_space(3.0);
                }
            }
            if let Some(link) = &r.link {
                let resp = ui.add(egui::Label::new(t.color(st.accent).underline()).sense(egui::Sense::click()));
                if resp.clicked() {
                    open_url(link);
                }
                resp.on_hover_text(link);
            } else {
                if wrap {
                    ui.label(t);
                } else {
                    ui.add(egui::Label::new(t).extend());
                }
            }
        }
    };
    if wrap {
        ui.horizontal_wrapped(body);
    } else {
        ui.horizontal(body);
    }
}

fn show_block(ui: &mut egui::Ui, b: &Block, st: &MdStyle, code_hl: &mut dyn FnMut(&mut egui::Ui, &str, &str), depth: usize) {
    let fs = st.font_size;
    match b {
        Block::Heading(l, runs) => {
            let size = match l { 1 => fs * 2.0, 2 => fs * 1.6, 3 => fs * 1.3, 4 => fs * 1.15, _ => fs };
            ui.add_space(if *l <= 2 { 10.0 } else { 6.0 });
            runs_ui(ui, runs, st, size, true, st.fg);
            if *l <= 2 {
                let r = ui.available_rect_before_wrap();
                ui.painter().hline(r.x_range(), r.top() + 2.0, egui::Stroke::new(1.0, st.border));
                ui.add_space(6.0);
            }
            ui.add_space(4.0);
        }
        Block::Para(runs) => {
            runs_ui(ui, runs, st, fs, false, st.fg);
            ui.add_space(fs * 0.6);
        }
        Block::Code(lang, code) => {
            egui::Frame::new().fill(st.code_bg).corner_radius(8.0).inner_margin(egui::Margin::same(10)).stroke(egui::Stroke::new(1.0, st.border)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                code_hl(ui, lang, code.trim_end_matches('\n'));
            });
            ui.add_space(fs * 0.6);
        }
        Block::Quote(blocks) => {
            let resp = egui::Frame::new().inner_margin(egui::Margin { left: 14, right: 4, top: 2, bottom: 2 }).show(ui, |ui| {
                for b in blocks {
                    show_block(ui, b, &MdStyle { fg: st.muted, ..*st }, code_hl, depth);
                }
            });
            let r = resp.response.rect;
            ui.painter().rect_filled(egui::Rect::from_min_max(r.left_top(), egui::pos2(r.left() + 4.0, r.bottom())), 2.0, st.border);
        }
        Block::List(start, items) => {
            for (i, (task, blocks)) in items.iter().enumerate() {
                ui.horizontal_top(|ui| {
                    ui.add_space(8.0 + depth as f32 * 4.0);
                    let marker = match (task, start) {
                        (Some(true), _) => "☑".to_string(),
                        (Some(false), _) => "☐".to_string(),
                        (None, Some(s)) => format!("{}.", s + i as u64),
                        (None, None) => if depth % 2 == 0 { "•".into() } else { "◦".into() },
                    };
                    ui.label(RichText::new(marker).size(fs).color(st.accent));
                    ui.vertical(|ui| {
                        for b in blocks {
                            match b {
                                Block::Para(r) => runs_ui(ui, r, st, fs, false, st.fg),
                                other => show_block(ui, other, st, code_hl, depth + 1),
                            }
                        }
                    });
                });
            }
            ui.add_space(fs * 0.5);
        }
        Block::Rule => {
            ui.add_space(6.0);
            let r = ui.available_rect_before_wrap();
            ui.painter().hline(r.x_range(), r.top(), egui::Stroke::new(1.0, st.border));
            ui.add_space(10.0);
        }
        Block::Table(rows) => {
            egui::Frame::new().stroke(egui::Stroke::new(1.0, st.border)).corner_radius(6.0).inner_margin(egui::Margin::same(6)).show(ui, |ui| {
                egui::Grid::new(ui.next_auto_id()).striped(true).spacing([18.0, 6.0]).show(ui, |ui| {
                    for (ri, row) in rows.iter().enumerate() {
                        for cell in row {
                            runs_ui_ex(ui, cell, st, fs, ri == 0, st.fg, false);
                        }
                        ui.end_row();
                    }
                });
            });
            ui.add_space(fs * 0.6);
        }
        Block::Html(h) => {
            ui.label(RichText::new(h.trim()).monospace().color(st.muted).size(fs * 0.85));
        }
    }
}
