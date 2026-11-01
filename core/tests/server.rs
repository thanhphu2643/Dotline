use base64::Engine;
use dotline_core::config::Config;
use dotline_core::server::{handle, sha256_hex, Request};
use dotline_core::Shared;
use serde_json::json;
use std::net::TcpListener;
use std::sync::Mutex;

fn shared() -> Mutex<Shared> {
    let dir = tempfile::tempdir().unwrap();
    Mutex::new(Shared::new(dir.path(), Config { port: 1, token: "secret".into() }, false))
}

fn call(s: &Mutex<Shared>, method: &str, path: &str, auth: Option<&str>, body: serde_json::Value) -> (u16, serde_json::Value) {
    let body = if body.is_null() { Vec::new() } else { body.to_string().into_bytes() };
    handle(s, &Request { method, path, authorization: auth, body: &body })
}

/// A local socket standing in for a 9100 network printer; returns its target and received bytes.
fn fake_printer(labels: usize) -> (String, std::thread::JoinHandle<Vec<Vec<u8>>>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let target = format!("tcp://{}", l.local_addr().unwrap());
    let h = std::thread::spawn(move || {
        (0..labels)
            .map(|_| {
                let (mut s, _) = l.accept().unwrap();
                let mut b = Vec::new();
                std::io::Read::read_to_end(&mut s, &mut b).unwrap();
                b
            })
            .collect()
    });
    (target, h)
}

const AUTH: Option<&str> = Some("Bearer secret");

#[test]
fn health_needs_no_token_everything_else_does() {
    let s = shared();
    assert_eq!(call(&s, "GET", "/health", None, json!(null)).0, 200);
    assert_eq!(call(&s, "GET", "/jobs/x", None, json!(null)), (401, json!({"error":"unauthorized"})));
    assert_eq!(call(&s, "GET", "/jobs/x", Some("Bearer wrong"), json!(null)).0, 401);
    assert_eq!(call(&s, "GET", "/jobs/x", AUTH, json!(null)).0, 404);
}

#[test]
fn prints_once_per_job_id() {
    let s = shared();
    let (target, received) = fake_printer(1);
    let payload = b"^XA^FDhi^FS^XZ";
    let job = json!({
        "id": "job-1", "printer": target,
        "payloadBase64": base64::engine::general_purpose::STANDARD.encode(payload),
        "payloadHash": sha256_hex(payload),
    });
    let (status, first) = call(&s, "POST", "/jobs", AUTH, job.clone());
    assert_eq!(status, 200, "{first}");
    assert_eq!(first["state"], "sent");
    // Retry of the same job: answered from the ledger, nothing reprinted.
    assert_eq!(call(&s, "POST", "/jobs", AUTH, job).1, first);
    assert_eq!(call(&s, "GET", "/jobs/job-1", AUTH, json!(null)).1["payloadHash"], sha256_hex(payload));
    assert_eq!(received.join().unwrap(), vec![payload.to_vec()]);
}

#[test]
fn rejects_mismatched_hash_and_reused_id() {
    let s = shared();
    let (target, received) = fake_printer(1);
    let b64 = |p: &[u8]| base64::engine::general_purpose::STANDARD.encode(p);
    let bad = json!({ "id": "j", "printer": target, "payloadBase64": b64(b"A"), "payloadHash": sha256_hex(b"B") });
    assert_eq!(call(&s, "POST", "/jobs", AUTH, bad).1["error"], "payloadHashMismatch");
    let ok = json!({ "id": "j", "printer": target, "payloadBase64": b64(b"A") });
    assert_eq!(call(&s, "POST", "/jobs", AUTH, ok).0, 200);
    let other = json!({ "id": "j", "printer": target, "payloadBase64": b64(b"C") });
    assert_eq!(call(&s, "POST", "/jobs", AUTH, other), (409, json!({"error":"jobIdConflict"})));
    received.join().unwrap();
}

#[test]
fn failed_delivery_is_recorded_and_retryable() {
    let s = shared();
    // Bind then drop: nothing listens on this port.
    let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let job = json!({ "id": "f", "printer": format!("tcp://127.0.0.1:{port}"), "payloadBase64": "QQ==" });
    let (status, body) = call(&s, "POST", "/jobs", AUTH, job);
    assert_eq!(status, 502);
    assert_eq!(body["state"], "failed");
    assert_eq!(call(&s, "GET", "/jobs/f", AUTH, json!(null)).1["state"], "failed");
}

#[test]
fn unknown_cups_queue_is_refused() {
    let s = shared();
    let job = json!({ "id": "q", "printer": "No_Such_Queue_dotline", "payloadBase64": "QQ==" });
    assert_eq!(call(&s, "POST", "/jobs", AUTH, job), (404, json!({"error":"printerNotFound"})));
}
