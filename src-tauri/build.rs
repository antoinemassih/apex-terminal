fn main() {
    tauri_build::build();
    // Icon for standalone apex-native is set at runtime via WM_SETICON + set_window_icon.
    // Build-time resource embedding conflicts with Tauri's resource in the shared crate.
}
