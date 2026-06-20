//! HTTP/1.1 server on :7891 — hand-written over std::net, no external deps.
//! One thread per connection. State is read from `DevSharedState`; mutations
//! go into `DevQueues` for `begin_frame()` to drain.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::dev_inspector::{DevSharedState, DevQueues, QueuedDevCmd, SseEvent};
use crate::dev_inspector::assert_engine::{evaluate, evaluate_layout, AssertionReport};
use crate::dev_inspector::layout::{self, ScenarioMeta};
use crate::dev_inspector::input_queue::DevInput;

const PORT: u16 = 7891;
const SCENARIO_DIR: &str = "dev/scenarios";

pub fn start(shared: Arc<Mutex<DevSharedState>>, queues: Arc<Mutex<DevQueues>>) {
    std::thread::Builder::new()
        .name("dev-inspector-http".into())
        .spawn(move || {
            let addr = format!("127.0.0.1:{PORT}");
            let listener = match TcpListener::bind(&addr) {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("[dev-inspector] bind {addr} failed: {e}");
                    return;
                }
            };
            for stream in listener.incoming() {
                match stream {
                    Ok(s) => {
                        let sh = shared.clone();
                        let qu = queues.clone();
                        std::thread::spawn(move || handle(s, sh, qu));
                    }
                    Err(e) => eprintln!("[dev-inspector] accept error: {e}"),
                }
            }
        })
        .expect("failed to spawn dev-inspector thread");
}

// ─── Request parsing ──────────────────────────────────────────────────────────

struct Request {
    method: String,
    path:   String,
    query:  String,
    body:   Vec<u8>,
}

fn read_request(stream: &mut TcpStream) -> Option<Request> {
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    let mut buf = Vec::with_capacity(4096);
    let mut tmp = [0u8; 4096];
    loop {
        match stream.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => {
                buf.extend_from_slice(&tmp[..n]);
                if buf.windows(4).any(|w| w == b"\r\n\r\n") { break; }
                if buf.len() > 65536 { return None; }
            }
            Err(_) => break,
        }
    }
    let header_end = buf.windows(4).position(|w| w == b"\r\n\r\n")?;
    let header_str = std::str::from_utf8(&buf[..header_end]).ok()?;
    let mut lines = header_str.lines();
    let first = lines.next()?;
    let mut parts = first.splitn(3, ' ');
    let method = parts.next()?.to_string();
    let path_raw = parts.next()?.to_string();
    let (path, query) = if let Some(q) = path_raw.find('?') {
        (path_raw[..q].to_string(), path_raw[q+1..].to_string())
    } else {
        (path_raw, String::new())
    };

    // Read body if Content-Length is set
    let mut content_len = 0usize;
    for line in lines {
        let lower = line.to_lowercase();
        if lower.starts_with("content-length:") {
            content_len = lower["content-length:".len()..].trim().parse().unwrap_or(0);
        }
    }
    let body_start = header_end + 4;
    let mut body = buf[body_start..].to_vec();
    while body.len() < content_len {
        let need = content_len - body.len();
        let to_read = need.min(4096);
        let mut tmp2 = vec![0u8; to_read];
        match stream.read(&mut tmp2) {
            Ok(0) => break,
            Ok(n) => body.extend_from_slice(&tmp2[..n]),
            Err(_) => break,
        }
    }

    Some(Request { method, path, query, body })
}

// ─── Response helpers ─────────────────────────────────────────────────────────

fn write_response(stream: &mut TcpStream, status: u16, content_type: &str, body: &[u8]) {
    let status_text = match status {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        404 => "Not Found",
        422 => "Unprocessable Entity",
        500 => "Internal Server Error",
        _   => "Unknown",
    };
    let header = format!(
        "HTTP/1.1 {status} {status_text}\r\n\
         Content-Type: {content_type}\r\n\
         Content-Length: {}\r\n\
         Access-Control-Allow-Origin: *\r\n\
         Connection: close\r\n\
         \r\n",
        body.len(),
    );
    let _ = stream.write_all(header.as_bytes());
    let _ = stream.write_all(body);
}

fn ok_json(stream: &mut TcpStream, val: &serde_json::Value) {
    let body = serde_json::to_vec(val).unwrap_or_default();
    write_response(stream, 200, "application/json", &body);
}
fn err_json(stream: &mut TcpStream, code: u16, msg: &str) {
    let body = serde_json::to_vec(&serde_json::json!({"error": msg})).unwrap_or_default();
    write_response(stream, code, "application/json", &body);
}
fn parse_body(body: &[u8]) -> serde_json::Value {
    serde_json::from_slice(body).unwrap_or(serde_json::Value::Null)
}

// ─── Connection handler ───────────────────────────────────────────────────────

fn handle(
    mut stream: TcpStream,
    shared: Arc<Mutex<DevSharedState>>,
    queues: Arc<Mutex<DevQueues>>,
) {
    let Some(req) = read_request(&mut stream) else { return };
    let method = req.method.as_str();
    let path   = req.path.as_str();

    // CORS pre-flight
    if method == "OPTIONS" {
        let header = "HTTP/1.1 204 No Content\r\n\
            Access-Control-Allow-Origin: *\r\n\
            Access-Control-Allow-Methods: GET,POST,OPTIONS\r\n\
            Access-Control-Allow-Headers: Content-Type\r\n\
            Connection: close\r\n\r\n";
        let _ = stream.write_all(header.as_bytes());
        return;
    }

    match (method, path) {
        // ── Health ────────────────────────────────────────────────────────
        ("GET", "/health") => {
            ok_json(&mut stream, &serde_json::json!({
                "status": "ok", "port": PORT,
            }));
        }

        // ── State ─────────────────────────────────────────────────────────
        ("GET", "/state") => {
            let state = shared.lock().unwrap();
            ok_json(&mut stream, &state.app_state);
        }
        ("GET", "/widget-tree") => {
            let state = shared.lock().unwrap();
            ok_json(&mut stream, &serde_json::to_value(&state.widget_tree).unwrap_or_default());
        }
        ("GET", "/layout-snapshot") => {
            let state = shared.lock().unwrap();
            let snapshot: serde_json::Value = state.widget_tree.iter()
                .map(|w| (w.id.clone(), serde_json::to_value(&w.rect).unwrap_or_default()))
                .collect::<serde_json::Map<_, _>>().into();
            ok_json(&mut stream, &snapshot);
        }
        ("GET", "/report") => {
            let state = shared.lock().unwrap();
            let html = build_html_report(&state);
            write_response(&mut stream, 200, "text/html; charset=utf-8", html.as_bytes());
        }

        // ── Domain endpoints ───────────────────────────────────────────────
        ("GET", "/chart") => {
            let state = shared.lock().unwrap();
            ok_json(&mut stream, &state.app_state);
        }
        ("GET", "/panes") => {
            let state = shared.lock().unwrap();
            let panes = state.app_state.get("panes").cloned()
                .unwrap_or(serde_json::Value::Array(vec![]));
            ok_json(&mut stream, &panes);
        }
        ("GET", "/watchlist") => {
            let state = shared.lock().unwrap();
            let wl = state.app_state.get("watchlist").cloned()
                .unwrap_or(serde_json::Value::Null);
            ok_json(&mut stream, &wl);
        }

        // ── Commands ───────────────────────────────────────────────────────
        ("POST", "/reset") => {
            queues.lock().unwrap().reset_pending = true;
            let frame_ok = wait_for_next_frame(&shared, 2000);
            ok_json(&mut stream, &serde_json::json!({
                "ok": true, "frame_advanced": frame_ok,
            }));
        }
        ("POST", "/cmd") => {
            let body = parse_body(&req.body);
            match parse_app_command(&body) {
                Ok(cmd) => {
                    queues.lock().unwrap().commands.push(QueuedDevCmd::App(cmd));
                    wait_for_next_frame(&shared, 1000);
                    ok_json(&mut stream, &serde_json::json!({"ok": true}));
                }
                Err(e) => err_json(&mut stream, 400, &e),
            }
        }
        ("POST", "/input") => {
            let body = parse_body(&req.body);
            match serde_json::from_value::<DevInput>(body) {
                Ok(input) => {
                    queues.lock().unwrap().inputs.push(input);
                    wait_for_next_frame(&shared, 1000);
                    ok_json(&mut stream, &serde_json::json!({"ok": true}));
                }
                Err(e) => err_json(&mut stream, 400, &e.to_string()),
            }
        }
        ("POST", "/input/sequence") => {
            let body = parse_body(&req.body);
            let events: Vec<DevInput> = match serde_json::from_value(body) {
                Ok(v) => v,
                Err(e) => { err_json(&mut stream, 400, &e.to_string()); return; }
            };
            {
                let mut q = queues.lock().unwrap();
                for e in events { q.inputs.push(e); }
            }
            wait_for_next_frame(&shared, 1000);
            ok_json(&mut stream, &serde_json::json!({"ok": true}));
        }

        // ── Assertions ─────────────────────────────────────────────────────
        ("POST", "/assert") => {
            let body = parse_body(&req.body);
            let assertions = match body.as_array() {
                Some(a) => a.clone(),
                None    => { err_json(&mut stream, 400, "body must be array"); return; }
            };
            let state = shared.lock().unwrap();
            let report = evaluate(&assertions, &state);
            let status = if report.failed == 0 { 200 } else { 422 };
            let val = serde_json::to_value(&report).unwrap_or_default();
            let body_bytes = serde_json::to_vec(&val).unwrap_or_default();
            write_response(&mut stream, status, "application/json", &body_bytes);
        }
        ("POST", "/assert-layout") => {
            let body = parse_body(&req.body);
            let assertions = match body.as_array() {
                Some(a) => a.clone(),
                None    => { err_json(&mut stream, 400, "body must be array"); return; }
            };
            let widgets = shared.lock().unwrap().widget_tree.clone();
            let report = evaluate_layout(&assertions, &widgets);
            let status = if report.failed == 0 { 200 } else { 422 };
            let val = serde_json::to_value(&report).unwrap_or_default();
            let body_bytes = serde_json::to_vec(&val).unwrap_or_default();
            write_response(&mut stream, status, "application/json", &body_bytes);
        }

        // ── Snapshots & checkpoints ────────────────────────────────────────
        ("POST", "/snapshot/save") => {
            let body = parse_body(&req.body);
            let name = body["name"].as_str().unwrap_or("unnamed");
            let widgets = shared.lock().unwrap().widget_tree.clone();
            match layout::save_snapshot(name, &widgets) {
                Ok(path) => ok_json(&mut stream, &serde_json::json!({"ok": true, "path": path})),
                Err(e)   => err_json(&mut stream, 500, &e),
            }
        }
        ("GET", "/layout-diff") => {
            let baseline = req.query.split('&')
                .find_map(|p| p.strip_prefix("baseline="))
                .unwrap_or("baseline")
                .to_string();
            let widgets = shared.lock().unwrap().widget_tree.clone();
            let diff = layout::diff_layout(&baseline, &widgets, 2.0, 4.0);
            ok_json(&mut stream, &diff);
        }
        ("POST", "/checkpoint/save") => {
            let body = parse_body(&req.body);
            let name  = body["name"].as_str().unwrap_or("unnamed");
            let state = shared.lock().unwrap().app_state.clone();
            match layout::save_checkpoint(name, &state) {
                Ok(path) => ok_json(&mut stream, &serde_json::json!({"ok": true, "path": path})),
                Err(e)   => err_json(&mut stream, 500, &e),
            }
        }

        // ── Batch ─────────────────────────────────────────────────────────
        ("POST", "/batch") => {
            handle_batch(&mut stream, &req.body, &shared, &queues);
        }

        // ── Scenarios ─────────────────────────────────────────────────────
        ("POST", "/run-scenario") => {
            handle_run_scenario(&mut stream, &req.body, &shared, &queues);
        }
        ("GET", "/scenario-list") => {
            let tag_filter = req.query.split('&')
                .find_map(|p| p.strip_prefix("tag="))
                .map(|s| s.to_string());
            let mut metas = layout::list_scenarios(SCENARIO_DIR);
            if let Some(tag) = tag_filter {
                metas.retain(|m| m.tags.as_ref()
                    .map(|t| t.contains(&tag)).unwrap_or(false));
            }
            ok_json(&mut stream, &serde_json::json!({
                "count": metas.len(),
                "scenarios": metas,
            }));
        }

        // ── SSE event stream ───────────────────────────────────────────────
        ("GET", "/events") => {
            handle_sse(&mut stream, &shared);
        }

        _ => {
            err_json(&mut stream, 404, &format!("not found: {method} {path}"));
        }
    }
}

// ─── Batch ────────────────────────────────────────────────────────────────────

fn handle_batch(
    stream: &mut TcpStream,
    body_bytes: &[u8],
    shared: &Arc<Mutex<DevSharedState>>,
    queues: &Arc<Mutex<DevQueues>>,
) {
    let requests: Vec<serde_json::Value> = match serde_json::from_slice(body_bytes) {
        Ok(v) => v,
        Err(e) => { err_json(stream, 400, &e.to_string()); return; }
    };
    let mut results = Vec::new();
    for req_val in requests {
        let method = req_val["method"].as_str().unwrap_or("GET");
        let path   = req_val["path"].as_str().unwrap_or("/");
        let body   = req_val.get("body").cloned().unwrap_or(serde_json::Value::Null);
        let body_bytes = serde_json::to_vec(&body).unwrap_or_default();

        // Route a mini-request
        let (status, resp_body) = route_for_batch(method, path, &body_bytes, shared, queues);
        results.push(serde_json::json!({
            "method": method,
            "path":   path,
            "status": status,
            "body":   serde_json::from_slice::<serde_json::Value>(&resp_body)
                .unwrap_or(serde_json::Value::String(
                    String::from_utf8_lossy(&resp_body).to_string())),
        }));
    }
    ok_json(stream, &serde_json::Value::Array(results));
}

fn route_for_batch(
    method: &str,
    path: &str,
    body: &[u8],
    shared: &Arc<Mutex<DevSharedState>>,
    queues: &Arc<Mutex<DevQueues>>,
) -> (u16, Vec<u8>) {
    match (method, path) {
        ("GET", "/state") => {
            let s = shared.lock().unwrap();
            (200, serde_json::to_vec(&s.app_state).unwrap_or_default())
        }
        ("GET", "/widget-tree") => {
            let s = shared.lock().unwrap();
            (200, serde_json::to_vec(&s.widget_tree).unwrap_or_default())
        }
        ("POST", "/cmd") => {
            let body_val = parse_body(body);
            match parse_app_command(&body_val) {
                Ok(cmd) => {
                    queues.lock().unwrap().commands.push(QueuedDevCmd::App(cmd));
                    wait_for_next_frame(shared, 1000);
                    (200, b"{\"ok\":true}".to_vec())
                }
                Err(e) => (400, format!("{{\"error\":{e:?}}}").into_bytes()),
            }
        }
        ("POST", "/assert") => {
            let body_val = parse_body(body);
            let assertions = body_val.as_array().cloned().unwrap_or_default();
            let state = shared.lock().unwrap();
            let report = evaluate(&assertions, &state);
            let code = if report.failed == 0 { 200 } else { 422 };
            (code, serde_json::to_vec(&report).unwrap_or_default())
        }
        _ => (404, b"{\"error\":\"not found\"}".to_vec()),
    }
}

// ─── Scenario runner ──────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
struct ScenarioFile {
    name:             String,
    description:      Option<String>,
    settle_ms:        Option<u64>,
    abort_on_failure: Option<bool>,
    steps:            Vec<ScenarioStep>,
}

#[derive(Debug, serde::Deserialize, Clone)]
struct ScenarioStep {
    action:      String,
    // Action-specific fields — we parse from the whole object:
    #[serde(flatten)]
    args:        serde_json::Value,
}

#[derive(Debug, serde::Serialize)]
struct StepResult {
    step:        usize,
    action:      String,
    pass:        bool,
    detail:      String,
    duration_ms: u64,
}

#[derive(Debug, serde::Serialize)]
struct ScenarioResult {
    scenario:    String,
    pass:        bool,
    passed:      usize,
    failed:      usize,
    step_count:  usize,
    duration_ms: u64,
    steps:       Vec<StepResult>,
}

fn handle_run_scenario(
    stream: &mut TcpStream,
    body_bytes: &[u8],
    shared: &Arc<Mutex<DevSharedState>>,
    queues: &Arc<Mutex<DevQueues>>,
) {
    // Resolve scenario: either an inline body or a `file` reference.
    let body = parse_body(body_bytes);
    let scenario: ScenarioFile = if let Some(file) = body["file"].as_str() {
        let path = format!("{SCENARIO_DIR}/{file}");
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) => { err_json(stream, 404, &format!("scenario {file}: {e}")); return; }
        };
        match serde_json::from_slice(&bytes) {
            Ok(s) => s,
            Err(e) => { err_json(stream, 400, &e.to_string()); return; }
        }
    } else {
        match serde_json::from_value(body) {
            Ok(s) => s,
            Err(e) => { err_json(stream, 400, &e.to_string()); return; }
        }
    };

    let result = run_scenario(scenario, shared, queues);
    let status = if result.pass { 200 } else { 422 };
    let val = serde_json::to_vec(&serde_json::to_value(&result).unwrap_or_default())
        .unwrap_or_default();
    write_response(stream, status, "application/json", &val);
}

fn run_scenario(
    scenario: ScenarioFile,
    shared: &Arc<Mutex<DevSharedState>>,
    queues: &Arc<Mutex<DevQueues>>,
) -> ScenarioResult {
    let start = Instant::now();
    let settle = Duration::from_millis(scenario.settle_ms.unwrap_or(0));
    let abort_on_fail = scenario.abort_on_failure.unwrap_or(false);
    let mut step_results = Vec::new();
    let mut passed = 0usize;
    let mut failed = 0usize;

    for (i, step) in scenario.steps.iter().enumerate() {
        let step_start = Instant::now();
        let (pass, detail) = execute_step(step, shared, queues);
        if settle.as_millis() > 0 {
            std::thread::sleep(settle);
        }
        let duration_ms = step_start.elapsed().as_millis() as u64;
        step_results.push(StepResult {
            step: i,
            action: step.action.clone(),
            pass, detail, duration_ms,
        });
        if pass { passed += 1; } else { failed += 1; }
        if !pass && abort_on_fail { break; }
    }

    ScenarioResult {
        scenario:   scenario.name.clone(),
        pass:       failed == 0,
        passed, failed,
        step_count: scenario.steps.len(),
        duration_ms: start.elapsed().as_millis() as u64,
        steps:      step_results,
    }
}

fn execute_step(
    step: &ScenarioStep,
    shared: &Arc<Mutex<DevSharedState>>,
    queues: &Arc<Mutex<DevQueues>>,
) -> (bool, String) {
    let args = &step.args;
    match step.action.as_str() {
        "reset" => {
            queues.lock().unwrap().reset_pending = true;
            let ok = wait_for_next_frame(shared, 2000);
            (true, format!("app reset (frame_advanced={ok})"))
        }

        "log" => {
            let msg = args["message"].as_str().unwrap_or("(no message)");
            eprintln!("[scenario] {msg}");
            (true, msg.to_string())
        }

        "wait" => {
            let ms = args["ms"].as_u64().unwrap_or(100);
            std::thread::sleep(Duration::from_millis(ms));
            (true, format!("waited {ms}ms"))
        }

        "wait_frames" => {
            let count = args["count"].as_u64().unwrap_or(1) as u64;
            for _ in 0..count {
                wait_for_next_frame(shared, 500);
            }
            (true, format!("waited {count} frame(s)"))
        }

        "cmd" => {
            let mut merged = args.clone();
            let cmd_name = merged["cmd"].as_str()
                .or_else(|| step.args["cmd"].as_str())
                .unwrap_or("")
                .to_string();
            if let Some(obj) = merged.as_object_mut() {
                obj.insert("cmd".into(), serde_json::Value::String(cmd_name.clone()));
            }
            match parse_app_command(&merged) {
                Ok(cmd) => {
                    queues.lock().unwrap().commands.push(QueuedDevCmd::App(cmd));
                    wait_for_next_frame(shared, 1000);
                    (true, format!("queued cmd={cmd_name}"))
                }
                Err(e) => (false, e),
            }
        }

        "cmd_batch" => {
            let cmds = args["cmds"].as_array().cloned().unwrap_or_default();
            let mut errors = Vec::new();
            for c in &cmds {
                match parse_app_command(c) {
                    Ok(cmd) => { queues.lock().unwrap().commands.push(QueuedDevCmd::App(cmd)); }
                    Err(e)  => errors.push(e),
                }
            }
            wait_for_next_frame(shared, 1000);
            if errors.is_empty() {
                (true, format!("queued {} commands", cmds.len()))
            } else {
                (false, errors.join("; "))
            }
        }

        "input" => {
            let input_body = args.clone();
            match serde_json::from_value::<DevInput>(input_body) {
                Ok(input) => {
                    queues.lock().unwrap().inputs.push(input);
                    wait_for_next_frame(shared, 1000);
                    (true, format!("injected {} input", step.action))
                }
                Err(e) => (false, e.to_string()),
            }
        }

        "key_sequence" => {
            let keys: Vec<String> = args["keys"].as_array()
                .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default();
            let delay = args["delay_ms"].as_u64().unwrap_or(0);
            for key in &keys {
                {
                    let mut q = queues.lock().unwrap();
                    q.inputs.push(DevInput::Key { key: key.clone() });
                }
                wait_for_next_frame(shared, 500);
                if delay > 0 { std::thread::sleep(Duration::from_millis(delay)); }
            }
            (true, format!("injected {} keys", keys.len()))
        }

        "type" => {
            let text = args["text"].as_str().unwrap_or("").to_string();
            queues.lock().unwrap().inputs.push(DevInput::Type { text: text.clone() });
            wait_for_next_frame(shared, 500);
            (true, format!("typed {:?}", text))
        }

        "assert" => {
            let assertions = args["assertions"].as_array().cloned().unwrap_or_default();
            let state = shared.lock().unwrap();
            let report = evaluate(&assertions, &state);
            let pass = report.failed == 0;
            let detail = report.results.iter()
                .map(|r| format!("[{}] {}", if r.pass {"✓"} else {"✗"}, r.detail))
                .collect::<Vec<_>>().join("; ");
            (pass, detail)
        }

        "assert_layout" => {
            let assertions = args["assertions"].as_array().cloned().unwrap_or_default();
            let widgets = shared.lock().unwrap().widget_tree.clone();
            let report = evaluate_layout(&assertions, &widgets);
            let pass = report.failed == 0;
            let detail = report.results.iter()
                .map(|r| format!("[{}] {}", if r.pass {"✓"} else {"✗"}, r.detail))
                .collect::<Vec<_>>().join("; ");
            (pass, detail)
        }

        "assert_poll" => {
            let assertions = args["assertions"].as_array().cloned().unwrap_or_default();
            let timeout_ms  = args["timeout_ms"].as_u64().unwrap_or(3000);
            let interval_ms = args["interval_ms"].as_u64().unwrap_or(100);
            let deadline = Instant::now() + Duration::from_millis(timeout_ms);

            loop {
                let report = evaluate(&assertions, &shared.lock().unwrap());
                if report.failed == 0 {
                    let detail = report.results.iter()
                        .map(|r| r.detail.as_str()).collect::<Vec<_>>().join("; ");
                    return (true, detail);
                }
                if Instant::now() >= deadline {
                    let detail = report.results.iter()
                        .filter(|r| !r.pass)
                        .map(|r| format!("✗ {}", r.detail))
                        .collect::<Vec<_>>().join("; ");
                    return (false, format!("poll timed out ({timeout_ms}ms): {detail}"));
                }
                std::thread::sleep(Duration::from_millis(interval_ms));
                wait_for_next_frame(shared, interval_ms + 100);
            }
        }

        "snapshot" => {
            let name = args["name"].as_str().unwrap_or("unnamed");
            let widgets = shared.lock().unwrap().widget_tree.clone();
            match layout::save_snapshot(name, &widgets) {
                Ok(path) => (true, format!("snapshot saved to {path}")),
                Err(e)   => (false, e),
            }
        }

        "loop" => {
            let count = args["count"].as_u64().unwrap_or(1);
            let inner_steps: Vec<ScenarioStep> = match serde_json::from_value(
                args["steps"].clone()
            ) {
                Ok(s) => s,
                Err(e) => return (false, format!("loop steps parse error: {e}")),
            };
            let mut any_fail = false;
            for i in 0..count {
                for s in &inner_steps {
                    let (pass, detail) = execute_step(s, shared, queues);
                    if !pass {
                        any_fail = true;
                        eprintln!("[scenario] loop iter {i} step {} failed: {detail}", s.action);
                    }
                }
            }
            (!any_fail, format!("{count} iterations done"))
        }

        "http_get" => {
            let path = args["path"].as_str().unwrap_or("/state");
            // Re-read shared state via the path (subset only)
            let state = shared.lock().unwrap().app_state.clone();
            (true, format!("GET {path}: {state}"))
        }
        "http_post" => {
            let path = args["path"].as_str().unwrap_or("/cmd");
            let body = args["body"].clone();
            // Execute inline
            let (code, _) = route_for_batch("POST", path,
                &serde_json::to_vec(&body).unwrap_or_default(), shared, queues);
            (code < 300, format!("POST {path} → {code}"))
        }

        unknown => (false, format!("unknown action: '{unknown}'")),
    }
}

// ─── Frame synchronisation ────────────────────────────────────────────────────

/// Block until the frame counter increments (one render cycle completes) or timeout.
/// This is the critical synchronisation primitive — no arbitrary sleeps.
pub fn wait_for_next_frame(shared: &Arc<Mutex<DevSharedState>>, timeout_ms: u64) -> bool {
    let start_frame = shared.lock().unwrap().frame_counter;
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        std::thread::sleep(Duration::from_millis(4)); // ~60 fps cadence
        if shared.lock().unwrap().frame_counter > start_frame { return true; }
        if Instant::now() > deadline { return false; }
    }
}

// ─── SSE event stream ─────────────────────────────────────────────────────────

fn handle_sse(stream: &mut TcpStream, shared: &Arc<Mutex<DevSharedState>>) {
    let header = "HTTP/1.1 200 OK\r\n\
        Content-Type: text/event-stream\r\n\
        Cache-Control: no-cache\r\n\
        Access-Control-Allow-Origin: *\r\n\
        Connection: keep-alive\r\n\
        \r\n";
    if stream.write_all(header.as_bytes()).is_err() { return; }

    let mut last_seq: u64 = 0;
    loop {
        // Drain new events since last_seq
        let events: Vec<SseEvent> = {
            let g = shared.lock().unwrap();
            g.sse_ring.iter()
                .filter(|e| e.seq > last_seq)
                .cloned()
                .collect()
        };
        for ev in &events {
            let data = serde_json::to_string(&ev.data).unwrap_or_default();
            let msg = format!("event: {}\ndata: {}\nid: {}\n\n", ev.name, data, ev.seq);
            if stream.write_all(msg.as_bytes()).is_err() { return; }
            last_seq = ev.seq;
        }
        // Send keepalive comment if no events
        if events.is_empty() {
            if stream.write_all(b": keepalive\n\n").is_err() { return; }
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

// ─── HTML report ──────────────────────────────────────────────────────────────

fn build_html_report(state: &DevSharedState) -> String {
    let fps    = state.fps;
    let frame  = state.frame_counter;
    let symbol = state.app_state["active_symbol"].as_str().unwrap_or("—");
    let tf     = state.app_state["active_timeframe"].as_str().unwrap_or("—");
    let bars   = state.app_state["bar_count"].as_u64().unwrap_or(0);
    let panes  = state.app_state["pane_count"].as_u64().unwrap_or(0);
    let dialogs = state.open_dialogs.join(", ");
    let violations = state.active_violations.len();
    let widgets = state.widget_tree.len();

    format!(r#"<!DOCTYPE html><html><head>
<title>Apex Terminal — Dev Inspector</title>
<style>
body{{font-family:monospace;background:#0d0d0d;color:#e0e0e0;padding:20px}}
h1{{color:#4af;margin:0 0 16px}}
table{{border-collapse:collapse;margin-bottom:20px}}
td,th{{padding:4px 12px;border:1px solid #333;text-align:left}}
th{{background:#1a1a2e;color:#7af}}
.pass{{color:#4d4}}
.fail{{color:#f44}}
</style></head><body>
<h1>Dev Inspector — Apex Terminal</h1>
<table>
<tr><th>Field</th><th>Value</th></tr>
<tr><td>Frame</td><td>{frame}</td></tr>
<tr><td>FPS</td><td>{fps:.1}</td></tr>
<tr><td>Symbol</td><td>{symbol}</td></tr>
<tr><td>Timeframe</td><td>{tf}</td></tr>
<tr><td>Bar count</td><td>{bars}</td></tr>
<tr><td>Pane count</td><td>{panes}</td></tr>
<tr><td>Open dialogs</td><td>{dialogs}</td></tr>
<tr><td>Widget records</td><td>{widgets}</td></tr>
<tr><td>Violations</td><td class="{}">{violations}</td></tr>
</table>
<p>API: <a href="http://localhost:{PORT}/state" style="color:#7af">/state</a>
 | <a href="/widget-tree" style="color:#7af">/widget-tree</a>
 | <a href="/scenario-list" style="color:#7af">/scenario-list</a>
</p>
</body></html>"#,
        if violations > 0 { "fail" } else { "pass" })
}

// ─── AppCommand parser ────────────────────────────────────────────────────────

fn parse_app_command(
    body: &serde_json::Value,
) -> Result<crate::chart_renderer::commands::AppCommand, String> {
    use crate::chart_renderer::commands::{AppCommand, ChartFlag};
    use crate::chart_renderer::gpu::{IndicatorType, PaneType};

    let cmd = body["cmd"].as_str().unwrap_or("").trim();
    let pane = body["pane"].as_u64().unwrap_or(0) as usize;

    match cmd {
        // ── Symbol / timeframe ─────────────────────────────────────────────
        "SwapPaneSymbol" | "swap_pane_symbol" => {
            let symbol = body["symbol"].as_str().unwrap_or("").to_uppercase();
            if symbol.is_empty() { return Err("symbol required".into()); }
            Ok(AppCommand::SwapPaneSymbol { pane, symbol })
        }
        "ChangeTimeframe" | "change_timeframe" => {
            let tf = body["tf"].as_str().unwrap_or("5m").to_string();
            Ok(AppCommand::ChangeTimeframe { pane, tf })
        }
        "ChangePaneType" | "change_pane_type" => {
            let kind_str = body["kind"].as_str().unwrap_or("Chart");
            let kind = parse_pane_type(kind_str)?;
            Ok(AppCommand::ChangePaneType { pane, kind })
        }

        // ── Display flags ──────────────────────────────────────────────────
        "SetChartFlag" | "set_chart_flag" => {
            let flag_str = body["flag"].as_str().unwrap_or("");
            let value    = body["value"].as_bool().unwrap_or(true);
            let flag     = parse_chart_flag(flag_str)?;
            Ok(AppCommand::SetChartFlag { pane, flag, value })
        }

        // ── Theme / style ──────────────────────────────────────────────────
        "SetThemeIdx" | "set_theme_idx" => {
            let idx = body["idx"].as_u64().unwrap_or(0) as usize;
            Ok(AppCommand::SetThemeIdx { pane, idx })
        }
        "SetStyleIdx" | "set_style_idx" => {
            let idx = body["idx"].as_u64().unwrap_or(0) as usize;
            Ok(AppCommand::SetStyleIdx { idx })
        }

        // ── Indicators ─────────────────────────────────────────────────────
        "AddIndicator" | "add_indicator" => {
            let kind_str = body["kind"].as_str().unwrap_or("RSI");
            let kind     = parse_indicator_type(kind_str)?;
            Ok(AppCommand::AddIndicator { pane, kind })
        }
        "RemoveIndicator" | "remove_indicator" => {
            let id = body["id"].as_u64().unwrap_or(0) as u32;
            Ok(AppCommand::RemoveIndicator { pane, id })
        }
        "RecomputeIndicators" | "recompute_indicators" => {
            Ok(AppCommand::RecomputeIndicators { pane })
        }

        // ── Orders ─────────────────────────────────────────────────────────
        "CancelAllOrders"   | "cancel_all_orders"  => Ok(AppCommand::CancelAllOrders),
        "ClearOrderHistory" | "clear_order_history"=> Ok(AppCommand::ClearOrderHistory),
        "PlaceAllDraftOrders" | "place_all_draft_orders" => Ok(AppCommand::PlaceAllDraftOrders),

        // ── Alerts ─────────────────────────────────────────────────────────
        "PlaceAllDraftAlerts" | "place_all_draft_alerts" => Ok(AppCommand::PlaceAllDraftAlerts),
        "AddPriceAlert" | "add_price_alert" => {
            let price = body["price"].as_f64().unwrap_or(0.0) as f32;
            let above = body["above"].as_bool().unwrap_or(true);
            Ok(AppCommand::AddPriceAlert { pane, price, above })
        }

        // ── Watchlist ──────────────────────────────────────────────────────
        "WatchlistAddSymbol" | "watchlist_add_symbol" => {
            let symbol = body["symbol"].as_str().unwrap_or("").to_uppercase();
            if symbol.is_empty() { return Err("symbol required".into()); }
            Ok(AppCommand::WatchlistAddSymbol { symbol })
        }
        "WatchlistRemoveSymbol" | "watchlist_remove_symbol" => {
            let symbol = body["symbol"].as_str().unwrap_or("").to_uppercase();
            Ok(AppCommand::WatchlistRemoveSymbol { symbol })
        }
        "WatchlistCreate" | "watchlist_create" => {
            let name = body["name"].as_str().unwrap_or("New List").to_string();
            Ok(AppCommand::WatchlistCreate { name })
        }
        "WatchlistDelete" | "watchlist_delete" => {
            let idx = body["idx"].as_u64().unwrap_or(0) as usize;
            Ok(AppCommand::WatchlistDelete { idx })
        }
        "WatchlistSwitchActive" | "watchlist_switch_active" => {
            let idx = body["idx"].as_u64().unwrap_or(0) as usize;
            Ok(AppCommand::WatchlistSwitchActive { idx })
        }
        "WatchlistAddSection" | "watchlist_add_section" => {
            let title = body["title"].as_str().unwrap_or("New Section").to_string();
            Ok(AppCommand::WatchlistAddSection { title })
        }

        // ── UI state ───────────────────────────────────────────────────────
        "CloseAllDialogs" | "close_all_dialogs" => Ok(AppCommand::CloseAllDialogs),

        "" => Err("cmd field is required".into()),
        other => Err(format!("unknown command: '{other}'")),
    }
}

fn parse_pane_type(s: &str) -> Result<crate::chart_renderer::gpu::PaneType, String> {
    use crate::chart_renderer::gpu::PaneType;
    match s {
        "Chart"       | "chart"       => Ok(PaneType::Chart),
        "Portfolio"   | "portfolio"   => Ok(PaneType::Portfolio),
        "Dashboard"   | "dashboard"   => Ok(PaneType::Dashboard),
        "Heatmap"     | "heatmap"     => Ok(PaneType::Heatmap),
        "Spreadsheet" | "spreadsheet" => Ok(PaneType::Spreadsheet),
        _ => Err(format!("unknown PaneType: '{s}'")),
    }
}

fn parse_chart_flag(s: &str) -> Result<crate::chart_renderer::commands::ChartFlag, String> {
    use crate::chart_renderer::commands::ChartFlag;
    match s {
        "ShowVolume"        | "show_volume"         => Ok(ChartFlag::ShowVolume),
        "LogScale"          | "log_scale"           => Ok(ChartFlag::LogScale),
        "Magnet"            | "magnet"              => Ok(ChartFlag::Magnet),
        "OhlcTooltip"       | "ohlc_tooltip"        => Ok(ChartFlag::OhlcTooltip),
        "MeasureTooltip"    | "measure_tooltip"     => Ok(ChartFlag::MeasureTooltip),
        "ShowOscillators"   | "show_oscillators"    => Ok(ChartFlag::ShowOscillators),
        "ShowPrevClose"     | "show_prev_close"     => Ok(ChartFlag::ShowPrevClose),
        "ShowPatternLabels" | "show_pattern_labels" => Ok(ChartFlag::ShowPatternLabels),
        "ShowFootprint"     | "show_footprint"      => Ok(ChartFlag::ShowFootprint),
        "HideAllIndicators" | "hide_all_indicators" => Ok(ChartFlag::HideAllIndicators),
        "HideAllDrawings"   | "hide_all_drawings"   => Ok(ChartFlag::HideAllDrawings),
        _ => Err(format!("unknown ChartFlag: '{s}'")),
    }
}

fn parse_indicator_type(s: &str) -> Result<crate::chart_renderer::gpu::IndicatorType, String> {
    use crate::chart_renderer::gpu::IndicatorType;
    match s.to_uppercase().as_str() {
        "SMA"             => Ok(IndicatorType::SMA),
        "EMA"             => Ok(IndicatorType::EMA),
        "WMA"             => Ok(IndicatorType::WMA),
        "DEMA"            => Ok(IndicatorType::DEMA),
        "TEMA"            => Ok(IndicatorType::TEMA),
        "VWAP"            => Ok(IndicatorType::VWAP),
        "BB" | "BOLLINGERBANDS" => Ok(IndicatorType::BollingerBands),
        "ICHI" | "ICHIMOKU"     => Ok(IndicatorType::Ichimoku),
        "PSAR" | "PARABOLICSAR" => Ok(IndicatorType::ParabolicSAR),
        "ST" | "SUPERTREND"     => Ok(IndicatorType::Supertrend),
        "KC" | "KELTNERCHANNELS"=> Ok(IndicatorType::KeltnerChannels),
        "RSI"   => Ok(IndicatorType::RSI),
        "MACD"  => Ok(IndicatorType::MACD),
        "STOCH" | "STOCHASTIC"  => Ok(IndicatorType::Stochastic),
        "ADX"   => Ok(IndicatorType::ADX),
        "CCI"   => Ok(IndicatorType::CCI),
        "WILLIAMSR" | "%R" | "WR" => Ok(IndicatorType::WilliamsR),
        "ATR"   => Ok(IndicatorType::ATR),
        "OBV"   => Ok(IndicatorType::OBV),
        _ => Err(format!("unknown IndicatorType: '{s}'")),
    }
}
