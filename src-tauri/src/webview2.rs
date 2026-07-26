use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

const WEBVIEW2_DOWNLOAD_URL: &str = "https://developer.microsoft.com/microsoft-edge/webview2/";

pub fn ensure_available() -> bool {
    if webview2_runtime_exists() {
        return true;
    }

    show_missing_runtime_message();
    false
}

fn webview2_runtime_exists() -> bool {
    if env::var_os("WEBVIEW2_BROWSER_EXECUTABLE_FOLDER")
        .as_deref()
        .is_some_and(|directory| Path::new(directory).join("msedgewebview2.exe").is_file())
    {
        return true;
    }

    [
        env::var_os("LOCALAPPDATA"),
        env::var_os("ProgramFiles(x86)"),
        env::var_os("ProgramFiles"),
    ]
    .into_iter()
    .flatten()
    .map(|directory| Path::new(&directory).join("Microsoft/EdgeWebView/Application"))
    .any(|directory| runtime_in(&directory))
}

fn runtime_in(application_directory: &Path) -> bool {
    fs::read_dir(application_directory)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .any(|entry| entry.path().join("msedgewebview2.exe").is_file())
}

fn show_missing_runtime_message() {
    let script = format!(
        "Add-Type -AssemblyName PresentationFramework; [void][System.Windows.MessageBox]::Show('GeoTagger 需要 Microsoft Edge WebView2 Runtime 才能启动。将打开官方下载页面；安装完成后请再次运行此 EXE。', '缺少 WebView2 Runtime', 'OK', 'Error'); Start-Process '{WEBVIEW2_DOWNLOAD_URL}'"
    );
    let _ = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .status();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_versioned_runtime_directory() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let runtime = directory.path().join("125.0.2535.67");
        fs::create_dir_all(&runtime).expect("runtime directory");
        fs::write(runtime.join("msedgewebview2.exe"), b"fixture").expect("runtime executable");
        assert!(runtime_in(directory.path()));
    }
}
