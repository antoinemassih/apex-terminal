fn main() {
    // Embed the Windows app icon + manifest into the .exe. Tauri-build used to
    // do this automatically; after removing Tauri we do it explicitly so the
    // file / pinned-taskbar icon shows the Xolio mark (the runtime WM_SETICON
    // path already sets the live window/taskbar icon). Non-fatal: if the RC
    // toolchain is missing, the icon just isn't embedded.
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("icons/apex-native.ico");
        res.set_manifest_file("apex-native.manifest");
        if let Err(e) = res.compile() {
            println!("cargo:warning=app icon/manifest embed skipped: {e}");
        }
        println!("cargo:rerun-if-changed=icons/apex-native.ico");
        println!("cargo:rerun-if-changed=apex-native.manifest");
    }

    // Embed the short git SHA so perf_log can stamp every jank event and
    // session summary with the exact build that produced the data.
    let sha = std::process::Command::new("git")
        .args(["rev-parse", "--short=12", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=APEX_GIT_SHA={}", sha);
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/heads");
}
