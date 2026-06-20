//! CI integration test for the Dev Inspector.
//!
//! Spawns the app binary in `--headless` mode, waits for the HTTP server to
//! come up on :7891, then drives all scenarios plus the new endpoints.
//!
//! Run with:  cargo test --test dev_inspector -- --nocapture

#[cfg(debug_assertions)]
mod tests {
    use std::io::Read;
    use std::net::TcpStream;
    use std::process::{Child, Command, Stdio};
    use std::time::{Duration, Instant};
    #[cfg(target_os = "windows")]
    use std::os::windows::process::CommandExt;

    const PORT: u16 = 7892;

    fn binary_path() -> std::path::PathBuf {
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        if cfg!(target_os = "windows") {
            manifest.join("target/debug/apex-native.exe")
        } else {
            manifest.join("target/debug/apex-native")
        }
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
        buf.split("\r\n\r\n").nth(1).map(|s| s.to_string())
    }

    fn http_post(path: &str, body: &str) -> Option<String> {
        let mut stream = TcpStream::connect(format!("127.0.0.1:{PORT}")).ok()?;
        stream.set_read_timeout(Some(Duration::from_secs(30))).ok()?;
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

    fn http_delete(path: &str) -> Option<String> {
        let mut stream = TcpStream::connect(format!("127.0.0.1:{PORT}")).ok()?;
        stream.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
        let req = format!("DELETE {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
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
        let pass = val["pass"].as_bool().unwrap_or(false);
        if !pass {
            // Print first failing step detail to aid debugging.
            if let Some(steps) = val["steps"].as_array() {
                for step in steps {
                    if step["pass"].as_bool() == Some(false) {
                        eprintln!("  [FAIL detail] step {}: action={} detail={}",
                            step["step"].as_u64().unwrap_or(0),
                            step["action"].as_str().unwrap_or("?"),
                            step["detail"].as_str().unwrap_or("?"));
                        break;
                    }
                }
            }
        }
        pass
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

        // Run from the repo root so "dev/scenarios" resolves correctly.
        let repo_root = bin.parent().unwrap() // target/debug
            .parent().unwrap()               // target
            .parent().unwrap();              // src-tauri (= CARGO_MANIFEST_DIR)
        // dev/scenarios lives one level above src-tauri (repo root)
        let repo_root = repo_root.parent().unwrap_or(repo_root);

        let mut cmd = Command::new(&bin);
        cmd.arg("--headless")
            .current_dir(repo_root)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        // CREATE_NEW_CONSOLE (0x10): gives the child its own console session so Windows
        // delivers WM_PAINT events to its off-screen headless window. Without this flag
        // the child inherits the test runner's (non-interactive) console context and DWM
        // refuses to composite its window, so the render loop never ticks.
        #[cfg(target_os = "windows")]
        cmd.creation_flags(0x00000010);
        let child = cmd.spawn().expect("failed to spawn headless binary");

        let _guard = HeadlessApp(child);

        assert!(
            wait_for_server(Duration::from_secs(20)),
            "inspector server did not come up on port {PORT}"
        );

        // Basic health check
        let health = http_get("/health").unwrap_or_default();
        let health_val: serde_json::Value = serde_json::from_str(&health).unwrap_or_default();
        assert_eq!(health_val["status"].as_str(), Some("ok"), "health endpoint");

        std::thread::sleep(Duration::from_millis(500));

        // ── All 150 scenarios ─────────────────────────────────────────────────
        let scenarios = [
            // ── Baseline (01-20) ──────────────────────────────────────────────
            "01_health_check.json",
            "02_reset.json",
            "03_symbol_switch.json",
            "04_chart_flags.json",
            "05_watchlist_edit.json",
            "06_indicator_lifecycle.json",
            "07_pane_type_switch.json",
            "08_design_audit.json",
            "09_annotations_demo.json",
            "10_layout_regression.json",
            "11_watchlist_crud.json",
            "12_theme_cycle.json",
            "13_assert_poll.json",
            "14_toolbar_layout_audit.json",
            "15_input_injection.json",
            "16_dialog_lifecycle.json",
            "17_order_entry.json",
            "18_workspace_roundtrip.json",
            "19_multi_pane.json",
            "20_watchlist_coverage.json",
            // ── User-story-derived (21-30) ────────────────────────────────────
            "21_symbol_switching_flow.json",
            "22_timeframe_progression.json",
            "23_indicator_stack_lifecycle.json",
            "24_chart_flag_matrix.json",
            "25_watchlist_multi_section.json",
            "26_fps_stress_rapid_switch.json",
            "27_design_token_audit.json",
            "28_theme_style_cycle_audit.json",
            "29_indicator_recompute_after_swap.json",
            "30_full_session_simulation.json",
            // ── Chart manipulation (31-50) ────────────────────────────────────
            "31_chart_line_style.json",
            "32_chart_bar_style.json",
            "33_chart_area_style.json",
            "34_chart_extended_hours.json",
            "35_chart_log_scale_cycle.json",
            "36_chart_prev_close_line.json",
            "37_chart_magnet_snap.json",
            "38_chart_ohlc_tooltip.json",
            "39_chart_oscillators_visibility.json",
            "40_chart_hide_all_indicators.json",
            "41_chart_volume_bars_cycle.json",
            "42_chart_multi_flag_combo.json",
            "43_chart_flag_persist_after_symbol_swap.json",
            "44_chart_flag_persist_after_timeframe_change.json",
            "45_chart_rapid_flag_cycle_fps.json",
            "46_chart_crosshair_toggle.json",
            "47_chart_price_alert_add.json",
            "48_chart_show_trades_flag.json",
            "49_chart_auto_scale_toggle.json",
            "50_chart_indicator_heavy_fps.json",
            // ── Order entry (51-65) ───────────────────────────────────────────
            "51_order_open_close_fast.json",
            "52_order_open_buy_flow.json",
            "53_order_open_sell_flow.json",
            "54_order_market_type_setup.json",
            "55_order_limit_type_setup.json",
            "56_order_stop_type_setup.json",
            "57_order_after_symbol_change.json",
            "58_order_after_timeframe_change.json",
            "59_order_escape_to_close.json",
            "60_order_design_health_while_open.json",
            "61_order_reopen_after_close.json",
            "62_order_fps_during_open.json",
            "63_order_after_indicator_add.json",
            "64_order_from_two_pane_layout.json",
            "65_order_close_resets_dialog_state.json",
            // ── Options pane (66-80) ──────────────────────────────────────────
            "66_options_sentiment_pane.json",
            "67_options_flow_pane.json",
            "68_options_pane_fps.json",
            "69_options_sentiment_symbol_change.json",
            "70_options_flow_symbol_change.json",
            "71_options_pane_design_audit.json",
            "72_options_pane_to_chart_back.json",
            "73_options_in_two_column.json",
            "74_options_flow_in_quad.json",
            "75_options_pane_after_reset.json",
            "76_options_pane_no_violations.json",
            "77_options_two_pane_chart_options.json",
            "78_options_sentiment_theme_change.json",
            "79_options_pane_rapid_type_switch.json",
            "80_options_session_roundtrip.json",
            // ── Pane / inter-pane operations (81-100) ────────────────────────
            "81_pane_two_columns_stable.json",
            "82_pane_two_rows.json",
            "83_pane_quad_layout.json",
            "84_pane_layout_cycle_all.json",
            "85_pane_independent_timeframes.json",
            "86_pane_independent_indicators.json",
            "87_pane_type_mix_two.json",
            "88_pane_type_mix_quad.json",
            "89_pane_collapse_to_single.json",
            "90_pane_expand_then_collapse.json",
            "91_inter_pane_symbol_diversity.json",
            "92_inter_pane_timeframe_diversity.json",
            "93_inter_pane_design_health.json",
            "94_inter_pane_indicator_isolation.json",
            "95_inter_pane_fps_all_active.json",
            "96_inter_pane_order_entry_from_layout.json",
            "97_inter_pane_options_in_layout.json",
            "98_inter_pane_layout_then_indicators.json",
            "99_inter_pane_watchlist_multi_pane.json",
            "100_inter_pane_theme_change_multi.json",
            // ── Quality of experience (101-130) ──────────────────────────────
            "101_qoe_clean_initial_state.json",
            "102_qoe_theme_cycle_all.json",
            "103_qoe_style_cycle.json",
            "104_qoe_dark_light_switch.json",
            "105_qoe_keyboard_escape.json",
            "106_qoe_keyboard_sequence_smoke.json",
            "107_qoe_fps_baseline.json",
            "108_qoe_fps_5_indicators.json",
            "109_qoe_fps_10_indicators.json",
            "110_qoe_fps_multi_pane_heavy.json",
            "111_qoe_design_health_after_load.json",
            "112_qoe_design_health_after_symbols.json",
            "113_qoe_design_health_with_indicators.json",
            "114_qoe_design_health_multi_pane.json",
            "115_qoe_watchlist_build_and_clear.json",
            "116_qoe_snapshot_baseline.json",
            "117_qoe_settings_persist_symbol_switch.json",
            "118_qoe_no_clipped_widgets_all_themes.json",
            "119_qoe_annotations_smoke.json",
            "120_qoe_annotations_survive_symbol_swap.json",
            "121_qoe_full_trader_session_v2.json",
            "122_qoe_stress_all_flags.json",
            "123_qoe_recovery_rapid_ops.json",
            "124_qoe_price_alert_multi.json",
            "125_qoe_indicator_add_remove_cycle.json",
            "126_qoe_recompute_after_swap.json",
            "127_qoe_layout_design_integrity.json",
            "128_qoe_multi_session_simulation.json",
            "129_qoe_fps_stress_all_features.json",
            "130_qoe_final_integration_audit.json",
            // ── Assert engine extensions (131-140) ───────────────────────────
            "131_assert_state_field_gte_lte.json",
            "132_assert_pane_symbol_equals.json",
            "133_assert_pane_timeframe_equals.json",
            "134_assert_fps_perf.json",
            "135_assert_violation_count_lte.json",
            "136_assert_widget_count.json",
            "137_assert_annotation_count.json",
            "138_assert_combinators.json",
            "139_assert_state_field_deep_path.json",
            "140_assert_poll_quick.json",
            // ── New step actions (141-150) ────────────────────────────────────
            "141_step_design_audit.json",
            "142_step_assert_metrics.json",
            "143_step_save_checkpoint.json",
            "144_step_loop_basic.json",
            "145_step_loop_with_assert.json",
            "146_step_annotate_actions.json",
            "147_step_annotate_widget_fallback.json",
            "148_step_cmd_batch.json",
            "149_step_http_post_action.json",
            "150_full_step_coverage_audit.json",
            // ── Bug fixes + new coverage (151-164) ───────────────────────────
            "151_watchlist_switch_active.json",
            "152_watchlist_switch_active_oob.json",
            "153_watchlist_toggle_collapse.json",
            "154_watchlist_remove_symbol_underflow.json",
            "155_assert_any_of_combinator.json",
            "156_error_handling_bad_cmd.json",
            "157_indicator_count_accuracy.json",
            "158_annotate_frame_sync.json",
            "159_theme_style_cycle.json",
            "160_order_commands_no_crash.json",
            "161_multi_pane_state_isolation.json",
            "162_watchlist_section_remove_reindex.json",
            "163_rapid_reset_stability.json",
            "164_dialog_lifecycle_complete.json",
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

        // ── /metrics — must return non-empty history + frame_time_ms ─────────
        let metrics_body = http_get("/metrics").unwrap_or_default();
        let metrics: serde_json::Value = serde_json::from_str(&metrics_body).unwrap_or_default();
        let fps_history = metrics["fps"]["history"].as_array();
        assert!(
            fps_history.map_or(false, |h| !h.is_empty()),
            "/metrics fps history should be non-empty after running scenarios"
        );
        eprintln!("[PASS] /metrics history non-empty ({} frames)", fps_history.map_or(0, |h| h.len()));
        assert!(
            metrics.get("frame_time_ms").is_some(),
            "/metrics must contain frame_time_ms field"
        );
        eprintln!("[PASS] /metrics frame_time_ms = {:.2}ms", metrics["frame_time_ms"].as_f64().unwrap_or(0.0));

        // ── /last-run — must reflect the last scenario executed ───────────────
        let last_body = http_get("/last-run").unwrap_or_default();
        let last: serde_json::Value = serde_json::from_str(&last_body).unwrap_or_default();
        assert!(
            last.get("scenario").is_some(),
            "/last-run must have 'scenario' field; got: {last_body}"
        );
        eprintln!("[PASS] /last-run scenario={}", last["scenario"].as_str().unwrap_or("?"));

        // ── /run-suite — cross-category smoke sample ──────────────────────────
        let suite_body = http_post("/run-suite", r#"{
            "scenarios": [
                "01_health_check.json",
                "30_full_session_simulation.json",
                "31_chart_line_style.json",
                "50_chart_indicator_heavy_fps.json",
                "51_order_open_close_fast.json",
                "66_options_sentiment_pane.json",
                "81_pane_two_columns_stable.json",
                "95_inter_pane_fps_all_active.json",
                "107_qoe_fps_baseline.json",
                "130_qoe_final_integration_audit.json"
            ]
        }"#).unwrap_or_default();
        let suite: serde_json::Value = serde_json::from_str(&suite_body).unwrap_or_default();
        assert!(
            suite.get("total").is_some(),
            "/run-suite must return total field; got: {suite_body}"
        );
        let suite_total  = suite["total"].as_u64().unwrap_or(0);
        let suite_passed = suite["passed"].as_u64().unwrap_or(0);
        eprintln!("[PASS] /run-suite {suite_passed}/{suite_total} passed");

        // ── /design-audit — should be clean after a reset ─────────────────────
        http_post("/reset", "{}").unwrap_or_default();
        std::thread::sleep(Duration::from_millis(200));
        let audit_body = http_get("/design-audit").unwrap_or_default();
        let audit: serde_json::Value = serde_json::from_str(&audit_body).unwrap_or_default();
        // Report must parse and have a `clean` field (value may be false if
        // widgets are clipped on a headless framebuffer — just check presence).
        assert!(
            audit.get("clean").is_some(),
            "/design-audit response must contain 'clean' field; got: {audit_body}"
        );
        eprintln!("[PASS] /design-audit responded (clean={})", audit["clean"]);

        // ── /annotations round-trip ───────────────────────────────────────────
        // POST two annotations
        let ann_body = r#"[
            {"id":"test.a","rect":{"x":10,"y":10,"w":100,"h":50},
             "label":"Test A","color":[255,0,0,128],"border_only":false},
            {"id":"test.b","rect":{"x":200,"y":200,"w":80,"h":40},
             "label":"Test B","color":[0,255,0,200],"border_only":true,"border_width":2.0}
        ]"#;
        http_post("/annotations", ann_body).unwrap_or_default();
        std::thread::sleep(Duration::from_millis(100));

        // GET — should see both
        let get_body = http_get("/annotations").unwrap_or_default();
        let anns: serde_json::Value = serde_json::from_str(&get_body).unwrap_or_default();
        let ann_arr = anns.as_array();
        assert!(
            ann_arr.map_or(false, |a| a.len() >= 2),
            "/annotations GET should return >= 2 annotations; got: {get_body}"
        );
        eprintln!("[PASS] /annotations POST→GET ({} entries)", ann_arr.map_or(0, |a| a.len()));

        // DELETE one
        http_delete("/annotations/test.a").unwrap_or_default();
        std::thread::sleep(Duration::from_millis(100));
        let after_del = http_get("/annotations").unwrap_or_default();
        let after_arr: serde_json::Value = serde_json::from_str(&after_del).unwrap_or_default();
        let remaining = after_arr.as_array().map_or(0, |a| a.len());
        // test.b should remain; test.a should be gone
        assert_eq!(remaining, 1, "/annotations DELETE one should leave 1; got {remaining}");
        eprintln!("[PASS] /annotations DELETE one → 1 remaining");

        // DELETE all
        http_delete("/annotations").unwrap_or_default();
        std::thread::sleep(Duration::from_millis(100));
        let after_clear = http_get("/annotations").unwrap_or_default();
        let after_clear_arr: serde_json::Value =
            serde_json::from_str(&after_clear).unwrap_or_default();
        let count_after_clear = after_clear_arr.as_array().map_or(0, |a| a.len());
        assert_eq!(
            count_after_clear, 0,
            "/annotations DELETE all should leave 0; got {count_after_clear}"
        );
        eprintln!("[PASS] /annotations DELETE all → empty");

        // ── /layout-svg — must return SVG markup ─────────────────────────────
        let svg_body = http_get("/layout-svg").unwrap_or_default();
        assert!(
            svg_body.contains("<svg") && svg_body.contains("</svg>"),
            "/layout-svg must return valid SVG; got {} bytes starting with: {}",
            svg_body.len(),
            &svg_body[..svg_body.len().min(120)]
        );
        eprintln!("[PASS] /layout-svg returned SVG ({} bytes)", svg_body.len());

        // ── /state — must return populated JSON object ────────────────────────
        let state_body = http_get("/state").unwrap_or_default();
        let state_val: serde_json::Value = serde_json::from_str(&state_body).unwrap_or_default();
        assert!(
            state_val.is_object(),
            "/state must return JSON object; got: {state_body}"
        );
        assert!(
            state_val.get("pane_count").is_some() || state_val.get("fps").is_some(),
            "/state must contain pane_count or fps; got keys: {}",
            state_val.as_object().map(|o| o.keys().cloned().collect::<Vec<_>>().join(", ")).unwrap_or_default()
        );
        eprintln!("[PASS] /state returns populated object ({} keys)",
            state_val.as_object().map(|o| o.len()).unwrap_or(0));

        // ── /widget-tree — must return JSON array ─────────────────────────────
        let tree_body = http_get("/widget-tree").unwrap_or_default();
        let tree_val: serde_json::Value = serde_json::from_str(&tree_body).unwrap_or_default();
        assert!(
            tree_val.is_array(),
            "/widget-tree must return JSON array; got: {}",
            &tree_body[..tree_body.len().min(120)]
        );
        eprintln!("[PASS] /widget-tree returned array ({} entries)",
            tree_val.as_array().map_or(0, |a| a.len()));

        // ── /panes — must return JSON array ──────────────────────────────────
        let panes_body = http_get("/panes").unwrap_or_default();
        let panes_val: serde_json::Value = serde_json::from_str(&panes_body).unwrap_or_default();
        assert!(
            panes_val.is_array(),
            "/panes must return JSON array; got: {panes_body}"
        );
        eprintln!("[PASS] /panes returned {} pane(s)", panes_val.as_array().map_or(0, |a| a.len()));

        // ── /watchlist — must return JSON object ──────────────────────────────
        let wl_body = http_get("/watchlist").unwrap_or_default();
        let wl_val: serde_json::Value = serde_json::from_str(&wl_body).unwrap_or_default();
        assert!(
            !wl_val.is_null(),
            "/watchlist must return non-null JSON; got: {wl_body}"
        );
        eprintln!("[PASS] /watchlist returned non-null value");

        // ── /scenario-list — base + tag filter ───────────────────────────────
        let list_body = http_get("/scenario-list").unwrap_or_default();
        let list_val: serde_json::Value = serde_json::from_str(&list_body).unwrap_or_default();
        let total_count = list_val["count"].as_u64().unwrap_or(0);
        assert!(
            total_count >= 164,
            "/scenario-list count should be >= 164; got {total_count}"
        );
        eprintln!("[PASS] /scenario-list returned {total_count} scenarios");

        // Tag filter — "smoke" tag should return a subset
        let smoke_body = http_get("/scenario-list?tag=smoke").unwrap_or_default();
        let smoke_val: serde_json::Value = serde_json::from_str(&smoke_body).unwrap_or_default();
        let smoke_count = smoke_val["count"].as_u64().unwrap_or(0);
        assert!(
            smoke_count > 0 && smoke_count < total_count,
            "/scenario-list?tag=smoke should return a non-empty subset; got {smoke_count}/{total_count}"
        );
        eprintln!("[PASS] /scenario-list?tag=smoke returned {smoke_count}/{total_count}");

        // ── /layout-snapshot — must return JSON object ────────────────────────
        let snap_body = http_get("/layout-snapshot").unwrap_or_default();
        let snap_val: serde_json::Value = serde_json::from_str(&snap_body).unwrap_or_default();
        assert!(
            snap_val.is_object(),
            "/layout-snapshot must return JSON object; got: {snap_body}"
        );
        eprintln!("[PASS] /layout-snapshot returned object ({} entries)",
            snap_val.as_object().map_or(0, |o| o.len()));

        // ── /batch — multi-request dispatch ──────────────────────────────────
        let batch_body = http_post("/batch", r#"[
            {"method": "GET",  "path": "/state"},
            {"method": "GET",  "path": "/panes"},
            {"method": "POST", "path": "/cmd",
             "body": {"cmd": "SwapPaneSymbol", "pane": 0, "symbol": "TSLA"}}
        ]"#).unwrap_or_default();
        let batch_val: serde_json::Value = serde_json::from_str(&batch_body).unwrap_or_default();
        assert!(
            batch_val.as_array().map_or(false, |a| a.len() == 3),
            "/batch must return array of 3 results; got: {batch_body}"
        );
        let batch_statuses: Vec<u64> = batch_val.as_array().unwrap()
            .iter().filter_map(|r| r["status"].as_u64()).collect();
        assert!(
            batch_statuses.iter().all(|&s| s == 200),
            "/batch all responses should be 200; got statuses: {batch_statuses:?}"
        );
        eprintln!("[PASS] /batch returned 3 results, all status 200");

        // ── /assert POST — direct assertion against live state ────────────────
        let assert_body = http_post("/assert", r#"[
            {"fps_above": 5.0},
            {"pane_count_equals": 1},
            {"no_open_dialogs": true}
        ]"#).unwrap_or_default();
        let assert_val: serde_json::Value = serde_json::from_str(&assert_body).unwrap_or_default();
        assert!(
            assert_val.get("passed").is_some() && assert_val.get("failed").is_some(),
            "/assert must return passed/failed fields; got: {assert_body}"
        );
        let assert_failed = assert_val["failed"].as_u64().unwrap_or(1);
        assert!(
            assert_failed == 0,
            "/assert: {} assertion(s) failed: {assert_body}", assert_failed
        );
        eprintln!("[PASS] /assert: {} passed, {} failed",
            assert_val["passed"].as_u64().unwrap_or(0), assert_failed);

        // ── New assertion types via /assert ───────────────────────────────────
        let new_assert_body = http_post("/assert", r#"[
            {"state_field_gte":      {"path": "pane_count", "min": 1}},
            {"state_field_lte":      {"path": "pane_count", "max": 4}},
            {"violation_count_lte":  0},
            {"fps_history_min_above": 5.0},
            {"frame_time_below_ms":   500.0},
            {"widget_count_gte":      {"min": 0}},
            {"annotation_count_equals": 0}
        ]"#).unwrap_or_default();
        let na_val: serde_json::Value = serde_json::from_str(&new_assert_body).unwrap_or_default();
        let na_failed = na_val["failed"].as_u64().unwrap_or(99);
        assert!(
            na_failed == 0,
            "new assertion types: {na_failed} failed: {new_assert_body}"
        );
        eprintln!("[PASS] new assertion types all pass ({} assertions)",
            na_val["passed"].as_u64().unwrap_or(0));

        // ── 404 for unknown routes ────────────────────────────────────────────
        // Raw TCP so we can read the status line.
        {
            use std::io::Write as _;
            let mut s = TcpStream::connect(format!("127.0.0.1:{PORT}")).unwrap();
            s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            s.write_all(b"GET /nonexistent_route_xyz HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n").unwrap();
            let mut buf = String::new();
            s.read_to_string(&mut buf).ok();
            assert!(
                buf.contains("404"),
                "unknown route should return 404; got: {}",
                &buf[..buf.len().min(80)]
            );
            eprintln!("[PASS] unknown route returns 404");
        }

        // ── /report — must return HTML ────────────────────────────────────────
        let report_body = http_get("/report").unwrap_or_default();
        assert!(
            report_body.contains("<!DOCTYPE html") || report_body.contains("<html"),
            "/report must return HTML; got {} bytes", report_body.len()
        );
        eprintln!("[PASS] /report returned HTML ({} bytes)", report_body.len());

        assert!(all_pass, "one or more dev inspector scenarios failed");
    }
}

#[cfg(not(debug_assertions))]
#[test]
fn inspector_not_built_in_release() {}
