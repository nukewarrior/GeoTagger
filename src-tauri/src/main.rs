#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(windows)]
    if std::env::var_os("GEOTAGGER_PORTABLE_SMOKE").is_some() {
        std::process::exit(geotagger_lib::portable_smoke());
    }

    #[cfg(windows)]
    if !geotagger_lib::webview2_available() {
        std::process::exit(1);
    }

    geotagger_lib::run();
}
