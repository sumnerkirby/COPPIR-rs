//! The native window: a system webview (WebKit on macOS, WebView2 on Windows,
//! WebKitGTK on Linux) pointed at the local server.
//!
//! The server is already listening by the time this runs, so the window goes
//! straight to the app; `index.html` carries its own loading screen.

use std::path::{Path, PathBuf};

use tao::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use wry::{NewWindowResponse, WebViewBuilder};

const TITLE: &str = "COPPIR — Common Operational Picture Program for Incident Response";

pub fn run(url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title(TITLE)
        .with_inner_size(LogicalSize::new(1440.0, 900.0))
        .with_min_inner_size(LogicalSize::new(1100.0, 700.0))
        .build(&event_loop)?;

    let origin = url.to_owned();
    let builder = WebViewBuilder::new()
        .with_url(url)
        .with_devtools(cfg!(debug_assertions))
        // Exports are blob downloads (`<a download>`). A webview has no
        // download shelf, so put them in the user's Downloads folder.
        .with_download_started_handler(|_src, dest| {
            let name = dest.file_name().map(PathBuf::from).unwrap_or_else(|| "coppir-export".into());
            match downloads_dir() {
                Some(dir) => {
                    *dest = unique_path(&dir, &name);
                    true
                }
                None => false,
            }
        })
        .with_download_completed_handler(|_src, path, ok| match (ok, path) {
            (true, Some(p)) => tracing::info!("saved {}", p.display()),
            _ => tracing::warn!("download failed"),
        })
        // Links off the app (map attribution, OSM) belong in the real browser.
        // Blank popups — the printable SITREP — open in a webview window.
        .with_new_window_req_handler(move |target, _features| {
            if (target.starts_with("http://") || target.starts_with("https://")) && !target.starts_with(&origin) {
                open_in_browser(&target);
                NewWindowResponse::Deny
            } else {
                NewWindowResponse::Allow
            }
        });

    #[cfg(any(target_os = "windows", target_os = "macos"))]
    let webview = builder.build(&window)?;
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let webview = {
        use tao::platform::unix::WindowExtUnix;
        use wry::WebViewBuilderExtUnix;
        builder.build_gtk(window.default_vbox().ok_or("no GTK container on the window")?)?
    };

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        // Keep the webview alive for as long as the loop runs.
        let _ = &webview;
        if let Event::WindowEvent { event: WindowEvent::CloseRequested, .. } = event {
            *control_flow = ControlFlow::Exit;
        }
    });
}

fn downloads_dir() -> Option<PathBuf> {
    directories::UserDirs::new()
        .and_then(|d| d.download_dir().map(Path::to_path_buf))
        .or_else(|| directories::UserDirs::new().map(|d| d.home_dir().to_path_buf()))
}

/// `name.ext`, or `name (2).ext`, … — whichever does not exist yet.
fn unique_path(dir: &Path, name: &Path) -> PathBuf {
    let first = dir.join(name);
    if !first.exists() {
        return first;
    }
    let stem = name.file_stem().and_then(|s| s.to_str()).unwrap_or("download");
    let ext = name.extension().and_then(|s| s.to_str()).map(|e| format!(".{e}")).unwrap_or_default();
    (2..).map(|n| dir.join(format!("{stem} ({n}){ext}"))).find(|p| !p.exists()).expect("some suffix is free")
}

fn open_in_browser(url: &str) {
    #[cfg(target_os = "macos")]
    let cmd = std::process::Command::new("open").arg(url).spawn();
    #[cfg(target_os = "windows")]
    let cmd = std::process::Command::new("rundll32").args(["url.dll,FileProtocolHandler", url]).spawn();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let cmd = std::process::Command::new("xdg-open").arg(url).spawn();
    if let Err(e) = cmd {
        tracing::warn!("could not open {url}: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::unique_path;
    use std::path::Path;

    #[test]
    fn unique_paths_do_not_clobber() {
        let dir = tempfile::tempdir().unwrap();
        let name = Path::new("log.csv");
        let a = unique_path(dir.path(), name);
        assert_eq!(a, dir.path().join("log.csv"));
        std::fs::write(&a, "").unwrap();
        assert_eq!(unique_path(dir.path(), name), dir.path().join("log (2).csv"));
    }
}
