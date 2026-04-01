fn main() {
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("icons/apex-native.ico");
        res.set("ProductName", "Apex Terminal");
        res.set("FileDescription", "Apex Terminal — Native GPU Trading Chart");
        res.compile().expect("Failed to compile Windows resources");
    }
}
