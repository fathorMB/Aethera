//! Endpoint di Aethera su 127.0.0.1:8090 per i client che vogliono dichiararsi: stato, manifest
//! dell'avvio, lock con TTL, telemetria recente. Il lock non tocca il motore: lo rende «in uso», e un
//! motore in uso rifiuta arresto e riavvio.
//!
//! HTTP/1.1 minimo, una connessione per richiesta. Le richieste con `Origin` vengono da un browser
//! e si rifiutano: l'endpoint è per processi locali.

use crate::engine::{Engine, EngineStatus, LockRequest};
use crate::services::Services;
use crate::system;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::time::Duration;

pub const DEFAULT_ADDR: &str = "127.0.0.1:8090";
const MAX_HEAD: usize = 16 * 1024;
const MAX_BODY: usize = 64 * 1024;

pub fn spawn(engine: Engine, services: Services, addr: &str) -> Result<SocketAddr, String> {
    let listener = TcpListener::bind(addr).map_err(|e| format!("endpoint {addr}: {e}"))?;
    let local = listener.local_addr().map_err(|e| e.to_string())?;
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let (engine, services) = (engine.clone(), services.clone());
            std::thread::spawn(move || handle(&engine, &services, stream));
        }
    });
    Ok(local)
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        413 => "Payload Too Large",
        _ => "Error",
    }
}

fn handle(engine: &Engine, services: &Services, mut stream: TcpStream) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let (status, body) = match read_request(&mut stream) {
        Ok(req) => route(engine, services, &req.method, &req.target, req.origin, &req.body),
        Err((status, msg)) => (status, json!({ "error": msg })),
    };
    let text = serde_json::to_string_pretty(&body).unwrap_or_default();
    let head = format!(
        "HTTP/1.1 {status} {}\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        reason(status),
        text.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(text.as_bytes());
}

struct Request {
    method: String,
    target: String,
    origin: bool,
    body: Vec<u8>,
}

fn read_request(stream: &mut TcpStream) -> Result<Request, (u16, String)> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i;
        }
        if buf.len() > MAX_HEAD {
            return Err((413, "intestazioni troppo lunghe".into()));
        }
        let n = stream.read(&mut chunk).map_err(|e| (400, e.to_string()))?;
        if n == 0 {
            return Err((400, "richiesta incompleta".into()));
        }
        buf.extend_from_slice(&chunk[..n]);
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
    let mut lines = head.split("\r\n");
    let mut first = lines.next().unwrap_or_default().split_whitespace();
    let method = first.next().unwrap_or_default().to_string();
    let target = first.next().unwrap_or_default().to_string();
    let mut length = 0usize;
    let mut origin = false;
    for line in lines {
        let Some((k, v)) = line.split_once(':') else { continue };
        match k.trim().to_ascii_lowercase().as_str() {
            "content-length" => length = v.trim().parse().map_err(|_| (400, "Content-Length non valido".to_string()))?,
            "origin" => origin = true,
            _ => {}
        }
    }
    if length > MAX_BODY {
        return Err((413, "corpo troppo lungo".into()));
    }
    let mut body = buf[head_end + 4..].to_vec();
    while body.len() < length {
        let n = stream.read(&mut chunk).map_err(|e| (400, e.to_string()))?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..n]);
    }
    body.truncate(length);
    Ok(Request { method, target, origin, body })
}

fn query_param(query: &str, key: &str) -> Option<String> {
    query.split('&').filter_map(|kv| kv.split_once('=')).find(|(k, _)| *k == key).map(|(_, v)| v.replace("%20", " "))
}

/// Stato piatto per i client: stato, alias, porta, id avvio, acceso da, in uso e perché.
pub fn status_json(s: &EngineStatus) -> Value {
    let base = json!({ "aethera": env!("CARGO_PKG_VERSION") });
    let mut v = match s {
        EngineStatus::Off { last } => json!({ "state": "off", "last_run_id": last.as_ref().map(|f| &f.run.run_id) }),
        EngineStatus::Exited { finished } => json!({
            "state": "exited", "run_id": finished.run.run_id, "profile": finished.run.profile, "code": finished.code,
        }),
        EngineStatus::Orphan { orphan, .. } => json!({
            "state": "orphan", "pid": orphan.pid, "port": orphan.port, "base_url": orphan.base_url, "alias": orphan.alias,
        }),
        EngineStatus::Loading { run, elapsed_s, usage, .. } => json!({
            "state": "loading", "run_id": run.run_id, "profile": run.profile, "alias": run.profile, "base_url": run.base_url,
            "started_at": run.started_at, "elapsed_s": elapsed_s, "in_use": usage.in_use, "usage": usage,
        }),
        EngineStatus::Ready { run, uptime_s, ctx_served, usage, counters, telemetry, degraded, .. } => json!({
            "state": if degraded.is_empty() { "ready" } else { "degraded" },
            "run_id": run.run_id, "profile": run.profile, "alias": run.profile, "base_url": run.base_url,
            "started_at": run.started_at, "uptime_s": uptime_s, "ctx_declared": run.ctx_declared, "ctx_served": ctx_served,
            "build": run.build, "in_use": usage.in_use, "usage": usage, "degraded": degraded,
            "requests": telemetry.requests, "counters": counters,
        }),
    };
    if let (Some(obj), Some(b)) = (v.as_object_mut(), base.as_object()) {
        obj.extend(b.clone());
    }
    v
}

pub fn route(engine: &Engine, services: &Services, method: &str, target: &str, origin: bool, body: &[u8]) -> (u16, Value) {
    if origin {
        return (403, json!({ "error": "richiesta da un browser rifiutata: l'endpoint è per i processi locali" }));
    }
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    match (method, path) {
        ("GET", "/status") => {
            let mut v = status_json(&engine.status());
            // I servizi stanno accanto allo stato del motore, non dentro `usage`: non lo rendono
            // «in uso» e non bloccano arresto né riavvio. Chi legge `in_use` legge il principale.
            if let Some(obj) = v.as_object_mut() {
                obj.insert("services".into(), json!(services.view(system::probe().as_ref())));
            }
            (200, v)
        }
        ("GET", "/services") => (200, json!({ "services": services.view(system::probe().as_ref()) })),
        ("GET", "/run") => match engine.manifest() {
            Some(m) => (200, serde_json::to_value(m).unwrap_or(Value::Null)),
            None => (404, json!({ "error": "nessun motore acceso da questo Aethera" })),
        },
        ("POST", "/lock") => {
            let req: LockRequest = match serde_json::from_slice(body) {
                Ok(r) => r,
                Err(e) => return (400, json!({ "error": format!("atteso {{\"client\": …, \"label\": …, \"ttl_s\": …}}: {e}") })),
            };
            match engine.acquire_lock(req) {
                Ok((run_id, lock)) => (201, json!({ "run_id": run_id, "lock": lock, "usage": engine.usage() })),
                Err(e) if e.starts_with("nessun motore") => (409, json!({ "error": e })),
                Err(e) => (400, json!({ "error": e })),
            }
        }
        ("DELETE", "/lock") => {
            let parsed: Value = serde_json::from_slice(body).unwrap_or(Value::Null);
            let id = query_param(query, "id").or_else(|| parsed["id"].as_str().map(str::to_string));
            let client = query_param(query, "client").or_else(|| parsed["client"].as_str().map(str::to_string));
            if id.is_none() && client.is_none() {
                return (400, json!({ "error": "indica id o client del lock da rilasciare" }));
            }
            let released = engine.release_lock(id.as_deref(), client.as_deref());
            (200, json!({ "released": released, "usage": engine.usage() }))
        }
        ("GET", "/telemetry/recent") => {
            let n = query_param(query, "n").and_then(|n| n.parse().ok()).unwrap_or(crate::telemetry::RECENT).clamp(1, 1000);
            match engine.recent(n) {
                Some((run_id, summary, records)) => (200, json!({ "run_id": run_id, "summary": summary, "records": records })),
                None => (404, json!({ "error": "nessun motore acceso da questo Aethera" })),
            }
        }
        (_, "/status" | "/run" | "/lock" | "/telemetry/recent" | "/services") => (405, json!({ "error": "metodo non ammesso" })),
        _ => (404, json!({ "error": "percorsi: GET /status · GET /run · POST /lock · DELETE /lock · GET /telemetry/recent" })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_with_engine_off() {
        let e = Engine::default();
        let (s, v) = route(&e, &Services::default(), "GET", "/status", false, b"");
        assert_eq!((s, v["state"].as_str()), (200, Some("off")));
        assert_eq!(route(&e, &Services::default(), "GET", "/run", false, b"").0, 404);
        assert_eq!(route(&e, &Services::default(), "POST", "/lock", false, br#"{"client":"nonio","ttl_s":60}"#).0, 409);
        assert_eq!(route(&e, &Services::default(), "POST", "/lock", false, b"nope").0, 400);
        assert_eq!(route(&e, &Services::default(), "DELETE", "/lock", false, b"").0, 400);
        assert_eq!(route(&e, &Services::default(), "DELETE", "/lock?client=nonio", false, b"").0, 200);
        assert_eq!(route(&e, &Services::default(), "GET", "/status", true, b"").0, 403);
        assert_eq!(route(&e, &Services::default(), "PUT", "/lock", false, b"").0, 405);
        assert_eq!(route(&e, &Services::default(), "GET", "/", false, b"").0, 404);
    }

    #[test]
    fn serves_over_tcp() {
        let addr = spawn(Engine::default(), Services::default(), "127.0.0.1:0").unwrap();
        let mut s = TcpStream::connect(addr).unwrap();
        s.write_all(b"GET /status HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).unwrap();
        assert!(out.starts_with("HTTP/1.1 200 OK"), "{out}");
        assert!(out.contains("\"state\": \"off\""), "{out}");
    }
}
