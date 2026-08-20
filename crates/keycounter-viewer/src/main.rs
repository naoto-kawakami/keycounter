use anyhow::{Context, Result};
use keycounter_common::{FILE_MAGIC, KEY_COUNT};
use std::{env, fs, path::Path};

#[derive(Clone, Copy)]
struct Key {
    vk: u8,
    label: &'static str,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

// Japanese JIS 109-key layout with navigation cluster and numpad.
const KEYS: &[Key] = &[
    // Function row
    Key{vk:0x1B,label:"Esc",x:0.0,y:0.0,w:1.0,h:1.0}, Key{vk:0x70,label:"F1",x:2.0,y:0.0,w:1.0,h:1.0}, Key{vk:0x71,label:"F2",x:3.0,y:0.0,w:1.0,h:1.0}, Key{vk:0x72,label:"F3",x:4.0,y:0.0,w:1.0,h:1.0}, Key{vk:0x73,label:"F4",x:5.0,y:0.0,w:1.0,h:1.0}, Key{vk:0x74,label:"F5",x:6.5,y:0.0,w:1.0,h:1.0}, Key{vk:0x75,label:"F6",x:7.5,y:0.0,w:1.0,h:1.0}, Key{vk:0x76,label:"F7",x:8.5,y:0.0,w:1.0,h:1.0}, Key{vk:0x77,label:"F8",x:9.5,y:0.0,w:1.0,h:1.0}, Key{vk:0x78,label:"F9",x:11.0,y:0.0,w:1.0,h:1.0}, Key{vk:0x79,label:"F10",x:12.0,y:0.0,w:1.0,h:1.0}, Key{vk:0x7A,label:"F11",x:13.0,y:0.0,w:1.0,h:1.0}, Key{vk:0x7B,label:"F12",x:14.0,y:0.0,w:1.0,h:1.0},
    // Number row
    Key{vk:0x19,label:"半角/全角",x:0.0,y:1.5,w:1.5,h:1.0}, Key{vk:0x31,label:"1",x:1.5,y:1.5,w:1.0,h:1.0}, Key{vk:0x32,label:"2",x:2.5,y:1.5,w:1.0,h:1.0}, Key{vk:0x33,label:"3",x:3.5,y:1.5,w:1.0,h:1.0}, Key{vk:0x34,label:"4",x:4.5,y:1.5,w:1.0,h:1.0}, Key{vk:0x35,label:"5",x:5.5,y:1.5,w:1.0,h:1.0}, Key{vk:0x36,label:"6",x:6.5,y:1.5,w:1.0,h:1.0}, Key{vk:0x37,label:"7",x:7.5,y:1.5,w:1.0,h:1.0}, Key{vk:0x38,label:"8",x:8.5,y:1.5,w:1.0,h:1.0}, Key{vk:0x39,label:"9",x:9.5,y:1.5,w:1.0,h:1.0}, Key{vk:0x30,label:"0",x:10.5,y:1.5,w:1.0,h:1.0}, Key{vk:0xBD,label:"-",x:11.5,y:1.5,w:1.0,h:1.0}, Key{vk:0xBB,label:"^",x:12.5,y:1.5,w:1.0,h:1.0}, Key{vk:0xDC,label:"\\",x:13.5,y:1.5,w:1.0,h:1.0}, Key{vk:0x08,label:"Backspace",x:14.5,y:1.5,w:1.5,h:1.0},
    // Main JIS rows
    Key{vk:0x51,label:"Q",x:1.0,y:2.5,w:1.0,h:1.0}, Key{vk:0x57,label:"W",x:2.0,y:2.5,w:1.0,h:1.0}, Key{vk:0x45,label:"E",x:3.0,y:2.5,w:1.0,h:1.0}, Key{vk:0x52,label:"R",x:4.0,y:2.5,w:1.0,h:1.0}, Key{vk:0x54,label:"T",x:5.0,y:2.5,w:1.0,h:1.0}, Key{vk:0x59,label:"Y",x:6.0,y:2.5,w:1.0,h:1.0}, Key{vk:0x55,label:"U",x:7.0,y:2.5,w:1.0,h:1.0}, Key{vk:0x49,label:"I",x:8.0,y:2.5,w:1.0,h:1.0}, Key{vk:0x4F,label:"O",x:9.0,y:2.5,w:1.0,h:1.0}, Key{vk:0x50,label:"P",x:10.0,y:2.5,w:1.0,h:1.0}, Key{vk:0xDB,label:"@",x:11.0,y:2.5,w:1.0,h:1.0}, Key{vk:0xC0,label:"[",x:12.0,y:2.5,w:1.0,h:1.0}, Key{vk:0x0D,label:"Enter",x:13.0,y:2.5,w:3.0,h:2.0},
    Key{vk:0x41,label:"A",x:1.5,y:3.5,w:1.0,h:1.0}, Key{vk:0x53,label:"S",x:2.5,y:3.5,w:1.0,h:1.0}, Key{vk:0x44,label:"D",x:3.5,y:3.5,w:1.0,h:1.0}, Key{vk:0x46,label:"F",x:4.5,y:3.5,w:1.0,h:1.0}, Key{vk:0x47,label:"G",x:5.5,y:3.5,w:1.0,h:1.0}, Key{vk:0x48,label:"H",x:6.5,y:3.5,w:1.0,h:1.0}, Key{vk:0x4A,label:"J",x:7.5,y:3.5,w:1.0,h:1.0}, Key{vk:0x4B,label:"K",x:8.5,y:3.5,w:1.0,h:1.0}, Key{vk:0x4C,label:"L",x:9.5,y:3.5,w:1.0,h:1.0}, Key{vk:0xBB,label:";",x:10.5,y:3.5,w:1.0,h:1.0}, Key{vk:0xBA,label:":",x:11.5,y:3.5,w:1.0,h:1.0},
    Key{vk:0x10,label:"Shift",x:0.0,y:4.5,w:2.5,h:1.0}, Key{vk:0x5A,label:"Z",x:2.5,y:4.5,w:1.0,h:1.0}, Key{vk:0x58,label:"X",x:3.5,y:4.5,w:1.0,h:1.0}, Key{vk:0x43,label:"C",x:4.5,y:4.5,w:1.0,h:1.0}, Key{vk:0x56,label:"V",x:5.5,y:4.5,w:1.0,h:1.0}, Key{vk:0x42,label:"B",x:6.5,y:4.5,w:1.0,h:1.0}, Key{vk:0x4E,label:"N",x:7.5,y:4.5,w:1.0,h:1.0}, Key{vk:0x4D,label:"M",x:8.5,y:4.5,w:1.0,h:1.0}, Key{vk:0xBC,label:",",x:9.5,y:4.5,w:1.0,h:1.0}, Key{vk:0xBE,label:".",x:10.5,y:4.5,w:1.0,h:1.0}, Key{vk:0xBF,label:"/",x:11.5,y:4.5,w:1.0,h:1.0}, Key{vk:0xE2,label:"\\_",x:12.5,y:4.5,w:1.0,h:1.0}, Key{vk:0x10,label:"Shift",x:13.5,y:4.5,w:2.5,h:1.0},
    Key{vk:0x11,label:"Ctrl",x:0.0,y:5.5,w:1.5,h:1.0}, Key{vk:0x5B,label:"Win",x:1.5,y:5.5,w:1.5,h:1.0}, Key{vk:0x12,label:"Alt",x:3.0,y:5.5,w:1.5,h:1.0}, Key{vk:0x1D,label:"無変換",x:4.5,y:5.5,w:1.5,h:1.0}, Key{vk:0x20,label:"Space",x:6.0,y:5.5,w:5.0,h:1.0}, Key{vk:0x1C,label:"変換",x:11.0,y:5.5,w:1.5,h:1.0}, Key{vk:0xF2,label:"ひらがな",x:12.5,y:5.5,w:1.5,h:1.0}, Key{vk:0x12,label:"Alt",x:14.0,y:5.5,w:1.5,h:1.0}, Key{vk:0x11,label:"Ctrl",x:15.5,y:5.5,w:1.5,h:1.0},
    // Navigation cluster
    Key{vk:0x2D,label:"Ins",x:17.5,y:1.5,w:1.0,h:1.0}, Key{vk:0x24,label:"Home",x:18.5,y:1.5,w:1.0,h:1.0}, Key{vk:0x21,label:"PgUp",x:19.5,y:1.5,w:1.0,h:1.0}, Key{vk:0x2E,label:"Del",x:17.5,y:2.5,w:1.0,h:1.0}, Key{vk:0x23,label:"End",x:18.5,y:2.5,w:1.0,h:1.0}, Key{vk:0x22,label:"PgDn",x:19.5,y:2.5,w:1.0,h:1.0}, Key{vk:0x26,label:"Up",x:18.5,y:4.0,w:1.0,h:1.0}, Key{vk:0x25,label:"Left",x:17.5,y:5.0,w:1.0,h:1.0}, Key{vk:0x28,label:"Down",x:18.5,y:5.0,w:1.0,h:1.0}, Key{vk:0x27,label:"Right",x:19.5,y:5.0,w:1.0,h:1.0},
    // Numeric keypad
    Key{vk:0x90,label:"Num",x:21.5,y:1.5,w:1.0,h:1.0}, Key{vk:0x6F,label:"/",x:22.5,y:1.5,w:1.0,h:1.0}, Key{vk:0x6A,label:"*",x:23.5,y:1.5,w:1.0,h:1.0}, Key{vk:0x6D,label:"-",x:24.5,y:1.5,w:1.0,h:1.0}, Key{vk:0x67,label:"7",x:21.5,y:2.5,w:1.0,h:1.0}, Key{vk:0x68,label:"8",x:22.5,y:2.5,w:1.0,h:1.0}, Key{vk:0x69,label:"9",x:23.5,y:2.5,w:1.0,h:1.0}, Key{vk:0x6B,label:"+",x:24.5,y:2.5,w:1.0,h:2.0}, Key{vk:0x64,label:"4",x:21.5,y:3.5,w:1.0,h:1.0}, Key{vk:0x65,label:"5",x:22.5,y:3.5,w:1.0,h:1.0}, Key{vk:0x66,label:"6",x:23.5,y:3.5,w:1.0,h:1.0}, Key{vk:0x61,label:"1",x:21.5,y:4.5,w:1.0,h:1.0}, Key{vk:0x62,label:"2",x:22.5,y:4.5,w:1.0,h:1.0}, Key{vk:0x63,label:"3",x:23.5,y:4.5,w:1.0,h:1.0}, Key{vk:0x0D,label:"Enter",x:24.5,y:4.5,w:1.0,h:2.0}, Key{vk:0x60,label:"0",x:21.5,y:5.5,w:2.0,h:1.0}, Key{vk:0x6E,label:".",x:23.5,y:5.5,w:1.0,h:1.0},
];

fn read_records(path: &Path) -> Result<Vec<[u32; KEY_COUNT]>> {
    let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    if bytes.len() < 12 || bytes[0..4] != FILE_MAGIC { anyhow::bail!("invalid .kbd file"); }
    let version = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    let key_count = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
    if version != 1 || key_count as usize != KEY_COUNT { anyhow::bail!("unsupported .kbd format"); }
    let record_size = KEY_COUNT * 4;
    if (bytes.len() - 12) % record_size != 0 { anyhow::bail!("truncated .kbd file"); }
    let mut out = Vec::new();
    for chunk in bytes[12..].chunks_exact(record_size) {
        let mut r = [0u32; KEY_COUNT];
        for i in 0..KEY_COUNT { r[i] = u32::from_le_bytes(chunk[i*4..i*4+4].try_into().unwrap()); }
        out.push(r);
    }
    Ok(out)
}

fn esc(s: &str) -> String { s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;") }

fn color(count: u32, max: u32) -> String {
    if count == 0 || max == 0 { return "#e7e7e7".into(); }
    // white -> red, using log scale so outliers don't dominate.
    let t = ((count as f64 + 1.0).ln() / (max as f64 + 1.0).ln()).clamp(0.0, 1.0);
    let r = 255u32;
    let g = (245.0 - 205.0 * t) as u32;
    let b = (245.0 - 205.0 * t) as u32;
    format!("#{:02x}{:02x}{:02x}", r, g, b)
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 || args.iter().any(|a| a == "--help") {
        eprintln!("Usage: keycounter-viewer <file.kbd> [output.svg] [--record N]");
        eprintln!("Default: aggregate all 10-minute records and write heatmap.svg");
        return Ok(());
    }
    let input = Path::new(&args[1]);
    let output = args.get(2).filter(|s| !s.starts_with("--")).map(Path::new).unwrap_or(Path::new("heatmap.svg"));
    let mut record_index: Option<usize> = None;
    for i in 0..args.len() { if args[i] == "--record" && i + 1 < args.len() { record_index = Some(args[i+1].parse()?); } }
    let records = read_records(input)?;
    if records.is_empty() { anyhow::bail!("no records in file"); }
    let mut counts = [0u64; KEY_COUNT];
    if let Some(i) = record_index {
        let r = records.get(i).context("record index out of range")?;
        for k in 0..KEY_COUNT { counts[k] = r[k] as u64; }
    } else {
        for r in &records { for k in 0..KEY_COUNT { counts[k] += r[k] as u64; } }
    }
    let max = counts.iter().copied().max().unwrap_or(0).min(u32::MAX as u64) as u32;
    let scale = 58.0;
    let margin = 30.0;
    let width = 26.5 * scale + margin * 2.0;
    let height = 7.5 * scale + margin * 2.0 + 40.0;
    let mut svg = format!(r###"<svg xmlns="http://www.w3.org/2000/svg" width="{width:.0}" height="{height:.0}" viewBox="0 0 {width:.0} {height:.0}"><rect width="100%" height="100%" fill="#111827"/><text x="30" y="28" fill="white" font-family="Segoe UI, sans-serif" font-size="18">Japanese JIS Full Keyboard Heatmap</text>"###);
    for key in KEYS {
        let c = counts[key.vk as usize];
        let fill = color(c.min(u32::MAX as u64) as u32, max);
        let x = margin + key.x * scale;
        let y = margin + 12.0 + key.y * scale;
        let w = key.w * scale - 3.0;
        let h = key.h * scale - 3.0;
        svg.push_str(&format!(r###"<rect x="{x:.1}" y="{y:.1}" width="{w:.1}" height="{h:.1}" rx="7" fill="{fill}" stroke="#374151"/><text x="{:.1}" y="{:.1}" text-anchor="middle" fill="#111827" font-family="Segoe UI, sans-serif" font-size="11">{}</text><text x="{:.1}" y="{:.1}" text-anchor="middle" fill="#374151" font-family="Consolas, monospace" font-size="9">{}</text>"###, x+w/2.0, y+h/2.0+3.0, esc(key.label), x+w/2.0, y+h-6.0, c));
    }
    svg.push_str(&format!(r###"<text x="30" y="{:.0}" fill="#d1d5db" font-family="Segoe UI, sans-serif" font-size="11">Records: {} × 10 min · Total keys: {} · Privacy: sequence/characters/timestamps not stored</text></svg>"###, height-12.0, records.len(), counts.iter().sum::<u64>()));
    fs::write(output, svg).with_context(|| format!("failed to write {}", output.display()))?;
    println!("Wrote {}", output.display());
    Ok(())
}
