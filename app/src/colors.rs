//! Color-code recognizer ("colorizer") and the color-expression evaluator used
//! for Sublime `.sublime-color-scheme` files.
//!
//! Recognized in documents:
//!   #rgb #rgba #rrggbb #rrggbbaa, 0xRRGGBB / 0xAARRGGBB,
//!   rgb()/rgba()/hsl()/hsla()/hwb()/lab()/lch()/oklab()/oklch()/color(srgb ..)
//!   (comma or space syntax, %, deg/rad/turn, "/ alpha"),
//!   code calls such as Color32::from_rgb(1,2,3), Color.FromArgb(a,r,g,b), vec3-like
//!   float tuples after an identifier ending in rgb/rgba/argb,
//!   CSS named colors (configurable).

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba {
    pub r: f32, // 0..1
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Rgba {
    pub fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r: r.clamp(0.0, 1.0), g: g.clamp(0.0, 1.0), b: b.clamp(0.0, 1.0), a: a.clamp(0.0, 1.0) }
    }
    pub fn from_u8(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self::new(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a as f32 / 255.0)
    }
    pub fn to_u8(self) -> [u8; 4] {
        let c = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        [c(self.r), c(self.g), c(self.b), c(self.a)]
    }
    pub fn to_color32(self) -> eframe::egui::Color32 {
        let [r, g, b, a] = self.to_u8();
        eframe::egui::Color32::from_rgba_unmultiplied(r, g, b, a)
    }
    pub fn luminance(self) -> f32 {
        let lin = |c: f32| if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) };
        0.2126 * lin(self.r) + 0.7152 * lin(self.g) + 0.0722 * lin(self.b)
    }
    pub fn to_hex(self) -> String {
        let [r, g, b, a] = self.to_u8();
        if a == 255 { format!("#{r:02x}{g:02x}{b:02x}") } else { format!("#{r:02x}{g:02x}{b:02x}{a:02x}") }
    }
    pub fn to_rgb_css(self) -> String {
        let [r, g, b, _] = self.to_u8();
        if self.a >= 0.999 { format!("rgb({r}, {g}, {b})") } else { format!("rgba({r}, {g}, {b}, {})", trim(self.a)) }
    }
    pub fn to_hsl_css(self) -> String {
        let (h, s, l) = rgb_to_hsl(self.r, self.g, self.b);
        if self.a >= 0.999 {
            format!("hsl({}, {}%, {}%)", h.round(), (s * 100.0).round(), (l * 100.0).round())
        } else {
            format!("hsla({}, {}%, {}%, {})", h.round(), (s * 100.0).round(), (l * 100.0).round(), trim(self.a))
        }
    }
    /// Mix toward `other` keeping `t` of self (Sublime's blend(x, p%) keeps p% of the base).
    pub fn blend(self, other: Rgba, keep: f32) -> Rgba {
        let k = keep.clamp(0.0, 1.0);
        Rgba::new(
            self.r * k + other.r * (1.0 - k),
            self.g * k + other.g * (1.0 - k),
            self.b * k + other.b * (1.0 - k),
            self.a,
        )
    }
}

fn trim(v: f32) -> String {
    let s = format!("{:.2}", v);
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

pub fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (f32, f32, f32) {
    let h = ((h % 360.0) + 360.0) % 360.0 / 360.0;
    if s <= 0.0 {
        return (l, l, l);
    }
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    let f = |mut t: f32| {
        if t < 0.0 { t += 1.0 }
        if t > 1.0 { t -= 1.0 }
        if t < 1.0 / 6.0 { p + (q - p) * 6.0 * t }
        else if t < 0.5 { q }
        else if t < 2.0 / 3.0 { p + (q - p) * (2.0 / 3.0 - t) * 6.0 }
        else { p }
    };
    (f(h + 1.0 / 3.0), f(h), f(h - 1.0 / 3.0))
}

pub fn rgb_to_hsl(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    if (max - min).abs() < 1e-6 {
        return (0.0, 0.0, l);
    }
    let d = max - min;
    let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
    let h = if max == r { (g - b) / d + if g < b { 6.0 } else { 0.0 } }
        else if max == g { (b - r) / d + 2.0 }
        else { (r - g) / d + 4.0 };
    (h * 60.0, s, l)
}

fn hwb_to_rgb(h: f32, w: f32, bl: f32) -> (f32, f32, f32) {
    let (mut w, mut bl) = (w, bl);
    if w + bl >= 1.0 {
        let g = w / (w + bl);
        return (g, g, g);
    }
    let (r, g, b) = hsl_to_rgb(h, 1.0, 0.5);
    let k = 1.0 - w - bl;
    w = w.max(0.0);
    bl = bl.max(0.0);
    let _ = bl;
    (r * k + w, g * k + w, b * k + w)
}

fn lin_to_srgb(c: f32) -> f32 {
    if c <= 0.0031308 { 12.92 * c } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 }
}

fn oklab_to_rgb(l: f32, a: f32, b: f32) -> (f32, f32, f32) {
    let l_ = l + 0.396_337_78 * a + 0.215_803_76 * b;
    let m_ = l - 0.105_561_346 * a - 0.063_854_17 * b;
    let s_ = l - 0.089_484_18 * a - 1.291_485_5 * b;
    let (l3, m3, s3) = (l_ * l_ * l_, m_ * m_ * m_, s_ * s_ * s_);
    let r = 4.076_741_7 * l3 - 3.307_711_6 * m3 + 0.230_969_94 * s3;
    let g = -1.268_438 * l3 + 2.609_757_4 * m3 - 0.341_319_38 * s3;
    let bb = -0.004_196_086_3 * l3 - 0.703_418_6 * m3 + 1.707_614_7 * s3;
    (lin_to_srgb(r), lin_to_srgb(g), lin_to_srgb(bb))
}

fn lab_to_rgb(l: f32, a: f32, b: f32) -> (f32, f32, f32) {
    // CIE Lab (D50) -> XYZ D50 -> linear sRGB (Bradford-adapted matrix)
    let fy = (l + 16.0) / 116.0;
    let fx = fy + a / 500.0;
    let fz = fy - b / 200.0;
    let e = 216.0 / 24389.0;
    let k = 24389.0 / 27.0;
    let finv = |t: f32| if t * t * t > e { t * t * t } else { (116.0 * t - 16.0) / k };
    let x = finv(fx) * 0.964_22;
    let y = if l > k * e { fy * fy * fy } else { l / k };
    let z = finv(fz) * 0.825_21;
    let r = 3.134_136_5 * x - 1.617_386_6 * y - 0.490_662_9 * z;
    let g = -0.978_795_6 * x + 1.916_254 * y + 0.033_442_2 * z;
    let bb = 0.071_955_4 * x - 0.228_976_7 * y + 1.405_386_1 * z;
    (lin_to_srgb(r), lin_to_srgb(g), lin_to_srgb(bb))
}

/// Which way a color was written, so the editor can write an edited color back the same way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorKind {
    Hex,
    HexInt,
    Rgb,
    Hsl,
    Hwb,
    Lab,
    Named,
    CodeCall,
}

#[derive(Clone, Debug)]
pub struct ColorMatch {
    pub start: usize, // byte offset
    pub end: usize,
    pub color: Rgba,
    pub kind: ColorKind,
}

impl ColorMatch {
    pub fn format_like(&self, c: Rgba, original: &str) -> String {
        match self.kind {
            ColorKind::Hex => {
                let h = c.to_hex();
                if original.chars().any(|ch| ch.is_ascii_uppercase()) { h.to_uppercase() } else { h }
            }
            ColorKind::HexInt => {
                let [r, g, b, a] = c.to_u8();
                let upper = original[2..].chars().any(|ch| ch.is_ascii_uppercase());
                let s = if original.len() >= 10 { format!("{a:02x}{r:02x}{g:02x}{b:02x}") } else { format!("{r:02x}{g:02x}{b:02x}") };
                format!("{}{}", &original[..2], if upper { s.to_uppercase() } else { s })
            }
            ColorKind::Rgb => c.to_rgb_css(),
            ColorKind::Hsl => c.to_hsl_css(),
            _ => c.to_hex(),
        }
    }
}

pub fn named_color(name: &str) -> Option<Rgba> {
    let lower = name.to_ascii_lowercase();
    if lower == "transparent" {
        return Some(Rgba::new(0.0, 0.0, 0.0, 0.0));
    }
    NAMED.iter().find(|(n, _)| *n == lower).map(|(_, v)| {
        Rgba::from_u8((v >> 16) as u8, (v >> 8) as u8, *v as u8, 255)
    })
}

fn is_word(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn parse_hex_digits(s: &str) -> Option<Rgba> {
    let v: Vec<u8> = s.bytes().map(|c| (c as char).to_digit(16).map(|d| d as u8)).collect::<Option<_>>()?;
    match v.len() {
        3 => Some(Rgba::from_u8(v[0] * 17, v[1] * 17, v[2] * 17, 255)),
        4 => Some(Rgba::from_u8(v[0] * 17, v[1] * 17, v[2] * 17, v[3] * 17)),
        6 => Some(Rgba::from_u8(v[0] * 16 + v[1], v[2] * 16 + v[3], v[4] * 16 + v[5], 255)),
        8 => Some(Rgba::from_u8(v[0] * 16 + v[1], v[2] * 16 + v[3], v[4] * 16 + v[5], v[6] * 16 + v[7])),
        _ => None,
    }
}

/// A numeric argument: value plus unit ("%", "deg", "rad", "turn", "" ).
#[derive(Clone, Debug)]
struct Arg {
    v: f32,
    unit: String,
    had_dot: bool,
}

fn parse_args(inner: &str) -> Option<(Vec<Arg>, Option<Arg>)> {
    // Split on commas, whitespace and '/'; '/' introduces alpha.
    let mut args = Vec::new();
    let mut alpha = None;
    let mut after_slash = false;
    for tok in inner.split(|c: char| c == ',' || c.is_whitespace()).filter(|t| !t.is_empty()) {
        let mut tok = tok;
        if tok == "/" {
            after_slash = true;
            continue;
        }
        if let Some(rest) = tok.strip_prefix('/') {
            after_slash = true;
            tok = rest;
        }
        let a = if tok.eq_ignore_ascii_case("none") {
            Arg { v: 0.0, unit: String::new(), had_dot: false }
        } else {
            let split = tok.find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == '+' || c == 'e' && false)).unwrap_or(tok.len());
            let (num, unit) = tok.split_at(split);
            let v: f32 = num.parse().ok()?;
            let unit = unit.to_ascii_lowercase();
            if !matches!(unit.as_str(), "" | "%" | "deg" | "rad" | "turn" | "grad" | "f") {
                return None;
            }
            Arg { v, unit, had_dot: num.contains('.') }
        };
        if after_slash { alpha = Some(a) } else { args.push(a) }
    }
    Some((args, alpha))
}

fn chan255(a: &Arg) -> f32 {
    if a.unit == "%" { a.v / 100.0 } else { a.v / 255.0 }
}
fn unit01(a: &Arg) -> f32 {
    if a.unit == "%" { a.v / 100.0 } else { a.v }
}
fn hue(a: &Arg) -> f32 {
    match a.unit.as_str() {
        "rad" => a.v.to_degrees(),
        "turn" => a.v * 360.0,
        "grad" => a.v * 0.9,
        _ => a.v,
    }
}
fn pct(a: &Arg) -> f32 {
    if a.unit == "%" { a.v / 100.0 } else if a.v > 1.0 { a.v / 100.0 } else { a.v }
}

/// Evaluate a CSS-style color function by name with its inner argument text.
pub fn eval_function(name: &str, inner: &str) -> Option<(Rgba, ColorKind)> {
    let lname = name.to_ascii_lowercase();
    let (args, alpha) = parse_args(inner)?;
    let mut a = alpha.as_ref().map(unit01);
    let res = match lname.as_str() {
        "rgb" | "rgba" => {
            if args.len() < 3 || args.len() > 4 { return None; }
            if args.len() == 4 { a = Some(unit01(&args[3])); }
            (Rgba::new(chan255(&args[0]), chan255(&args[1]), chan255(&args[2]), a.unwrap_or(1.0)), ColorKind::Rgb)
        }
        "hsl" | "hsla" => {
            if args.len() < 3 || args.len() > 4 { return None; }
            if args.len() == 4 { a = Some(unit01(&args[3])); }
            let (r, g, b) = hsl_to_rgb(hue(&args[0]), pct(&args[1]), pct(&args[2]));
            (Rgba::new(r, g, b, a.unwrap_or(1.0)), ColorKind::Hsl)
        }
        "hwb" | "hwba" => {
            if args.len() < 3 { return None; }
            if args.len() == 4 { a = Some(unit01(&args[3])); }
            let (r, g, b) = hwb_to_rgb(hue(&args[0]), pct(&args[1]), pct(&args[2]));
            (Rgba::new(r, g, b, a.unwrap_or(1.0)), ColorKind::Hwb)
        }
        "lab" => {
            if args.len() != 3 { return None; }
            let l = if args[0].unit == "%" { args[0].v } else { args[0].v };
            let (r, g, b) = lab_to_rgb(l, args[1].v, args[2].v);
            (Rgba::new(r, g, b, a.unwrap_or(1.0)), ColorKind::Lab)
        }
        "lch" => {
            if args.len() != 3 { return None; }
            let h = hue(&args[2]).to_radians();
            let (r, g, b) = lab_to_rgb(args[0].v, args[1].v * h.cos(), args[1].v * h.sin());
            (Rgba::new(r, g, b, a.unwrap_or(1.0)), ColorKind::Lab)
        }
        "oklab" => {
            if args.len() != 3 { return None; }
            let l = if args[0].unit == "%" { args[0].v / 100.0 } else { args[0].v };
            let (r, g, b) = oklab_to_rgb(l, args[1].v, args[2].v);
            (Rgba::new(r, g, b, a.unwrap_or(1.0)), ColorKind::Lab)
        }
        "oklch" => {
            if args.len() != 3 { return None; }
            let l = if args[0].unit == "%" { args[0].v / 100.0 } else { args[0].v };
            let h = hue(&args[2]).to_radians();
            let (r, g, b) = oklab_to_rgb(l, args[1].v * h.cos(), args[1].v * h.sin());
            (Rgba::new(r, g, b, a.unwrap_or(1.0)), ColorKind::Lab)
        }
        _ => return None,
    };
    Some(res)
}

/// Code-style constructors: `from_rgb(255, 0, 0)`, `FromArgb(255, 10, 20, 30)`,
/// `vec3(1.0, 0.5, 0.0)` after an identifier ending in rgb/rgba/argb/color.
fn eval_code_call(ident: &str, inner: &str) -> Option<Rgba> {
    let l = ident.to_ascii_lowercase();
    let nums: Vec<&str> = inner.split(',').map(|s| s.trim().trim_end_matches('f').trim_end_matches('F')).collect();
    if nums.len() < 3 || nums.len() > 4 {
        return None;
    }
    let vals: Vec<f32> = nums.iter().map(|s| s.parse::<f32>().ok()).collect::<Option<_>>()?;
    let floats = nums.iter().any(|s| s.contains('.')) && vals.iter().all(|v| *v <= 1.0 && *v >= 0.0);
    let norm = |v: f32| if floats { v } else { v / 255.0 };
    if vals.iter().any(|v| *v < 0.0 || (!floats && *v > 255.0)) {
        return None;
    }
    let argb = l.ends_with("argb");
    if argb {
        if vals.len() != 4 { return None; }
        return Some(Rgba::new(norm(vals[1]), norm(vals[2]), norm(vals[3]), norm(vals[0])));
    }
    let ok = l.ends_with("rgb") || l.ends_with("rgba") || l.ends_with("rgb8") || l.ends_with("rgba8")
        || l.ends_with("unmultiplied") || l.ends_with("rgba_premultiplied") || l == "color" || l.ends_with("::color") || l.ends_with(".color") || l == "color32" || l.ends_with("colorf") || l == "rgbcolor";
    if !ok {
        return None;
    }
    let a = if vals.len() == 4 { norm(vals[3]) } else { 1.0 };
    Some(Rgba::new(norm(vals[0]), norm(vals[1]), norm(vals[2]), a))
}

/// Settings for where named colors are recognized.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum NamedMode {
    Off,
    StyleFiles,
    Everywhere,
}

/// Find every color code in `text`.
pub fn find_colors(text: &str, named: bool) -> Vec<ColorMatch> {
    let bytes = text.as_bytes();
    let n = bytes.len();
    let mut out = Vec::new();
    let mut i = 0;
    while i < n {
        let c = bytes[i];
        // #hex
        if c == b'#' && (i == 0 || !is_word(bytes[i - 1]) && bytes[i - 1] != b'&') {
            let mut j = i + 1;
            while j < n && bytes[j].is_ascii_hexdigit() && j - i <= 9 {
                j += 1;
            }
            let len = j - i - 1;
            if matches!(len, 3 | 4 | 6 | 8) && (j >= n || !is_word(bytes[j])) {
                if let Some(col) = parse_hex_digits(&text[i + 1..j]) {
                    out.push(ColorMatch { start: i, end: j, color: col, kind: ColorKind::Hex });
                    i = j;
                    continue;
                }
            }
        }
        // 0xRRGGBB / 0xAARRGGBB
        if c == b'0' && i + 1 < n && (bytes[i + 1] == b'x' || bytes[i + 1] == b'X') && (i == 0 || !is_word(bytes[i - 1])) {
            let mut j = i + 2;
            while j < n && bytes[j].is_ascii_hexdigit() {
                j += 1;
            }
            let len = j - i - 2;
            if (len == 6 || len == 8) && (j >= n || !is_word(bytes[j]) || matches!(bytes[j], b'u' | b'U' | b'l' | b'L')) {
                let d = &text[i + 2..j];
                let col = if len == 6 {
                    parse_hex_digits(d)
                } else {
                    // AARRGGBB
                    parse_hex_digits(&format!("{}{}", &d[2..], &d[..2]))
                };
                if let Some(col) = col {
                    out.push(ColorMatch { start: i, end: j, color: col, kind: ColorKind::HexInt });
                    i = j;
                    continue;
                }
            }
            i = j.max(i + 1);
            continue;
        }
        // identifiers: functions or named colors
        if (c.is_ascii_alphabetic() || c == b'_') && (i == 0 || !is_word(bytes[i - 1]) || false) {
            let mut j = i;
            while j < n && (is_word(bytes[j]) || bytes[j] == b':' || bytes[j] == b'.') {
                j += 1;
            }
            // trim trailing separators
            let mut id_end = j;
            while id_end > i && (bytes[id_end - 1] == b':' || bytes[id_end - 1] == b'.') {
                id_end -= 1;
            }
            let ident = &text[i..id_end];
            if id_end < n && bytes[id_end] == b'(' {
                if let Some(close_rel) = text[id_end + 1..].find(')') {
                    let close = id_end + 1 + close_rel;
                    let inner = &text[id_end + 1..close];
                    if inner.len() < 80 && !inner.contains('(') && !inner.contains('\n') {
                        // last segment of the identifier is the function name
                        let last = ident.rsplit(|ch| ch == ':' || ch == '.').next().unwrap_or(ident);
                        let css = eval_function(last, inner);
                        let code = if css.is_none() { eval_code_call(ident, inner).or_else(|| eval_code_call(last, inner)) } else { None };
                        if let Some((col, kind)) = css {
                            let start = id_end - last.len();
                            out.push(ColorMatch { start, end: close + 1, color: col, kind });
                            i = close + 1;
                            continue;
                        } else if let Some(col) = code {
                            out.push(ColorMatch { start: i, end: close + 1, color: col, kind: ColorKind::CodeCall });
                            i = close + 1;
                            continue;
                        }
                    }
                }
            } else if named && id_end - i >= 3 && id_end - i <= 20 && ident.bytes().all(|b| b.is_ascii_alphabetic()) {
                let before_ok = i == 0 || !matches!(bytes[i - 1], b'-' | b'.' | b'$' | b'@' | b'#');
                let after_ok = id_end >= n || !matches!(bytes[id_end], b'-' | b':' | b'(' | b'_');
                if before_ok && after_ok {
                    if let Some(col) = named_color(ident) {
                        out.push(ColorMatch { start: i, end: id_end, color: col, kind: ColorKind::Named });
                    }
                }
            }
            i = j.max(i + 1);
            continue;
        }
        i += 1;
    }
    out
}

// ---------------------------------------------------------------------------
// Sublime color-scheme expressions: "var(x)", "color(var(x) alpha(0.5))",
// "color(var(x) blend(#fff 50%))", "hsl(..)", "#hex", named colors.
// Mirrors the modifiers the original parses in function 0x140173198:
// rgb/rgba/hsl/hsla/blend/blenda/alpha/lightness/saturation/min-contrast.

pub fn eval_scheme_color(expr: &str, vars: &std::collections::HashMap<String, String>, depth: u32) -> Option<Rgba> {
    let e = expr.trim();
    if depth > 16 {
        return None;
    }
    if let Some(name) = e.strip_prefix("var(").and_then(|s| s.strip_suffix(')')) {
        let v = vars.get(name.trim())?;
        return eval_scheme_color(v, vars, depth + 1);
    }
    if let Some(h) = e.strip_prefix('#') {
        return parse_hex_digits(h);
    }
    if let Some(body) = e.strip_prefix("color(").and_then(|s| s.strip_suffix(')')) {
        // base color then modifiers
        let parts = split_top_level(body);
        let mut it = parts.into_iter();
        let base = it.next()?;
        let mut col = eval_scheme_color(&base, vars, depth + 1)?;
        for m in it {
            let (mname, margs) = match m.find('(') {
                Some(p) if m.ends_with(')') => (m[..p].trim().to_ascii_lowercase(), m[p + 1..m.len() - 1].to_string()),
                _ => continue,
            };
            match mname.as_str() {
                "alpha" | "a" => {
                    let s = margs.trim();
                    col.a = if let Some(p) = s.strip_suffix('%') { p.parse::<f32>().ok()? / 100.0 } else { s.parse().ok()? };
                }
                "blend" | "blenda" => {
                    let sub = split_top_level(&margs);
                    if sub.len() >= 2 {
                        let other = eval_scheme_color(&sub[0], vars, depth + 1)?;
                        let p: f32 = sub[1].trim().trim_end_matches('%').parse().ok()?;
                        let alpha = col.a;
                        col = col.blend(other, p / 100.0);
                        col.a = if mname == "blenda" { alpha * (p / 100.0) + other.a * (1.0 - p / 100.0) } else { alpha };
                    }
                }
                "lightness" | "l" | "saturation" | "s" => {
                    let s = margs.trim();
                    let (h, sat, l) = rgb_to_hsl(col.r, col.g, col.b);
                    let rel = s.starts_with('+') || s.starts_with('-');
                    let v: f32 = s.trim_start_matches('+').trim_end_matches('%').parse().ok()?;
                    let v = v / 100.0;
                    let (sat, l) = if mname.starts_with('l') {
                        (sat, if rel { l + v } else { v })
                    } else {
                        (if rel { sat + v } else { v }, l)
                    };
                    let (r, g, b) = hsl_to_rgb(h, sat.clamp(0.0, 1.0), l.clamp(0.0, 1.0));
                    col = Rgba::new(r, g, b, col.a);
                }
                _ => {} // min-contrast etc. are ignored
            }
        }
        return Some(col);
    }
    if let Some(p) = e.find('(') {
        if e.ends_with(')') {
            return eval_function(&e[..p], &e[p + 1..e.len() - 1]).map(|x| x.0);
        }
    }
    if e.eq_ignore_ascii_case("white") {
        return Some(Rgba::new(1.0, 1.0, 1.0, 1.0));
    }
    named_color(e)
}

fn split_top_level(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0;
    let mut cur = String::new();
    for ch in s.chars() {
        match ch {
            '(' => { depth += 1; cur.push(ch); }
            ')' => { depth -= 1; cur.push(ch); }
            c if (c.is_whitespace() || c == ',') && depth == 0 => {
                if !cur.is_empty() { out.push(std::mem::take(&mut cur)); }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() { out.push(cur); }
    out
}

pub static NAMED: &[(&str, u32)] = &[
    ("aliceblue", 0xf0f8ff), ("antiquewhite", 0xfaebd7), ("aqua", 0x00ffff), ("aquamarine", 0x7fffd4),
    ("azure", 0xf0ffff), ("beige", 0xf5f5dc), ("bisque", 0xffe4c4), ("black", 0x000000),
    ("blanchedalmond", 0xffebcd), ("blue", 0x0000ff), ("blueviolet", 0x8a2be2), ("brown", 0xa52a2a),
    ("burlywood", 0xdeb887), ("cadetblue", 0x5f9ea0), ("chartreuse", 0x7fff00), ("chocolate", 0xd2691e),
    ("coral", 0xff7f50), ("cornflowerblue", 0x6495ed), ("cornsilk", 0xfff8dc), ("crimson", 0xdc143c),
    ("cyan", 0x00ffff), ("darkblue", 0x00008b), ("darkcyan", 0x008b8b), ("darkgoldenrod", 0xb8860b),
    ("darkgray", 0xa9a9a9), ("darkgreen", 0x006400), ("darkgrey", 0xa9a9a9), ("darkkhaki", 0xbdb76b),
    ("darkmagenta", 0x8b008b), ("darkolivegreen", 0x556b2f), ("darkorange", 0xff8c00), ("darkorchid", 0x9932cc),
    ("darkred", 0x8b0000), ("darksalmon", 0xe9967a), ("darkseagreen", 0x8fbc8f), ("darkslateblue", 0x483d8b),
    ("darkslategray", 0x2f4f4f), ("darkslategrey", 0x2f4f4f), ("darkturquoise", 0x00ced1), ("darkviolet", 0x9400d3),
    ("deeppink", 0xff1493), ("deepskyblue", 0x00bfff), ("dimgray", 0x696969), ("dimgrey", 0x696969),
    ("dodgerblue", 0x1e90ff), ("firebrick", 0xb22222), ("floralwhite", 0xfffaf0), ("forestgreen", 0x228b22),
    ("fuchsia", 0xff00ff), ("gainsboro", 0xdcdcdc), ("ghostwhite", 0xf8f8ff), ("gold", 0xffd700),
    ("goldenrod", 0xdaa520), ("gray", 0x808080), ("green", 0x008000), ("greenyellow", 0xadff2f),
    ("grey", 0x808080), ("honeydew", 0xf0fff0), ("hotpink", 0xff69b4), ("indianred", 0xcd5c5c),
    ("indigo", 0x4b0082), ("ivory", 0xfffff0), ("khaki", 0xf0e68c), ("lavender", 0xe6e6fa),
    ("lavenderblush", 0xfff0f5), ("lawngreen", 0x7cfc00), ("lemonchiffon", 0xfffacd), ("lightblue", 0xadd8e6),
    ("lightcoral", 0xf08080), ("lightcyan", 0xe0ffff), ("lightgoldenrodyellow", 0xfafad2), ("lightgray", 0xd3d3d3),
    ("lightgreen", 0x90ee90), ("lightgrey", 0xd3d3d3), ("lightpink", 0xffb6c1), ("lightsalmon", 0xffa07a),
    ("lightseagreen", 0x20b2aa), ("lightskyblue", 0x87cefa), ("lightslategray", 0x778899), ("lightslategrey", 0x778899),
    ("lightsteelblue", 0xb0c4de), ("lightyellow", 0xffffe0), ("lime", 0x00ff00), ("limegreen", 0x32cd32),
    ("linen", 0xfaf0e6), ("magenta", 0xff00ff), ("maroon", 0x800000), ("mediumaquamarine", 0x66cdaa),
    ("mediumblue", 0x0000cd), ("mediumorchid", 0xba55d3), ("mediumpurple", 0x9370db), ("mediumseagreen", 0x3cb371),
    ("mediumslateblue", 0x7b68ee), ("mediumspringgreen", 0x00fa9a), ("mediumturquoise", 0x48d1cc), ("mediumvioletred", 0xc71585),
    ("midnightblue", 0x191970), ("mintcream", 0xf5fffa), ("mistyrose", 0xffe4e1), ("moccasin", 0xffe4b5),
    ("navajowhite", 0xffdead), ("navy", 0x000080), ("oldlace", 0xfdf5e6), ("olive", 0x808000),
    ("olivedrab", 0x6b8e23), ("orange", 0xffa500), ("orangered", 0xff4500), ("orchid", 0xda70d6),
    ("palegoldenrod", 0xeee8aa), ("palegreen", 0x98fb98), ("paleturquoise", 0xafeeee), ("palevioletred", 0xdb7093),
    ("papayawhip", 0xffefd5), ("peachpuff", 0xffdab9), ("peru", 0xcd853f), ("pink", 0xffc0cb),
    ("plum", 0xdda0dd), ("powderblue", 0xb0e0e6), ("purple", 0x800080), ("rebeccapurple", 0x663399),
    ("red", 0xff0000), ("rosybrown", 0xbc8f8f), ("royalblue", 0x4169e1), ("saddlebrown", 0x8b4513),
    ("salmon", 0xfa8072), ("sandybrown", 0xf4a460), ("seagreen", 0x2e8b57), ("seashell", 0xfff5ee),
    ("sienna", 0xa0522d), ("silver", 0xc0c0c0), ("skyblue", 0x87ceeb), ("slateblue", 0x6a5acd),
    ("slategray", 0x708090), ("slategrey", 0x708090), ("snow", 0xfffafa), ("springgreen", 0x00ff7f),
    ("steelblue", 0x4682b4), ("tan", 0xd2b48c), ("teal", 0x008080), ("thistle", 0xd8bfd8),
    ("tomato", 0xff6347), ("turquoise", 0x40e0d0), ("violet", 0xee82ee), ("wheat", 0xf5deb3),
    ("white", 0xffffff), ("whitesmoke", 0xf5f5f5), ("yellow", 0xffff00), ("yellowgreen", 0x9acd32),
];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn finds_many_kinds() {
        let t = "a { color: #ff0000; b: rgb(0 128 255 / 50%); c: hsl(120deg, 100%, 50%); d: tomato; e: 0xFF00FF; f: oklch(70% 0.1 200); }\nColor32::from_rgb(10, 20, 30); Color.FromArgb(255, 1, 2, 3); #include";
        let m = find_colors(t, true);
        let kinds: Vec<_> = m.iter().map(|x| &t[x.start..x.end]).collect();
        assert!(kinds.contains(&"#ff0000"));
        assert!(kinds.contains(&"rgb(0 128 255 / 50%)"));
        assert!(kinds.contains(&"hsl(120deg, 100%, 50%)"));
        assert!(kinds.contains(&"tomato"));
        assert!(kinds.contains(&"0xFF00FF"));
        assert!(kinds.contains(&"oklch(70% 0.1 200)"));
        assert!(kinds.contains(&"Color32::from_rgb(10, 20, 30)"));
        assert!(kinds.contains(&"Color.FromArgb(255, 1, 2, 3)"));
        assert!(!kinds.iter().any(|k| k.contains("include")));
    }
    #[test]
    fn scheme_expr() {
        let mut v = std::collections::HashMap::new();
        v.insert("blue".to_string(), "hsl(210, 50%, 60%)".to_string());
        let c = eval_scheme_color("color(var(blue) alpha(0.5))", &v, 0).unwrap();
        assert!((c.a - 0.5).abs() < 0.01);
        let w = eval_scheme_color("color(var(blue) blend(#000 50%))", &v, 0).unwrap();
        assert!(w.b < 0.5);
    }
}
