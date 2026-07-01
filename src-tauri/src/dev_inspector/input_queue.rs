//! Input event queue — dev-injected events indistinguishable from real user input.

use egui::{Key, Modifiers, PointerButton};

/// An input event queued via `POST /input`.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DevInput {
    Click        { x: f32, y: f32 },
    DoubleClick  { x: f32, y: f32 },
    RightClick   { x: f32, y: f32 },
    Move         { x: f32, y: f32 },
    /// Pointer press / release primitives — split across frames for a reliable
    /// egui widget click (same-frame press+release can miss `clicked()`).
    Down         { x: f32, y: f32 },
    Up           { x: f32, y: f32 },
    Drag         { from_x: f32, from_y: f32, to_x: f32, to_y: f32 },
    Scroll       { x: f32, y: f32, delta_y: f32 },
    Key          { key: String },
    Type         { text: String },
}

/// Drain all queued DevInputs from the inspector into an egui event list.
/// Called from gpu.rs just before `ctx.run(raw_input, ...)` so events
/// are visible to all widgets in the frame.
pub fn drain_inputs_raw(events: &mut Vec<egui::Event>) {
    let inputs = crate::dev_inspector::drain_inputs();
    for input in inputs {
        append_events(events, input);
    }
}

fn append_events(events: &mut Vec<egui::Event>, input: DevInput) {
    use egui::Event;

    match input {
        DevInput::Move { x, y } => {
            events.push(Event::PointerMoved(egui::pos2(x, y)));
        }

        DevInput::Down { x, y } => {
            let pos = egui::pos2(x, y);
            events.push(Event::PointerMoved(pos));
            events.push(Event::PointerButton {
                pos, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE,
            });
        }

        DevInput::Up { x, y } => {
            let pos = egui::pos2(x, y);
            events.push(Event::PointerButton {
                pos, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE,
            });
        }

        DevInput::Click { x, y } => {
            let pos = egui::pos2(x, y);
            events.push(Event::PointerMoved(pos));
            events.push(Event::PointerButton {
                pos, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE,
            });
            events.push(Event::PointerButton {
                pos, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE,
            });
        }

        DevInput::DoubleClick { x, y } => {
            let pos = egui::pos2(x, y);
            events.push(Event::PointerMoved(pos));
            for _ in 0..2 {
                events.push(Event::PointerButton {
                    pos, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE,
                });
                events.push(Event::PointerButton {
                    pos, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE,
                });
            }
        }

        DevInput::RightClick { x, y } => {
            let pos = egui::pos2(x, y);
            events.push(Event::PointerMoved(pos));
            events.push(Event::PointerButton {
                pos, button: PointerButton::Secondary, pressed: true, modifiers: Modifiers::NONE,
            });
            events.push(Event::PointerButton {
                pos, button: PointerButton::Secondary, pressed: false, modifiers: Modifiers::NONE,
            });
        }

        DevInput::Drag { from_x, from_y, to_x, to_y } => {
            events.push(Event::PointerMoved(egui::pos2(from_x, from_y)));
            events.push(Event::PointerButton {
                pos: egui::pos2(from_x, from_y),
                button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE,
            });
            let mid = egui::pos2((from_x + to_x) / 2.0, (from_y + to_y) / 2.0);
            events.push(Event::PointerMoved(mid));
            events.push(Event::PointerMoved(egui::pos2(to_x, to_y)));
            events.push(Event::PointerButton {
                pos: egui::pos2(to_x, to_y),
                button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE,
            });
        }

        DevInput::Scroll { x, y, delta_y } => {
            events.push(Event::PointerMoved(egui::pos2(x, y)));
            events.push(Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, delta_y),
                modifiers: Modifiers::NONE,
            });
        }

        DevInput::Key { key: key_str } => {
            let (key, mods) = parse_key_string(&key_str);
            if let Some(key) = key {
                events.push(Event::Key {
                    key,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: mods,
                });
                events.push(Event::Key {
                    key,
                    physical_key: None,
                    pressed: false,
                    repeat: false,
                    modifiers: mods,
                });
            }
        }

        DevInput::Type { text } => {
            events.push(Event::Text(text));
        }
    }
}

/// Parse `"ctrl+shift+p"` → `(Some(Key::P), Modifiers { ctrl, shift, .. })`.
/// Case-insensitive. Modifiers: ctrl, shift, alt, mac_cmd.
fn parse_key_string(s: &str) -> (Option<Key>, Modifiers) {
    let parts: Vec<&str> = s.split('+').collect();
    let mut mods = Modifiers::NONE;
    let mut key_str = "";

    for (i, part) in parts.iter().enumerate() {
        let lower = part.to_lowercase();
        match lower.as_str() {
            "ctrl" | "control" => mods.ctrl = true,
            "shift"            => mods.shift = true,
            "alt"              => mods.alt = true,
            "cmd" | "meta"     => mods.mac_cmd = true,
            _ if i == parts.len() - 1 => key_str = part,
            _ => {}
        }
    }

    let key = match key_str.to_lowercase().as_str() {
        "a" => Some(Key::A), "b" => Some(Key::B), "c" => Some(Key::C),
        "d" => Some(Key::D), "e" => Some(Key::E), "f" => Some(Key::F),
        "g" => Some(Key::G), "h" => Some(Key::H), "i" => Some(Key::I),
        "j" => Some(Key::J), "k" => Some(Key::K), "l" => Some(Key::L),
        "m" => Some(Key::M), "n" => Some(Key::N), "o" => Some(Key::O),
        "p" => Some(Key::P), "q" => Some(Key::Q), "r" => Some(Key::R),
        "s" => Some(Key::S), "t" => Some(Key::T), "u" => Some(Key::U),
        "v" => Some(Key::V), "w" => Some(Key::W), "x" => Some(Key::X),
        "y" => Some(Key::Y), "z" => Some(Key::Z),
        "0" => Some(Key::Num0), "1" => Some(Key::Num1), "2" => Some(Key::Num2),
        "3" => Some(Key::Num3), "4" => Some(Key::Num4), "5" => Some(Key::Num5),
        "6" => Some(Key::Num6), "7" => Some(Key::Num7), "8" => Some(Key::Num8),
        "9" => Some(Key::Num9),
        "enter" | "return"  => Some(Key::Enter),
        "escape" | "esc"    => Some(Key::Escape),
        "tab"               => Some(Key::Tab),
        "backspace"         => Some(Key::Backspace),
        "delete" | "del"    => Some(Key::Delete),
        "arrowup" | "up"    => Some(Key::ArrowUp),
        "arrowdown" | "down"=> Some(Key::ArrowDown),
        "arrowleft" | "left"=> Some(Key::ArrowLeft),
        "arrowright"| "right"=> Some(Key::ArrowRight),
        "home"              => Some(Key::Home),
        "end"               => Some(Key::End),
        "pageup"            => Some(Key::PageUp),
        "pagedown"          => Some(Key::PageDown),
        "space" | " "       => Some(Key::Space),
        "f1"  => Some(Key::F1),  "f2"  => Some(Key::F2),  "f3"  => Some(Key::F3),
        "f4"  => Some(Key::F4),  "f5"  => Some(Key::F5),  "f6"  => Some(Key::F6),
        "f7"  => Some(Key::F7),  "f8"  => Some(Key::F8),  "f9"  => Some(Key::F9),
        "f10" => Some(Key::F10), "f11" => Some(Key::F11), "f12" => Some(Key::F12),
        _ => {
            eprintln!("[dev-inspector] unknown key: {key_str:?}");
            None
        }
    };

    (key, mods)
}
