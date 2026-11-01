use crate::jobs::Job;
use crate::{printers, Shared};
use base64::Engine;
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use subtle::ConstantTimeEq;
use tiny_http::{Header, Method, Response, Server};

const MAX_BODY: u64 = 8 * 1024 * 1024;
pub const APP: &str = "dotline";

pub struct Request<'a> {
    pub method: &'a str,
    pub path: &'a str,
    pub authorization: Option<&'a str>,
    pub body: &'a [u8],
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Submit {
    id: String,
    printer: String,
    payload_base64: String,
    payload_hash: Option<String>,
    title: Option<String>,
}

fn err(status: u16, code: &str) -> (u16, Value) {
    (status, json!({ "error": code }))
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// Routes one request. Pure apart from printing, so tests drive it directly.
pub fn handle(shared: &Mutex<Shared>, req: &Request) -> (u16, Value) {
    if req.method == "GET" && req.path == "/health" {
        return (200, json!({ "app": APP, "version": env!("CARGO_PKG_VERSION") }));
    }
    let token = shared.lock().unwrap().config.token.clone();
    let presented = req.authorization.and_then(|h| h.strip_prefix("Bearer ")).unwrap_or("");
    if !bool::from(presented.as_bytes().ct_eq(token.as_bytes())) {
        return err(401, "unauthorized");
    }
    match (req.method, req.path) {
        ("GET", "/printers") => match printers::list() {
            Ok(list) => (200, json!({ "printers": list })),
            Err(e) => (503, json!({ "error": "printersUnavailable", "detail": e })),
        },
        ("POST", "/jobs") => submit(shared, req.body),
        ("GET", path) if path.starts_with("/jobs/") => {
            let id = &path["/jobs/".len()..];
            match shared.lock().unwrap().jobs.get(id) {
                Some(job) => (200, serde_json::to_value(job).unwrap()),
                None => err(404, "jobNotFound"),
            }
        }
        _ => err(404, "routeNotFound"),
    }
}

fn submit(shared: &Mutex<Shared>, body: &[u8]) -> (u16, Value) {
    let Ok(job) = serde_json::from_slice::<Submit>(body) else { return err(400, "invalidJob") };
    if job.id.is_empty() || job.id.len() > 200 || job.printer.is_empty() {
        return err(400, "invalidJob");
    }
    let Ok(payload) = base64::engine::general_purpose::STANDARD.decode(job.payload_base64.as_bytes()) else {
        return err(400, "invalidPayload");
    };
    let hash = sha256_hex(&payload);
    if job.payload_hash.as_deref().is_some_and(|h| !h.eq_ignore_ascii_case(&hash)) {
        return err(400, "payloadHashMismatch");
    }
    // Idempotent by job id: a retried request never prints a second label.
    if let Some(prev) = shared.lock().unwrap().jobs.get(&job.id) {
        if prev.payload_hash != hash {
            return err(409, "jobIdConflict");
        }
        if prev.state != "failed" {
            return (200, serde_json::to_value(prev).unwrap());
        }
    }
    match printers::exists(&job.printer) {
        Ok(true) => {}
        Ok(false) => return err(404, "printerNotFound"),
        Err(e) => return (503, json!({ "error": "printersUnavailable", "detail": e })),
    }
    let title = job.title.unwrap_or_else(|| format!("Dotline job {}", job.id));
    let result = printers::send(&job.printer, &title, &payload);
    let record = Job {
        id: job.id,
        printer: job.printer,
        payload_hash: hash,
        bytes: payload.len(),
        state: if result.is_ok() { "sent" } else { "failed" }.into(),
        detail: result.as_ref().err().cloned(),
        request_id: result.ok(),
        created_at: now(),
    };
    let status = if record.state == "sent" { 200 } else { 502 };
    let value = serde_json::to_value(&record).unwrap();
    shared.lock().unwrap().jobs.record_with_payload(record, &payload);
    (status, value)
}

fn cors(origin: Option<String>) -> Vec<Header> {
    let h = |k: &str, v: &str| Header::from_bytes(k.as_bytes(), v.as_bytes()).unwrap();
    vec![
        h("Access-Control-Allow-Origin", origin.as_deref().unwrap_or("*")),
        h("Vary", "Origin"),
        h("Access-Control-Allow-Methods", "GET, POST, OPTIONS"),
        h("Access-Control-Allow-Headers", "Authorization, Content-Type"),
        // Chrome Private Network Access: lets an https page reach 127.0.0.1.
        h("Access-Control-Allow-Private-Network", "true"),
        h("Access-Control-Max-Age", "600"),
    ]
}

pub fn bind(port: u16) -> Result<Server, String> {
    Server::http(("127.0.0.1", port)).map_err(|e| format!("port {port}: {e}"))
}

pub fn serve(server: Server, shared: Arc<Mutex<Shared>>, stop: Arc<AtomicBool>) {
    // One request at a time keeps labels in the order the caller sent them.
    while !stop.load(Ordering::Relaxed) {
        let Ok(Some(mut request)) = server.recv_timeout(Duration::from_millis(250)) else { continue };
        let header = |name: &'static str| {
            request.headers().iter().find(|h| h.field.equiv(name)).map(|h| h.value.as_str().to_string())
        };
        let origin = header("Origin");
        let authorization = header("Authorization");
        let (status, value) = if *request.method() == Method::Options {
            (204, Value::Null)
        } else {
            let mut body = Vec::new();
            let read = request.as_reader().take(MAX_BODY + 1).read_to_end(&mut body);
            if read.is_err() || body.len() as u64 > MAX_BODY {
                err(413, "payloadTooLarge")
            } else {
                let method = request.method().as_str().to_string();
                let path = request.url().split('?').next().unwrap_or("").to_string();
                handle(&shared, &Request { method: &method, path: &path, authorization: authorization.as_deref(), body: &body })
            }
        };
        let body = if value.is_null() { String::new() } else { value.to_string() };
        let mut response = Response::from_string(body).with_status_code(status);
        for h in cors(origin) {
            response.add_header(h);
        }
        response.add_header(Header::from_bytes("Content-Type", "application/json").unwrap());
        let _ = request.respond(response);
    }
}
