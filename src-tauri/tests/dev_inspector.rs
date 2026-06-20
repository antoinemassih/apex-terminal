//! CI integration test for the Dev Inspector.
//!
//! Spawns the app binary in `--headless` mode, waits for the HTTP server to
//! come up on :7891, then drives all five smoke scenarios via the REST API.
//!
//! Run with:  cargo test --test dev_inspector -- --nocapture
//!
//! The test is skipped automatically in release builds (the inspector is
//! compiled out) and when the binary cannot be found (CI without a prior build).

#[cfg(debug_assertions)]
mod tests {
    use std::io::Read;
    use std::net::TcpStream;
    use std::process::{Child, Command, Stdio};
    use std::time::{Duration, Instant};

    const PORT: u16 = 7891;
    const API: &str = "http://127.0.0.1:7891";

    // Path to the debug binary — built by `cargo build` before running tests.
    fn binary_path() -> std::path::PathBuf {
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        // apex-terminal-native (the standalone binary crate)
        let bin = if cfg!(target_os = "windows") {
            manifest.join("../target/debug/apex-terminal-native.exe")
        } else {
            manifest.join("../target/debug/apex-terminal-native")
        };
        bin
    }

    fn wait_for_server(timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        loop {
            if TcpStream::connect(format!("127.0.0.1:{PORT}")).is_ok() {
                return true;
            }
            if Instant::now() > deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    }

    fn http_get(path: &str) -> Option<String> {
        let mut stream = TcpStream::connect(format!("127.0.0.1:{PORT}")).ok()?;
        stream.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
        let req = format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
        use std::io::Write;
        stream.write_all(req.as_bytes()).ok()?;
        let mut buf = String::new();
        stream.read_to_string(&mut buf).ok()?;
        // Return body (after \r\n\r\n)
        buf.split("\r\n\r\n").nth(1).map(|s| s.to_string())
    }

    fn http_post(path: &str, body: &str) -> Option<String> {
        let mut stream = TcpStream::connect(format!("127.0.0.1:{PORT}")).ok()?;
        stream.set_read_timeout(Some(Duration::from_secs(10))).ok()?;
        let req = format!(
            "POST {path} HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        use std::io::Write;
        stream.write_all(req.as_bytes()).ok()?;
        let mut buf = String::new();
        stream.read_to_string(&mut buf).ok()?;
        buf.split("\r\n\r\n").nth(1).map(|s| s.to_string())
    }

    fn run_scenario(file: &str) -> bool {
        let body = format!(r#"{{"file":"{file}"}}"#);
        let resp = http_post("/run-scenario", &body).unwrap_or_default();
        let val: serde_json::Value = serde_json::from_str(&resp).unwrap_or_default();
        val["pass"].as_bool().unwrap_or(false)
    }

    struct HeadlessApp(Child);

    impl Drop for HeadlessApp {
        fn drop(&mut self) {
            let _ = self.0.kill();
        }
    }

    #[test]
    fn inspector_smoke_test() {
        let bin = binary_path();
        if !bin.exists() {
            eprintln!("[skip] binary not found at {bin:?} — run `cargo build` first");
            return;
        }

        let child = Command::new(&bin)
            .arg("--headless")
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to spawn headless binary");

        let _guard = HeadlessApp(child);

        // Wait up to 20s for the HTTP server
        assert!(
            wait_for_server(Duration::from_secs(20)),
            "inspector server did not come up on port {PORT}"
        );

        // Basic health check
        let health = http_get("/health").unwrap_or_default();
        let health_val: serde_json::Value = serde_json::from_str(&health).unwrap_or_default();
        assert_eq!(health_val["status"].as_str(), Some("ok"), "health endpoint");

        // Wait a few frames before running scenarios
        std::thread::sleep(Duration::from_millis(500));

        // Run all 5 scenarios
        let scenarios = [
            "01_health_check.json",
            "02_reset.json",
            "03_symbol_switch.json",
            "04_chart_flags.json",
            "05_watchlist_edit.json",
        ];

        let mut all_pass = true;
        for scenario in &scenarios {
            let pass = run_scenario(scenario);
            if pass {
                eprintln!("[PASS] {scenario}");
            } else {
                eprintln!("[FAIL] {scenario}");
                all_pass = false;
            }
        }

        assert!(all_pass, "one or more dev inspector scenarios failed");
    }
}

// No-op for release builds where the inspector is compiled out
#[cfg(not(debug_assertions))]
#[test]
fn inspector_not_built_in_release() {
    // This is expected — the inspector is a debug-only feature.
}
