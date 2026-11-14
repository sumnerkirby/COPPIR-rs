//! On Windows, compile the icon and version details into the executable, so
//! Explorer, the taskbar and the Start menu show them. Nothing elsewhere.

fn main() {
    println!("cargo:rerun-if-changed=packaging/icons/icon.ico");
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("packaging/icons/icon.ico");
        res.set("ProductName", "COPPIR");
        res.set("FileDescription", "COPPIR: common operational picture for incident response exercises");
        if let Err(e) = res.compile() {
            println!("cargo:warning=building without Windows resources: {e}");
        }
    }
}
