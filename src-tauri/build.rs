fn main() {
    tauri_build::build();
    // Note: winresource icon embedding is skipped here to avoid duplicate resource
    // conflict with Tauri's own resource embedding. The standalone apex-native binary
    // gets its icon from the programmatic make_window_icon() in gpu.rs instead.
}
