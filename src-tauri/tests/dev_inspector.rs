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
        // DETACHED_PROCESS (0x08): detach from the test runner's console so the child
        // gets its own console context. CREATE_NEW_CONSOLE conflicts with Stdio::null()
        // on some Windows versions, causing the process to exit before binding the port.
        // In --headless mode the ticker thread drives frames; DWM events are not needed.
        #[cfg(target_os = "windows")]
        cmd.creation_flags(0x00000008);
        let child = cmd.spawn().expect("failed to spawn headless binary");

        let _guard = HeadlessApp(child);

        assert!(
            wait_for_server(Duration::from_secs(150)),
            "inspector server did not come up on port {PORT}"
        );

        // Basic health check
        let health = http_get("/health").unwrap_or_default();
        let health_val: serde_json::Value = serde_json::from_str(&health).unwrap_or_default();
        assert_eq!(health_val["status"].as_str(), Some("ok"), "health endpoint");

        std::thread::sleep(Duration::from_millis(500));

        // ── All 420 scenarios ─────────────────────────────────────────────────
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
            // ── Extension: new step actions + assertions (165-200) ────────────
            "165_for_each_symbol_matrix.json",
            "166_capture_pane_state.json",
            "167_retry_stable_fps.json",
            "168_include_shared_steps.json",
            "169_expect_fail_bad_assert.json",
            "170_annotation_id_exists.json",
            "171_open_dialog_count.json",
            "172_watchlist_section_title.json",
            "173_state_field_contains.json",
            "174_state_field_not_null.json",
            "175_captures_roundtrip.json",
            "176_for_each_timeframes.json",
            "177_for_each_indicators.json",
            "178_capture_multi_field.json",
            "179_retry_with_delay.json",
            "180_expect_fail_cmd_error.json",
            "181_story_chart_regression.json",
            "182_story_watchlist_journey.json",
            "183_story_order_entry_journey.json",
            "184_story_pane_management.json",
            "185_story_design_quality.json",
            "186_for_each_watchlist_sections.json",
            "187_capture_annotation_baseline.json",
            "188_story_recovery_sequence.json",
            "189_story_performance_capture.json",
            "190_new_assertions_combined.json",
            "191_for_each_pane_types.json",
            "192_retry_reset_stability.json",
            "193_expect_fail_wrong_symbol.json",
            "194_include_nested.json",
            "195_for_each_symbols_capture.json",
            "196_story_usability_keyboard.json",
            "197_story_usability_watchlist.json",
            "198_story_theming_full.json",
            "199_story_stability_session.json",
            "200_comprehensive_extension_audit.json",
            "201_dialog_headless_commands.json",
            "202_multi_pane_dialog_isolation.json",
            "203_reset_clears_dialogs.json",
            "204_state_field_new_assertions.json",
            "205_dialog_survives_symbol_change.json",
            "206_for_each_dialog_open_close.json",
            "207_capture_dialog_state.json",
            "208_escape_key_clears_all.json",
            "209_pane_count_via_layout.json",
            "210_for_each_pane_symbols.json",
            "211_story_performance_regression.json",
            "212_story_full_trader_session.json",
            "213_retry_with_dialog_assert.json",
            "214_new_assertions_audit.json",
            "215_story_watchlist_and_orders.json",
            // ── Canvas introspection (216-228) ────────────────────────────────
            "216_canvas_initial_state.json",
            "217_canvas_drawing_add_remove.json",
            "218_canvas_drawing_lifecycle.json",
            "219_canvas_viewport_assertions.json",
            "220_canvas_indicator_output.json",
            "221_canvas_indicator_near.json",
            "222_canvas_bars_valid.json",
            "223_canvas_multi_drawing_types.json",
            "224_canvas_drawing_anchor_price.json",
            "225_canvas_pane_viewport_density.json",
            "226_canvas_indicator_multi_pane.json",
            "227_canvas_reset_clears_drawings.json",
            "228_canvas_full_story.json",
            // ── Canvas extended (229-278) ─────────────────────────────────────
            "229_canvas_five_drawings_count.json",
            "230_canvas_hzone_kind.json",
            "231_canvas_trendline_anchors.json",
            "232_canvas_hzone_anchors.json",
            "233_canvas_mixed_kind_counts.json",
            "234_canvas_drawing_removal_verify.json",
            "235_canvas_clear_drawings_cmd.json",
            "236_canvas_drawing_trendline_count.json",
            "237_canvas_drawing_survives_viewport.json",
            "238_canvas_ten_drawings_count.json",
            "239_canvas_viewport_narrow.json",
            "240_canvas_viewport_wide.json",
            "241_canvas_viewport_boundary_low.json",
            "242_canvas_viewport_boundary_high.json",
            "243_canvas_viewport_px_per_bar_baseline.json",
            "244_canvas_viewport_sequential_changes.json",
            "245_canvas_viewport_bars_still_valid.json",
            "246_canvas_viewport_very_wide.json",
            "247_canvas_indicator_sma_near.json",
            "248_canvas_indicator_ema.json",
            "249_canvas_indicator_rsi_bounds.json",
            "250_canvas_indicator_vwap.json",
            "251_canvas_indicator_overwrite.json",
            "252_canvas_indicator_two_kinds.json",
            "253_canvas_indicator_three_kinds.json",
            "254_canvas_indicator_clear.json",
            "255_canvas_indicator_zero_value.json",
            "256_canvas_indicator_large_value.json",
            "257_canvas_two_pane_drawing_isolation.json",
            "258_canvas_three_pane_layout.json",
            "259_canvas_four_pane_layout.json",
            "260_canvas_pane_layout_shrink.json",
            "261_canvas_two_pane_different_viewports.json",
            "262_canvas_two_pane_indicator_isolation.json",
            "263_canvas_drawing_not_exists_after_remove.json",
            "264_canvas_no_indicators_initial.json",
            "265_canvas_indicator_exists_after_set.json",
            "266_canvas_indicator_count_two.json",
            "267_canvas_drawing_anchor_count_horizline.json",
            "268_canvas_drawing_anchor_count_trendline.json",
            "269_canvas_bars_count_gte_10.json",
            "270_canvas_bars_ohlcv_wide_viewport.json",
            "271_canvas_bars_volume_nonneg.json",
            "272_canvas_bars_valid_two_panes.json",
            "273_canvas_bars_after_drawing_added.json",
            "274_story_support_resistance.json",
            "275_story_indicator_lifecycle.json",
            "276_story_drawing_management.json",
            "277_story_two_pane_trading_view.json",
            "278_story_canvas_full_regression.json",
            // ── DrawingKind coverage (279-297) ────────────────────────────────
            "279_canvas_drawing_kind_fibonacci.json",
            "280_canvas_drawing_kind_channel.json",
            "281_canvas_drawing_kind_fibchannel.json",
            "282_canvas_drawing_kind_pitchfork.json",
            "283_canvas_drawing_kind_gannfan.json",
            "284_canvas_drawing_kind_gannbox.json",
            "285_canvas_drawing_kind_regressionchannel.json",
            "286_canvas_drawing_kind_xabcd.json",
            "287_canvas_drawing_kind_elliottwave.json",
            "288_canvas_drawing_kind_anchoredvwap.json",
            "289_canvas_drawing_kind_pricerange.json",
            "290_canvas_drawing_kind_riskreward.json",
            "291_canvas_drawing_kind_verticalline.json",
            "292_canvas_drawing_kind_ray.json",
            "293_canvas_drawing_kind_fibextension.json",
            "294_canvas_drawing_kind_fibtimezone.json",
            "295_canvas_drawing_kind_fibarc.json",
            "296_canvas_drawing_kind_barmarker.json",
            "297_canvas_drawing_kind_textnote.json",
            // ── Indicator multi-series (298-304) ──────────────────────────────
            "298_canvas_indicator_macd.json",
            "299_canvas_indicator_bollingerbands.json",
            "300_canvas_indicator_atr.json",
            "301_canvas_indicator_stochastic.json",
            "302_canvas_indicator_cci.json",
            "303_canvas_indicator_obv.json",
            "304_canvas_indicator_williamsr.json",
            // ── Layout/System (305-316) ───────────────────────────────────────
            "305_layout_single_pane_stable.json",
            "306_layout_two_rows_stable.json",
            "307_layout_three_columns_stable.json",
            "308_layout_quad_stable.json",
            "309_layout_cycle_ascending.json",
            "310_layout_cycle_descending.json",
            "311_layout_two_pane_symbols.json",
            "312_layout_quad_four_symbols.json",
            "313_layout_single_no_clip.json",
            "314_layout_two_no_clip.json",
            "315_layout_quad_no_clip.json",
            "316_layout_initial_design_audit.json",
            // ── PaneType/Switching (317-323) ─────────────────────────────────
            "317_panetype_portfolio.json",
            "318_panetype_dashboard.json",
            "319_panetype_heatmap.json",
            "320_panetype_spreadsheet.json",
            "321_panetype_back_to_chart.json",
            "322_panetype_two_pane_mixed.json",
            "323_panetype_quad_all_types.json",
            // ── Timeframe/Switching (324-332) ────────────────────────────────
            "324_timeframe_1m.json",
            "325_timeframe_5m.json",
            "326_timeframe_15m.json",
            "327_timeframe_30m.json",
            "328_timeframe_1h.json",
            "329_timeframe_4h.json",
            "330_timeframe_1d.json",
            "331_timeframe_1wk.json",
            "332_timeframe_two_pane_independent.json",
            // ── Indicator/Lifecycle (333-346) ────────────────────────────────
            "333_indicator_add_remove_rsi.json",
            "334_indicator_add_two.json",
            "335_indicator_add_three.json",
            "336_indicator_per_pane_isolated.json",
            "337_indicator_vwap.json",
            "338_indicator_wma.json",
            "339_indicator_ema.json",
            "340_indicator_ichimoku.json",
            "341_indicator_parabolicsar.json",
            "342_indicator_supertrend.json",
            "343_indicator_adx.json",
            "344_indicator_atr.json",
            "345_indicator_cci.json",
            "346_indicator_obv.json",
            // ── Watchlist/CRUD (347-360) ─────────────────────────────────────
            "347_watchlist_add_symbol.json",
            "348_watchlist_add_remove_symbol.json",
            "349_watchlist_add_multiple_symbols.json",
            "350_watchlist_add_section.json",
            "351_watchlist_remove_section.json",
            "352_watchlist_section_count_cycle.json",
            "353_watchlist_create_new.json",
            "354_watchlist_switch_active.json",
            "355_watchlist_delete.json",
            "356_watchlist_rename_active.json",
            "357_watchlist_toggle_collapse.json",
            "358_watchlist_preserved_after_layout.json",
            "359_watchlist_add_option_section.json",
            "360_watchlist_preserved_after_symbol_swap.json",
            // ── Dialog/Lifecycle (361-370) ───────────────────────────────────
            "361_dialog_hotkey_editor_open.json",
            "362_dialog_reset_closes_all.json",
            "363_dialog_order-entry_open.json",
            "364_dialog_order-entry_close.json",
            "365_dialog_settings_open.json",
            "366_dialog_settings_close.json",
            "367_dialog_orders-panel_open.json",
            "368_dialog_orders-panel_close.json",
            "369_dialog_multiple_open.json",
            "370_dialog_close_all.json",
            // ── ChartFlag/Settings (371-382) ─────────────────────────────────
            "371_chartflag_showvolume_on.json",
            "372_chartflag_showvolume_off.json",
            "373_chartflag_logscale_on.json",
            "374_chartflag_logscale_off.json",
            "375_chartflag_showoscillators_on.json",
            "376_chartflag_showoscillators_off.json",
            "377_chartflag_showprevclose_on.json",
            "378_chartflag_magnet_on.json",
            "379_chartflag_ohlctooltip_on.json",
            "380_chartflag_hideallindicators_on.json",
            "381_chartflag_hidealldrawings_on.json",
            "382_chartflag_hide_drawings_then_show.json",
            // ── Canvas/Integrity (383-390) ────────────────────────────────────
            "383_canvas_ohlc_valid_after_swap.json",
            "384_canvas_visible_bars_min.json",
            "385_canvas_two_pane_ohlc_valid.json",
            "386_canvas_pane_count_matches_layout.json",
            "387_canvas_rsi_value_in_range.json",
            "388_canvas_drawing_cleared_on_reset.json",
            "389_canvas_ohlc_valid_after_tf_change.json",
            "390_canvas_indicator_count_matches.json",
            "391_pane_indicator_isolation_basic.json",
            "392_pane_indicator_isolation_cross.json",
            "393_pane_indicator_remove_isolation.json",
            "394_pane_indicator_four_pane.json",
            "395_pane_symbol_isolation.json",
            "396_pane_timeframe_isolation.json",
            "397_pane_type_isolation.json",
            "398_rapid_symbol_swap.json",
            "399_rapid_timeframe_cycle.json",
            "400_rapid_layout_cycle.json",
            "401_rapid_indicator_add_remove.json",
            "402_rapid_dialog_cycle.json",
            "403_stress_combined_state.json",
            "404_reset_clears_indicators.json",
            "405_reset_clears_all_dialogs.json",
            "406_reset_collapses_layout.json",
            "407_reset_clears_watchlist_sections.json",
            "408_reset_clears_ohlc_validity.json",
            "409_dialog_survives_layout_change.json",
            "410_dialog_survives_symbol_swap.json",
            "411_dialog_survives_indicator_add.json",
            "412_two_dialogs_survive_layout.json",
            "413_close_all_clears_every_dialog.json",
            "414_escape_key_closes_dialogs.json",
            "415_symbol_preserved_after_pane_type_change.json",
            "416_two_pane_ohlc_independence.json",
            "417_indicator_survives_symbol_swap.json",
            "418_indicator_survives_tf_change.json",
            "419_layout_shrink_with_indicators.json",
            "420_full_multi_pane_session.json",
            // ── Options chains / order management / DOM (421-440) ─────────────
            "421_options_chain_open_close.json",
            "422_options_flow_pane_switch.json",
            "423_options_pane_symbol_change.json",
            "424_options_pane_in_two_pane.json",
            "425_options_fps_stability.json",
            "426_order_entry_open_close_safe.json",
            "427_order_entry_after_symbol_change.json",
            "428_order_entry_reset_clears.json",
            "429_order_entry_multi_pane.json",
            "430_order_entry_and_orders_panel.json",
            "431_orders_panel_open_close.json",
            "432_orders_panel_after_layout_change.json",
            "433_cancel_all_orders_safe.json",
            "434_clear_order_history_safe.json",
            "435_chart_price_alert_add.json",
            "436_chart_price_alert_multi.json",
            "437_order_entry_escape_closes.json",
            "438_options_chart_combo.json",
            "439_order_management_full_noop_flow.json",
            "440_trading_ui_stress.json",
            // ── DOM / Drawings / Indicators / Viewport / Watchlist (441-450) ──
            "441_dom_state_reset_on_symbol_change.json",
            "442_chart_drawing_horiz_line.json",
            "443_chart_drawing_multi_clear.json",
            "444_indicator_add_clear.json",
            "445_viewport_set_price_range.json",
            "446_two_pane_symbol_independence.json",
            "447_layout_cycle_pane_counts.json",
            "448_watchlist_add_remove.json",
            "449_options_pane_rapid_symbol_cycle.json",
            "450_comprehensive_session_no_order_submit.json",
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
        // Reset to a clean single-pane baseline before running the suite so that
        // multi-pane scenarios (81, 95) don't inherit state from the 304-scenario loop.
        http_post("/reset", "{}").unwrap_or_default();
        std::thread::sleep(Duration::from_millis(300));
        let suite_body = http_post("/run-suite", r#"{
            "scenarios": [
                "01_health_check.json",
                "30_full_session_simulation.json",
                "31_chart_line_style.json",
                "50_chart_indicator_heavy_fps.json",
                "51_order_open_close_fast.json",
                "66_options_sentiment_pane.json",
                "107_qoe_fps_baseline.json",
                "130_qoe_final_integration_audit.json",
                "216_canvas_initial_state.json",
                "278_story_canvas_full_regression.json"
            ]
        }"#).unwrap_or_default();
        let suite: serde_json::Value = serde_json::from_str(&suite_body).unwrap_or_default();
        assert!(
            suite.get("total").is_some(),
            "/run-suite must return total field; got: {suite_body}"
        );
        let suite_total  = suite["total"].as_u64().unwrap_or(0);
        let suite_passed = suite["passed"].as_u64().unwrap_or(0);
        if let Some(results) = suite["results"].as_array() {
            for r in results {
                if r["pass"].as_bool() != Some(true) {
                    eprintln!("  [run-suite FAIL] {}: {}",
                        r["scenario"].as_str().unwrap_or("?"),
                        r["steps"].as_array()
                            .and_then(|steps| steps.iter().find(|s| s["pass"].as_bool() == Some(false)))
                            .and_then(|s| s["detail"].as_str())
                            .unwrap_or("(no detail)"));
                }
            }
        }
        assert!(
            suite_passed == suite_total,
            "/run-suite {suite_passed}/{suite_total} passed — {} scenario(s) failed",
            suite_total.saturating_sub(suite_passed)
        );
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

        // ── /canvas — must return snapshot with panes array ───────────────────
        http_post("/reset", "").unwrap_or_default();
        std::thread::sleep(Duration::from_millis(50));
        let canvas_body = http_get("/canvas").unwrap_or_default();
        let canvas_val: serde_json::Value = serde_json::from_str(&canvas_body).unwrap_or_default();
        assert!(
            canvas_val.is_object(),
            "/canvas must return JSON object; got: {canvas_body}"
        );
        let canvas_panes = canvas_val["panes"].as_array();
        assert!(
            canvas_panes.map_or(false, |p| !p.is_empty()),
            "/canvas must have non-empty 'panes' array; got: {canvas_body}"
        );
        let pane0 = canvas_panes.and_then(|p| p.first());
        assert!(
            pane0.and_then(|p| p.get("viewport")).is_some(),
            "/canvas pane[0] must have 'viewport' field"
        );
        assert!(
            pane0.and_then(|p| p.get("bars")).is_some(),
            "/canvas pane[0] must have 'bars' field"
        );
        assert!(
            pane0.and_then(|p| p.get("drawings")).is_some(),
            "/canvas pane[0] must have 'drawings' field"
        );
        eprintln!("[PASS] /canvas returned snapshot with {} pane(s)",
            canvas_panes.map_or(0, |p| p.len()));

        // ── /scenario-list — base + tag filter ───────────────────────────────
        let list_body = http_get("/scenario-list").unwrap_or_default();
        let list_val: serde_json::Value = serde_json::from_str(&list_body).unwrap_or_default();
        let total_count = list_val["count"].as_u64().unwrap_or(0);
        assert!(
            total_count >= 450,
            "/scenario-list count should be >= 450; got {total_count}"
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

        // Reset to baseline before checking symbol-dependent assertions.
        http_post("/reset", "");
        std::thread::sleep(Duration::from_millis(50));

        // ── New assertion types via /assert ───────────────────────────────────
        let new_assert_body = http_post("/assert", r#"[
            {"state_field_gte":      {"path": "pane_count", "min": 1}},
            {"state_field_lte":      {"path": "pane_count", "max": 4}},
            {"violation_count_lte":  0},
            {"fps_history_min_above": 5.0},
            {"frame_time_below_ms":   500.0},
            {"widget_count_gte":      {"min": 0}},
            {"annotation_count_equals": 0},
            {"state_field_equals":        {"path": "active_symbol",   "value": "SPY"}},
            {"state_field_equals":        {"path": "pane_count",      "value": 1}},
            {"state_field_length_equals": {"path": "panes",           "length": 1}},
            {"state_field_length_equals": {"path": "open_dialogs",    "length": 0}},
            {"state_field_length_equals": {"path": "active_symbol",   "length": 3}}
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

        // ── /captures — round-trip via scenario run ───────────────────────────
        // Run a scenario that uses capture, then read /captures
        http_post("/run-scenario", r#"{"file":"175_captures_roundtrip.json"}"#)
            .unwrap_or_default();
        // Captures are cleared per-scenario, so after run they are empty
        // (cleared at start of next run); read them directly after scenario:
        // Actually, captures persist after the scenario ends — they're only cleared
        // at the START of the next run_scenario call. So they should have values.
        // Wait: the scenario resets at the end (reset step), but captures are not
        // cleared by reset — only by starting a new scenario. So they persist.
        let cap_body = http_get("/captures").unwrap_or_default();
        let cap_val: serde_json::Value = serde_json::from_str(&cap_body).unwrap_or_default();
        assert!(
            cap_val.is_object(),
            "/captures must return JSON object; got: {cap_body}"
        );
        eprintln!("[PASS] /captures returned object ({} key(s))",
            cap_val.as_object().map_or(0, |o| o.len()));

        // DELETE /captures — clear all
        http_delete("/captures").unwrap_or_default();
        let cap_after = http_get("/captures").unwrap_or_default();
        let cap_after_val: serde_json::Value = serde_json::from_str(&cap_after).unwrap_or_default();
        assert!(
            cap_after_val.as_object().map_or(false, |o| o.is_empty()),
            "/captures after DELETE should be empty; got: {cap_after}"
        );
        eprintln!("[PASS] /captures DELETE cleared all");

        // ── /stories — grouped scenario index ────────────────────────────────
        let stories_body = http_get("/stories").unwrap_or_default();
        let stories_val: serde_json::Value = serde_json::from_str(&stories_body).unwrap_or_default();
        assert!(
            stories_val.get("total_stories").is_some(),
            "/stories must return total_stories field; got: {stories_body}"
        );
        let story_count = stories_val["total_stories"].as_u64().unwrap_or(0);
        assert!(
            story_count > 0,
            "/stories total_stories should be > 0; got {story_count}"
        );
        eprintln!("[PASS] /stories returned {story_count} story group(s)");

        // ── /coverage — scenario coverage statistics ──────────────────────────
        let cov_body = http_get("/coverage").unwrap_or_default();
        let cov_val: serde_json::Value = serde_json::from_str(&cov_body).unwrap_or_default();
        assert!(
            cov_val.get("total_scenarios").is_some() && cov_val.get("with_story").is_some(),
            "/coverage must return total_scenarios and with_story; got: {cov_body}"
        );
        let cov_total = cov_val["total_scenarios"].as_u64().unwrap_or(0);
        let cov_story = cov_val["with_story"].as_u64().unwrap_or(0);
        assert!(
            cov_total >= 450,
            "/coverage total_scenarios should be >= 450; got {cov_total}"
        );
        assert!(
            cov_story > 0,
            "/coverage with_story should be > 0; got {cov_story}"
        );
        eprintln!("[PASS] /coverage: {cov_story}/{cov_total} scenarios have stories ({:.0}%)",
            cov_val["coverage_pct"].as_u64().unwrap_or(0));

        assert!(all_pass, "one or more dev inspector scenarios failed");
    }
}

#[cfg(not(debug_assertions))]
#[test]
fn inspector_not_built_in_release() {}
