//! Reads back the 1-bit raster a web app renders (1 = black, MSB first)
//! from the exact bytes sent: GoDEX EZPL `Q` bands or a ZPL `^GFA` field.
//! Anything else (hand-written commands, test labels) previews as text.
use base64::Engine;
use serde_json::{json, Value};

const MAX_BYTES: usize = 1_000_000;

pub struct Bitmap {
    pub width_dots: usize,
    pub height_dots: usize,
    pub width_bytes: usize,
    pub bytes: Vec<u8>,
}

pub fn language(payload: &[u8]) -> &'static str {
    let head = String::from_utf8_lossy(&payload[..payload.len().min(256)]);
    if head.contains("^XA") {
        "zpl"
    } else if head.lines().any(|l| matches!(l.split_whitespace().next(), Some("SIZE" | "CLS" | "GAP"))) {
        "tspl"
    } else if head.contains("^L") || head.contains("^Q") {
        "ezpl"
    } else {
        "raw"
    }
}

fn mm(text: &str, prefix: &str) -> Option<f64> {
    text.lines()
        .find_map(|l| l.trim().strip_prefix(prefix))
        .and_then(|v| v.split(',').next())
        .and_then(|v| v.trim().parse().ok())
}

pub fn decode_ezpl(payload: &[u8]) -> Option<Bitmap> {
    let start = payload.windows(3).position(|w| w == b"^L\r" || w == b"^L\n")? + 2;
    let mut at = start;
    while at < payload.len() && (payload[at] == b'\r' || payload[at] == b'\n') {
        at += 1;
    }
    let (mut bytes, mut width_bytes, mut row) = (Vec::new(), 0usize, 0usize);
    while payload.get(at) == Some(&b'Q') {
        let end = at + payload[at..].iter().take(80).position(|&b| b == b'\r')?;
        let header = std::str::from_utf8(&payload[at..end]).ok()?;
        let parts: Vec<usize> = header.strip_prefix("Q")?.split(',').map(|p| p.parse().ok()).collect::<Option<_>>()?;
        let [x, y, stride, height] = parts[..] else { return None };
        if x != 0 || y != row || stride == 0 || height == 0 || (width_bytes != 0 && stride != width_bytes) {
            return None;
        }
        width_bytes = stride;
        let data = end + 1;
        let len = stride * height;
        if data + len > payload.len() || bytes.len() + len > MAX_BYTES {
            return None;
        }
        bytes.extend_from_slice(&payload[data..data + len]);
        at = data + len;
        if payload.get(at) != Some(&b'\r') {
            return None;
        }
        at += 1;
        if payload.get(at) == Some(&b'\n') {
            at += 1;
        }
        row += height;
    }
    (row > 0).then(|| Bitmap { width_dots: width_bytes * 8, height_dots: row, width_bytes, bytes })
}

fn acs_count(c: u8) -> usize {
    match c {
        b'G'..=b'Y' => (c - b'F') as usize,
        b'g'..=b'z' => (c - b'f') as usize * 20,
        _ => 0,
    }
}

pub fn decode_zpl(payload: &[u8]) -> Option<Bitmap> {
    let text = std::str::from_utf8(payload).ok()?;
    let width_dots: usize = text.split("^PW").nth(1).map(|s| s.chars().take_while(char::is_ascii_digit).collect::<String>())?.parse().ok()?;
    let field = text.split("^GFA,").nth(1)?;
    let mut parts = field.splitn(4, ',');
    let total: usize = parts.next()?.parse().ok()?;
    parts.next()?;
    let width_bytes: usize = parts.next()?.parse().ok()?;
    let data = parts.next()?.split("^FS").next()?;
    if width_bytes == 0 || total % width_bytes != 0 || total > MAX_BYTES || width_dots.div_ceil(8) != width_bytes {
        return None;
    }
    let row_hex = width_bytes * 2;
    let (mut rows, mut row): (Vec<String>, String) = (Vec::new(), String::new());
    let data: Vec<u8> = data.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    let mut i = 0;
    while i < data.len() {
        match data[i] {
            b':' => rows.push(rows.last()?.clone()),
            c @ (b',' | b'!') => {
                let fill = if c == b',' { '0' } else { 'F' };
                if row.len() > row_hex {
                    return None;
                }
                let pad = row_hex - row.len();
                row.extend(std::iter::repeat_n(fill, pad));
                rows.push(std::mem::take(&mut row));
            }
            _ => {
                let mut n = 0;
                while i < data.len() && acs_count(data[i]) > 0 {
                    n += acs_count(data[i]);
                    i += 1;
                }
                let c = *data.get(i)? as char;
                if !c.is_ascii_hexdigit() {
                    return None;
                }
                row.extend(std::iter::repeat_n(c, n.max(1)));
                if row.len() > row_hex {
                    return None;
                }
                if row.len() == row_hex {
                    rows.push(std::mem::take(&mut row));
                }
            }
        }
        i += 1;
    }
    let height_dots = total / width_bytes;
    if !row.is_empty() || rows.len() != height_dots {
        return None;
    }
    let bytes = rows.iter().flat_map(|r| hex::decode(r).unwrap_or_default()).collect::<Vec<u8>>();
    (bytes.len() == total).then_some(Bitmap { width_dots, height_dots, width_bytes, bytes })
}

/// JSON the app renders: a bitmap (base64 rows) or the readable command text.
pub fn describe(payload: &[u8]) -> Value {
    let language = language(payload);
    let bitmap = match language {
        "zpl" => decode_zpl(payload),
        "ezpl" => decode_ezpl(payload),
        _ => None,
    };
    let text = String::from_utf8_lossy(payload);
    let size = if language == "ezpl" {
        json!({ "widthMm": mm(&text, "^W"), "heightMm": mm(&text, "^Q") })
    } else {
        json!({})
    };
    match bitmap {
        Some(b) => json!({
            "kind": "bitmap",
            "language": language,
            "widthDots": b.width_dots,
            "heightDots": b.height_dots,
            "widthBytes": b.width_bytes,
            "data": base64::engine::general_purpose::STANDARD.encode(&b.bytes),
            "media": size,
        }),
        None => {
            let printable: String = text.chars().take(8_000).map(|c| if c.is_control() && c != '\n' && c != '\r' { '·' } else { c }).collect();
            json!({ "kind": "text", "language": language, "text": printable.replace("\r\n", "\n").replace('\r', "\n"), "media": size })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_ezpl_bands() {
        let mut p = b"^Q25,3\r\n^W50\r\n^L\r\n".to_vec();
        p.extend(b"Q0,0,2,2\r");
        p.extend([0xff, 0x00, 0x0a, 0x0d]); // data may hold CR/LF bytes
        p.extend(b"\rQ0,2,2,1\r");
        p.extend([0x80, 0x01]);
        p.extend(b"\rE\r\n");
        let b = decode_ezpl(&p).unwrap();
        assert_eq!((b.width_dots, b.height_dots), (16, 3));
        assert_eq!(b.bytes, vec![0xff, 0x00, 0x0a, 0x0d, 0x80, 0x01]);
        let d = describe(&p);
        assert_eq!(d["kind"], "bitmap");
        assert_eq!(d["media"]["widthMm"], 50.0);
    }

    #[test]
    fn decodes_zpl_acs() {
        // 16 dots wide, 3 rows: "FF00", repeat, "8" then zero fill.
        let p = b"^XA\n^PW16\n^FO0,0^GFA,6,6,2,FF00:8,^FS\n^XZ\n";
        let b = decode_zpl(p).unwrap();
        assert_eq!(b.bytes, vec![0xff, 0x00, 0xff, 0x00, 0x80, 0x00]);
        // "J" repeats 4 times.
        let b = decode_zpl(b"^XA^PW16^GFA,2,2,2,JF^FS^XZ").unwrap();
        assert_eq!(b.bytes, vec![0xff, 0xff]);
    }

    #[test]
    fn text_commands_preview_as_text() {
        let d = describe(&crate::printers::test_payload("zpl"));
        assert_eq!((d["kind"].as_str(), d["language"].as_str()), (Some("text"), Some("zpl")));
        assert_eq!(describe(&crate::printers::test_payload("ezpl"))["language"], "ezpl");
        assert_eq!(describe(&crate::printers::test_payload("tspl"))["language"], "tspl");
        assert_eq!(describe(b"hello")["language"], "raw");
    }
}
