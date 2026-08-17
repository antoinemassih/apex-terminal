//! W1-04 (audit): out-of-app alert delivery.
//!
//! Before this, a fired alert did exactly two things: push an in-app toast and
//! `eprintln!("... sound notification placeholder")`. There was **zero**
//! out-of-app delivery — the day-trader persona called this the single biggest
//! reason the terminal can't be a daily driver: you step away from the screen
//! and the alert you set reaches you in no way at all.
//!
//! This module adds the two channels that need **no new dependencies**:
//!
//! 1. **Audible** — `MessageBeep` (Win32, already available via the
//!    `windows-sys` `Win32_UI_WindowsAndMessaging` feature this crate enables).
//!    Works with zero configuration, which is what makes it the real fix for
//!    the placeholder. No-op on non-Windows (cfg-gated) until a portable audio
//!    backend is chosen.
//! 2. **Webhook** — a JSON POST to a user-configured URL via `reqwest`
//!    (already a dependency). This is what actually reaches a phone: point it
//!    at ntfy / Pushover / a Discord webhook / IFTTT and the alert arrives
//!    wherever the trader is. Opt-in via `APEX_ALERT_WEBHOOK`, matching the
//!    existing env-var config style (`apex_data::config`).
//!
//! Deliberately NOT here (needs a new crate, so it is a separate decision):
//! native OS toast notifications. Tracked as W1-04b.

/// Env var holding the alert webhook URL. Empty/unset disables webhook delivery.
const WEBHOOK_ENV: &str = "APEX_ALERT_WEBHOOK";

/// Env var to silence the audible alert (`1`/`true` = muted). Sound is ON by
/// default — an alert that makes no noise is the bug we are fixing.
const MUTE_ENV: &str = "APEX_ALERT_MUTE";

/// Configured webhook URL, if any. Trimmed; empty is treated as unset.
pub fn webhook_url() -> Option<String> {
    std::env::var(WEBHOOK_ENV)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Whether the audible alert is muted by config.
pub fn is_muted() -> bool {
    std::env::var(MUTE_ENV)
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Build the webhook JSON payload for an alert.
///
/// Shape is deliberately flat and generic so it works with the common relays
/// without transformation: ntfy reads `title`/`message`, Discord reads
/// `content`, and everything else can pick fields out. Pure → unit-testable.
pub fn webhook_payload(symbol: &str, message: &str, price: Option<f32>) -> serde_json::Value {
    // `content` is what a Discord webhook renders; `title`/`message` is what
    // ntfy/Pushover use. Emitting all three keeps one payload universal.
    let content = format!("[APEX] {message}");
    let mut v = serde_json::json!({
        "source": "apex-terminal",
        "kind": "alert",
        "symbol": symbol,
        "title": format!("APEX alert — {symbol}"),
        "message": message,
        "content": content,
    });
    if let Some(p) = price {
        v["price"] = serde_json::json!(p);
    }
    v
}

/// Play the system alert sound (Windows), **off the calling thread**.
/// Replaces the old `eprintln!("sound notification placeholder")`.
///
/// MUST NOT run inline: `deliver()` is called from the alert-eval path inside
/// the render loop, and `MessageBeep` is only documented as asynchronous once
/// the audio device is already open — the first call (or a call while the
/// audio subsystem is initialising / no device is present) can block for tens
/// of milliseconds. Blocking the render thread mid-frame is unacceptable in a
/// charting terminal: it stalls the frame with pane geometry half-updated. So
/// the beep is dispatched to a short-lived guarded thread and the render
/// thread never waits on the audio stack.
fn play_alert_sound() {
    if is_muted() {
        return;
    }
    crate::foundation::guard::spawn_guarded("alert_sound", || {
        #[cfg(target_os = "windows")]
        {
            // MB_ICONEXCLAMATION — the system "warning" sound.
            // NOTE: windows-sys puts MessageBeep under System::Diagnostics::Debug
            // (not UI::WindowsAndMessaging, where the MB_* constants live), hence
            // the split paths and the added Win32_System_Diagnostics_Debug feature.
            unsafe {
                let _ = windows_sys::Win32::System::Diagnostics::Debug::MessageBeep(
                    windows_sys::Win32::UI::WindowsAndMessaging::MB_ICONEXCLAMATION,
                );
            }
        }
    });
}

/// W1-04b: env to suppress the native OS toast (`1`/`true`). Toasts are ON by
/// default — a desktop trader who stepped away expects the alert in the corner
/// of their screen, which is the one thing the audible + webhook channels don't
/// give a glance-able signal for.
const NO_TOAST_ENV: &str = "APEX_ALERT_NO_TOAST";

fn toast_suppressed() -> bool {
    std::env::var(NO_TOAST_ENV)
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// W1-04b: show a native Windows toast for a fired alert.
///
/// Spawned off the caller (render/eval) thread — WinRT toast construction and
/// the XML pipeline are not something to run mid-frame. Best-effort: a failure
/// is reported through `errors_sink`, never panicked or swallowed. The toast is
/// SILENT (`Sound::None`) on purpose — `play_alert_sound()` already owns the
/// audio, and a toast with its own sound would double-beep.
///
/// No-op on non-Windows until a portable toast backend is chosen (tracked with
/// the same cfg as the `MessageBeep` path above).
fn show_toast(symbol: &str, message: &str) {
    if toast_suppressed() {
        return;
    }
    #[cfg(target_os = "windows")]
    {
        let title = format!("APEX alert — {symbol}");
        let body = message.to_string();
        crate::foundation::guard::spawn_guarded("alert_toast", move || {
            use tauri_winrt_notification::{Duration, Toast};
            // POWERSHELL_APP_ID is the standard AppUserModelID fallback for an
            // app that hasn't registered its own AUMID (that comes with the
            // installer, W4-07). Without a valid AUMID Windows silently drops
            // the toast, so this attribution is deliberate, not a placeholder.
            let result = Toast::new(Toast::POWERSHELL_APP_ID)
                .title(&title)
                .text1(&body)
                .sound(None) // audio is play_alert_sound()'s job — see doc above
                .duration(Duration::Short)
                .show();
            if let Err(e) = result {
                crate::data::connectivity::errors_sink::report(
                    crate::data::connectivity::errors_sink::ErrorLevel::Warn,
                    "alert_delivery",
                    "toast_failed",
                    format!("native toast failed: {e}"),
                );
            }
        });
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (symbol, message);
    }
}

/// Deliver a fired alert out-of-app: audible + native toast + webhook (all
/// best-effort).
///
/// Never blocks the caller — the toast and webhook run on spawned threads,
/// because this is called from the render/eval path. Failures are reported
/// through `errors_sink` rather than swallowed, so a misconfigured webhook or a
/// dropped toast is visible instead of silently losing the trader's alerts.
pub fn deliver(symbol: &str, message: &str, price: Option<f32>) {
    play_alert_sound();
    // Toast BEFORE the webhook early-return — it must fire whether or not a
    // webhook URL is configured.
    show_toast(symbol, message);

    let Some(url) = webhook_url() else { return };
    let payload = webhook_payload(symbol, message, price);
    crate::foundation::guard::spawn_guarded("alert_webhook", move || {
        let client = crate::foundation::http::blocking_client();
        match client
            .post(&url)
            .json(&payload)
            .timeout(std::time::Duration::from_secs(5))
            .send()
        {
            Ok(resp) if resp.status().is_success() => {}
            Ok(resp) => {
                crate::data::connectivity::errors_sink::report(
                    crate::data::connectivity::errors_sink::ErrorLevel::Warn,
                    "alert_delivery",
                    "webhook_rejected",
                    format!("alert webhook returned HTTP {}", resp.status().as_u16()),
                );
            }
            Err(e) => {
                crate::data::connectivity::errors_sink::report(
                    crate::data::connectivity::errors_sink::ErrorLevel::Warn,
                    "alert_delivery",
                    "webhook_failed",
                    format!("alert webhook POST failed: {e}"),
                );
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_carries_the_fields_relays_need() {
        let v = webhook_payload("SPY", "price above 450.00", Some(451.25));
        assert_eq!(v["symbol"], "SPY");
        assert_eq!(v["kind"], "alert");
        assert_eq!(v["message"], "price above 450.00");
        // Discord renders `content`; ntfy/Pushover use title/message.
        assert!(v["content"].as_str().unwrap().contains("price above 450.00"));
        assert!(v["title"].as_str().unwrap().contains("SPY"));
        assert_eq!(v["price"].as_f64(), Some(451.25));
    }

    #[test]
    fn payload_omits_price_when_absent() {
        // Drawing-crossing alerts have no single "alert price" — the field must
        // be absent rather than a fabricated 0.0.
        let v = webhook_payload("QQQ", "Price crossed trendline at 380.10", None);
        assert!(v.get("price").is_none(), "no price → field omitted, not zeroed");
        assert_eq!(v["symbol"], "QQQ");
    }
}
