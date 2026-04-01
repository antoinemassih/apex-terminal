fn main() {
    tauri_build::build();

    // Embed icon for the standalone apex-native binary
    if std::env::var("CARGO_BIN_NAME").as_deref() == Ok("apex-native") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("icons/apex-native.ico");
        res.set("ProductName", "Apex Terminal");
        res.set("FileDescription", "Apex Terminal — Native GPU Trading Chart");
        if let Err(e) = res.compile() {
            eprintln!("Warning: failed to embed icon: {e}");
        }
    }
}
