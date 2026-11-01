//! Dotline core: a localhost HTTP endpoint any web app can send raw label
//! jobs to (ZPL, EZPL, TSPL, ...), delivered through CUPS or raw TCP.
//! The Swift menu bar app drives it through the C ABI at the bottom.

pub mod config;
pub mod jobs;
pub mod preview;
pub mod printers;
pub mod server;

use serde_json::json;
use std::ffi::{c_char, CStr, CString};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

pub struct Shared {
    pub dir: PathBuf,
    pub config: config::Config,
    pub jobs: jobs::Ledger,
}

impl Shared {
    pub fn new(dir: &Path, config: config::Config, persist: bool) -> Self {
        let jobs = jobs::Ledger::open(persist.then(|| dir.join("jobs.json")));
        Shared { dir: dir.to_path_buf(), config, jobs }
    }
}

struct Agent {
    shared: Arc<Mutex<Shared>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

static AGENT: Mutex<Option<Agent>> = Mutex::new(None);

pub fn start(dir: &Path, port: u16) -> Result<(), String> {
    stop();
    let config = config::load(dir, port)?;
    let server = server::bind(config.port)?;
    let shared = Arc::new(Mutex::new(Shared::new(dir, config, true)));
    let stop = Arc::new(AtomicBool::new(false));
    let thread = {
        let (shared, stop) = (shared.clone(), stop.clone());
        std::thread::Builder::new()
            .name("dotline-http".into())
            .spawn(move || server::serve(server, shared, stop))
            .map_err(|e| e.to_string())?
    };
    *AGENT.lock().unwrap() = Some(Agent { shared, stop, thread: Some(thread) });
    Ok(())
}

pub fn stop() {
    let agent = AGENT.lock().unwrap().take();
    if let Some(mut agent) = agent {
        agent.stop.store(true, Ordering::Relaxed);
        if let Some(t) = agent.thread.take() {
            let _ = t.join();
        }
    }
}

fn with_shared<T>(f: impl FnOnce(&mut Shared) -> T) -> Option<T> {
    let agent = AGENT.lock().unwrap();
    let shared = agent.as_ref()?.shared.clone();
    drop(agent);
    let mut guard = shared.lock().unwrap();
    Some(f(&mut guard))
}

pub fn snapshot() -> serde_json::Value {
    let printers = printers::list();
    let state = with_shared(|s| (s.config.clone(), s.jobs.recent(500)));
    match state {
        None => json!({ "running": false }),
        Some((config, jobs)) => json!({
            "running": true,
            "port": config.port,
            "token": config.token,
            "printers": printers.as_ref().ok(),
            "printersError": printers.as_ref().err(),
            "jobs": jobs,
        }),
    }
}

pub fn test_print(printer: &str, language: &str) -> serde_json::Value {
    let id = format!("test-{}", config::new_token());
    let payload = printers::test_payload(language);
    let result = printers::send(printer, "Dotline test", &payload);
    let job = jobs::Job {
        id,
        printer: printer.into(),
        payload_hash: server::sha256_hex(&payload),
        bytes: payload.len(),
        state: if result.is_ok() { "sent" } else { "failed" }.into(),
        detail: result.as_ref().err().cloned(),
        request_id: result.as_ref().ok().cloned(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
    };
    let value = serde_json::to_value(&job).unwrap();
    with_shared(|s| s.jobs.record_with_payload(job, &payload));
    value
}

// ---- C ABI (see include/nwpa.h) -------------------------------------------

fn out(value: serde_json::Value) -> *mut c_char {
    CString::new(value.to_string()).unwrap().into_raw()
}

unsafe fn arg(ptr: *const c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(ptr) }.to_string_lossy().into_owned()
}

/// Starts (or restarts) the server. `port` 0 keeps the stored port.
/// Returns JSON `{"ok":true}` or `{"ok":false,"error":"..."}`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dotline_start(data_dir: *const c_char, port: u16) -> *mut c_char {
    let dir = unsafe { arg(data_dir) };
    out(match start(Path::new(&dir), port) {
        Ok(()) => json!({ "ok": true }),
        Err(e) => json!({ "ok": false, "error": e }),
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn dotline_stop() {
    stop();
}

/// JSON: running, port, token, printers, recent jobs (newest first).
#[unsafe(no_mangle)]
pub extern "C" fn dotline_snapshot() -> *mut c_char {
    out(snapshot())
}

pub fn rotate_token() -> serde_json::Value {
    with_shared(|s| {
        s.config.token = config::new_token();
        config::save(&s.dir, &s.config).map(|_| s.config.token.clone())
    })
    .map_or(json!({ "ok": false, "error": "notRunning" }), |r| match r {
        Ok(token) => json!({ "ok": true, "token": token }),
        Err(e) => json!({ "ok": false, "error": e }),
    })
}

/// Issues a new access token; every connected web app must update it.
#[unsafe(no_mangle)]
pub extern "C" fn dotline_rotate_token() -> *mut c_char {
    out(rotate_token())
}

/// Prints a small test label. `language` is "zpl", "ezpl" or "tspl".
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dotline_test_print(printer: *const c_char, language: *const c_char) -> *mut c_char {
    let (printer, language) = unsafe { (arg(printer), arg(language)) };
    out(test_print(&printer, &language))
}

/// Preview of a recorded job's exact bytes: `{kind:"bitmap"|"text", ...}`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dotline_job_preview(id: *const c_char) -> *mut c_char {
    let id = unsafe { arg(id) };
    let payload = with_shared(|s| s.jobs.get(&id).and_then(|j| s.jobs.payload(&j.payload_hash))).flatten();
    out(payload.map_or(json!({ "kind": "none" }), |p| preview::describe(&p)))
}

/// Preview of a label file on disk (.prn/.zpl/.ezpl).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dotline_file_preview(path: *const c_char) -> *mut c_char {
    let path = unsafe { arg(path) };
    out(match std::fs::metadata(&path) {
        Ok(m) if m.len() > 8 * 1024 * 1024 => json!({ "kind": "none", "error": "fileTooLarge" }),
        _ => match std::fs::read(&path) {
            Ok(p) => preview::describe(&p),
            Err(e) => json!({ "kind": "none", "error": e.to_string() }),
        },
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn dotline_clear_jobs() {
    with_shared(|s| s.jobs.clear());
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn dotline_string_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        drop(unsafe { CString::from_raw(ptr) });
    }
}
