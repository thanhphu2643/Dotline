//! Dotline for Windows: the same localhost API as the macOS app, driven from
//! a tray icon. Printing goes through the Windows spooler (RAW datatype).
#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
mod tray;

#[cfg(windows)]
fn main() {
    tray::run();
}

#[cfg(not(windows))]
fn main() {
    eprintln!("Dotline for Windows only builds for Windows targets; on macOS use the app in ../app.");
}
