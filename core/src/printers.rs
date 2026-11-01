//! Raw delivery: CUPS queues through `lp -o raw` (macOS/Linux), the Windows
//! spooler with the RAW datatype, or `tcp://host:port`
//! (port 9100 on most network label printers) straight to the socket.
use serde::Serialize;
use std::io::Write;
use std::net::{TcpStream, ToSocketAddrs};
#[cfg(not(windows))]
use std::process::{Command, Stdio};
use std::time::Duration;

#[derive(Clone, Serialize, PartialEq, Debug)]
pub struct Printer {
    pub name: String,
    /// idle | printing | disabled | unknown
    pub state: String,
    #[serde(rename = "isDefault")]
    pub is_default: bool,
}

#[cfg(not(windows))]
fn lpstat(args: &[&str]) -> Result<String, String> {
    let out = Command::new("/usr/bin/lpstat")
        .args(args)
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .output()
        .map_err(|e| format!("lpstat: {e}"))?;
    // "No destinations added" exits non-zero; an empty list is not an error.
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

pub fn parse_lpstat(printers: &str, default: &str) -> Vec<Printer> {
    let default = default
        .lines()
        .find_map(|l| l.strip_prefix("system default destination: "))
        .map(str::trim);
    let mut list: Vec<Printer> = printers
        .lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("printer ")?;
            let (name, status) = rest.split_once(' ')?;
            let state = if status.contains("disabled") {
                "disabled"
            } else if status.contains("printing") {
                "printing"
            } else if status.contains("idle") {
                "idle"
            } else {
                "unknown"
            };
            Some(Printer { name: name.to_string(), state: state.into(), is_default: Some(name) == default })
        })
        .collect();
    list.sort_by(|a, b| a.name.cmp(&b.name));
    list
}

#[cfg(not(windows))]
pub fn list() -> Result<Vec<Printer>, String> {
    Ok(parse_lpstat(&lpstat(&["-p"])?, &lpstat(&["-d"])?))
}

#[cfg(windows)]
pub fn list() -> Result<Vec<Printer>, String> {
    winspool::list()
}

/// Exact name match only: a fuzzy match could send a regular label to a
/// similarly named folded/spare printer.
pub fn exists(target: &str) -> Result<bool, String> {
    if target.starts_with("tcp://") {
        return Ok(true);
    }
    Ok(list()?.iter().any(|p| p.name == target))
}

/// Delivers raw bytes; returns the CUPS request id (or the socket target).
pub fn send(target: &str, title: &str, payload: &[u8]) -> Result<String, String> {
    if let Some(addr) = target.strip_prefix("tcp://") {
        return send_tcp(addr, payload).map(|_| target.to_string());
    }
    #[cfg(windows)]
    return winspool::send(target, title, payload);
    #[cfg(not(windows))]
    send_cups(target, title, payload)
}

#[cfg(not(windows))]
fn send_cups(target: &str, title: &str, payload: &[u8]) -> Result<String, String> {
    let mut child = Command::new("/usr/bin/lp")
        .args(["-d", target, "-o", "raw", "-t", title, "-"])
        .env("LC_ALL", "C")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("lp: {e}"))?;
    child.stdin.take().unwrap().write_all(payload).map_err(|e| format!("lp: {e}"))?;
    let out = child.wait_with_output().map_err(|e| format!("lp: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(err.trim().trim_start_matches("lp: ").to_string());
    }
    // "request id is Zebra-42 (1 file(s))"
    Ok(stdout
        .split_whitespace()
        .skip_while(|w| *w != "is")
        .nth(1)
        .unwrap_or_default()
        .to_string())
}

fn send_tcp(addr: &str, payload: &[u8]) -> Result<(), String> {
    let sock = addr
        .to_socket_addrs()
        .map_err(|e| format!("{addr}: {e}"))?
        .next()
        .ok_or_else(|| format!("{addr}: no address"))?;
    let mut stream = TcpStream::connect_timeout(&sock, Duration::from_secs(5)).map_err(|e| format!("{addr}: {e}"))?;
    stream.set_write_timeout(Some(Duration::from_secs(20))).ok();
    stream.write_all(payload).and_then(|_| stream.flush()).map_err(|e| format!("{addr}: {e}"))
}

/// Windows print spooler, RAW datatype: bytes reach the printer untouched.
#[cfg(windows)]
mod winspool {
    use super::Printer;
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::Foundation::GetLastError;
    use windows_sys::Win32::Graphics::Printing::*;

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    unsafe fn from_wide(p: *const u16) -> String {
        if p.is_null() {
            return String::new();
        }
        let len = (0..).take_while(|&i| unsafe { *p.add(i) } != 0).count();
        String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(p, len) })
    }

    fn default_printer() -> Option<String> {
        let mut len = 0u32;
        unsafe { GetDefaultPrinterW(null_mut(), &mut len) };
        if len == 0 {
            return None;
        }
        let mut buf = vec![0u16; len as usize];
        (unsafe { GetDefaultPrinterW(buf.as_mut_ptr(), &mut len) } != 0).then(|| unsafe { from_wide(buf.as_ptr()) })
    }

    pub fn list() -> Result<Vec<Printer>, String> {
        let flags = PRINTER_ENUM_LOCAL | PRINTER_ENUM_CONNECTIONS;
        let (mut needed, mut count) = (0u32, 0u32);
        unsafe { EnumPrintersW(flags, null(), 2, null_mut(), 0, &mut needed, &mut count) };
        if needed == 0 {
            return Ok(Vec::new());
        }
        // u64 backing keeps PRINTER_INFO_2W pointer fields aligned.
        let mut buf = vec![0u64; (needed as usize).div_ceil(8)];
        if unsafe { EnumPrintersW(flags, null(), 2, buf.as_mut_ptr().cast(), needed, &mut needed, &mut count) } == 0 {
            return Err(format!("EnumPrinters failed ({})", unsafe { GetLastError() }));
        }
        let default = default_printer();
        let infos = unsafe { std::slice::from_raw_parts(buf.as_ptr().cast::<PRINTER_INFO_2W>(), count as usize) };
        let mut list: Vec<Printer> = infos
            .iter()
            .map(|info| {
                let name = unsafe { from_wide(info.pPrinterName) };
                let state = if info.Status & PRINTER_STATUS_PAUSED != 0 || info.Status & PRINTER_STATUS_OFFLINE != 0 {
                    "disabled"
                } else if info.Status & PRINTER_STATUS_PRINTING != 0 {
                    "printing"
                } else {
                    "idle"
                };
                Printer { is_default: default.as_deref() == Some(name.as_str()), name, state: state.into() }
            })
            .collect();
        list.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(list)
    }

    pub fn send(target: &str, title: &str, payload: &[u8]) -> Result<String, String> {
        let err = |what: &str| format!("{what} failed ({})", unsafe { GetLastError() });
        let name = wide(target);
        let mut handle: PRINTER_HANDLE = unsafe { std::mem::zeroed() };
        if unsafe { OpenPrinterW(name.as_ptr(), &mut handle, null()) } == 0 {
            return Err(err("OpenPrinter"));
        }
        let (mut doc_name, mut datatype) = (wide(title), wide("RAW"));
        let doc = DOC_INFO_1W { pDocName: doc_name.as_mut_ptr(), pOutputFile: null_mut(), pDatatype: datatype.as_mut_ptr() };
        let result = (|| {
            let job = unsafe { StartDocPrinterW(handle, 1, &doc) };
            if job == 0 {
                return Err(err("StartDocPrinter"));
            }
            let mut written = 0u32;
            let ok = unsafe { StartPagePrinter(handle) } != 0
                && unsafe { WritePrinter(handle, payload.as_ptr().cast(), payload.len() as u32, &mut written) } != 0
                && written as usize == payload.len();
            let write_err = (!ok).then(|| err("WritePrinter"));
            unsafe {
                EndPagePrinter(handle);
                EndDocPrinter(handle);
            }
            write_err.map_or(Ok(format!("{target}-{job}")), Err)
        })();
        unsafe { ClosePrinter(handle) };
        result
    }
}

pub fn test_payload(language: &str) -> Vec<u8> {
    match language {
        "zpl" => b"^XA^CI28^FO40,40^A0N,36,36^FDDotline^FS^FO40,90^A0N,28,28^FDTest OK^FS^XZ".to_vec(),
        // TSC and most low-cost label printers.
        "tspl" => [
            "SIZE 50 mm,30 mm", "GAP 3 mm,0 mm", "DIRECTION 1", "CLS",
            "TEXT 30,30,\"3\",0,1,1,\"Dotline\"", "TEXT 30,80,\"2\",0,1,1,\"Test OK\"", "PRINT 1,1", "",
        ]
        .join("\r\n")
        .into_bytes(),
        // GoDEX EZPL, CRLF line endings.
        _ => [
            "^Q25,3", "^W50", "^H10", "^P1", "^S3", "^AD", "^C1", "^R0", "~Q+0", "^O0", "^D0", "^E18", "~R255", "^L",
            "AC,30,30,1,1,0,0,Dotline", "AB,30,80,1,1,0,0,Test OK", "E", "",
        ]
        .join("\r\n")
        .into_bytes(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lpstat() {
        let p = "printer Zebra_ZD421 is idle.  enabled since Mon\n\tReady\nprinter GoDEX_RT730i disabled since Tue -\n\tPaused\nprinter Brother now printing Brother-3.  enabled since x\n";
        let list = parse_lpstat(p, "system default destination: GoDEX_RT730i\n");
        assert_eq!(list.len(), 3);
        assert_eq!(list[0], Printer { name: "Brother".into(), state: "printing".into(), is_default: false });
        assert_eq!(list[1].state, "disabled");
        assert!(list[1].is_default);
        assert_eq!(list[2].state, "idle");
    }

    #[test]
    fn empty_lpstat() {
        assert!(parse_lpstat("", "no system default destination\n").is_empty());
    }

    #[test]
    fn tcp_target_delivers_bytes() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let reader = std::thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            let mut buf = Vec::new();
            std::io::Read::read_to_end(&mut s, &mut buf).unwrap();
            buf
        });
        send(&format!("tcp://{addr}"), "t", b"^XA^XZ").unwrap();
        assert_eq!(reader.join().unwrap(), b"^XA^XZ");
    }
}
