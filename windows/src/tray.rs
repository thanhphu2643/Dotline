use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tao::event::{Event, StartCause};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    IDYES, MB_ICONINFORMATION, MB_ICONWARNING, MB_OK, MB_YESNO, MessageBoxW,
};

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

#[derive(Clone)]
enum Action {
    CopyToken,
    CopyAddress,
    TestPrint(String, &'static str),
    OpenData,
    ToggleAutostart,
    RotateToken,
    Quit,
}

/// English UI, Vietnamese when Windows is set to Vietnamese.
fn t(en: &'static str, vi: &'static str) -> &'static str {
    static VI: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    let vi_ui = *VI.get_or_init(|| {
        let mut buf = [0u16; 85];
        let n = unsafe { windows_sys::Win32::Globalization::GetUserDefaultLocaleName(buf.as_mut_ptr(), buf.len() as i32) };
        n > 0 && String::from_utf16_lossy(&buf[..n as usize - 1]).starts_with("vi")
    });
    if vi_ui { vi } else { en }
}

fn message(text: &str, warn: bool, yes_no: bool) -> bool {
    let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
    let flags = if yes_no { MB_YESNO | MB_ICONWARNING } else if warn { MB_OK | MB_ICONWARNING } else { MB_OK | MB_ICONINFORMATION };
    unsafe { MessageBoxW(std::ptr::null_mut(), wide(text).as_ptr(), wide("Dotline").as_ptr(), flags) == IDYES }
}

fn data_dir() -> PathBuf {
    PathBuf::from(std::env::var("APPDATA").unwrap_or_else(|_| ".".into())).join("Dotline")
}

fn icon() -> Icon {
    let decoder = png::Decoder::new(std::io::Cursor::new(include_bytes!("../assets/icon-32.png")));
    let mut reader = decoder.read_info().expect("icon");
    let mut buf = vec![0; reader.output_buffer_size().expect("icon size")];
    let info = reader.next_frame(&mut buf).expect("icon frame");
    buf.truncate(info.buffer_size());
    Icon::from_rgba(buf, info.width, info.height).expect("icon rgba")
}

fn autostart() -> bool {
    winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
        .open_subkey(RUN_KEY)
        .and_then(|k| k.get_value::<String, _>("Dotline"))
        .is_ok()
}

fn set_autostart(on: bool) {
    let Ok((key, _)) = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER).create_subkey(RUN_KEY) else { return };
    if on {
        if let Ok(exe) = std::env::current_exe() {
            let _ = key.set_value("Dotline", &format!("\"{}\"", exe.display()));
        }
    } else {
        let _ = key.delete_value("Dotline");
    }
}

fn copy(text: &str) {
    if let Ok(mut clipboard) = arboard::Clipboard::new() {
        let _ = clipboard.set_text(text.to_string());
    }
}

/// The parts of a snapshot the menu shows; the menu is rebuilt only when this changes.
fn menu_key(snap: &Value) -> String {
    let jobs: Vec<&Value> = snap["jobs"].as_array().map(|j| j.iter().take(8).collect()).unwrap_or_default();
    serde_json::json!([snap["running"], snap["port"], snap["printers"], jobs.iter().map(|j| [&j["id"], &j["state"]]).collect::<Vec<_>>()]).to_string()
}

fn item(actions: &mut HashMap<MenuId, Action>, text: &str, action: Action) -> MenuItem {
    let i = MenuItem::new(text, true, None);
    actions.insert(i.id().clone(), action);
    i
}

fn build_menu(snap: &Value, actions: &mut HashMap<MenuId, Action>) -> Menu {
    actions.clear();
    let menu = Menu::new();
    let running = snap["running"].as_bool().unwrap_or(false);
    let port = snap["port"].as_u64().unwrap_or(0);
    let status = if running {
        format!("Dotline {} · {} 127.0.0.1:{port}", env!("CARGO_PKG_VERSION"), t("running on", "đang chạy ở"))
    } else {
        format!("Dotline · {}", t("stopped", "đã dừng"))
    };
    let _ = menu.append(&MenuItem::new(status, false, None));
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&item(actions, t("Copy access token", "Sao chép mã truy cập"), Action::CopyToken));
    let _ = menu.append(&item(actions, t("Copy address", "Sao chép địa chỉ"), Action::CopyAddress));

    let printers = Submenu::new(t("Printers", "Máy in"), true);
    let list = snap["printers"].as_array().cloned().unwrap_or_default();
    if list.is_empty() {
        let _ = printers.append(&MenuItem::new(t("No printers found", "Không thấy máy in"), false, None));
    }
    for p in &list {
        let name = p["name"].as_str().unwrap_or_default().to_string();
        let mark = if p["isDefault"].as_bool() == Some(true) { " ★" } else { "" };
        let sub = Submenu::new(format!("{name}{mark}"), true);
        for (label, lang) in [("Test print · ZPL (Zebra)", "zpl"), ("Test print · EZPL (GoDEX)", "ezpl"), ("Test print · TSPL (TSC)", "tspl")] {
            let _ = sub.append(&item(actions, label, Action::TestPrint(name.clone(), lang)));
        }
        let _ = printers.append(&sub);
    }
    let _ = menu.append(&printers);

    let recent = Submenu::new(t("Recent jobs", "Lệnh in gần đây"), true);
    let jobs = snap["jobs"].as_array().cloned().unwrap_or_default();
    if jobs.is_empty() {
        let _ = recent.append(&MenuItem::new(t("No jobs yet", "Chưa có lệnh in"), false, None));
    }
    for j in jobs.iter().take(8) {
        let ok = j["state"] == "sent";
        let text = format!("{} {} → {}", if ok { "✓" } else { "✗" }, j["id"].as_str().unwrap_or(""), j["printer"].as_str().unwrap_or(""));
        let _ = recent.append(&MenuItem::new(text, false, None));
    }
    let _ = menu.append(&recent);

    let _ = menu.append(&PredefinedMenuItem::separator());
    let auto = CheckMenuItem::new(t("Start with Windows", "Khởi động cùng Windows"), true, autostart(), None);
    actions.insert(auto.id().clone(), Action::ToggleAutostart);
    let _ = menu.append(&auto);
    let _ = menu.append(&item(actions, t("Open data folder", "Mở thư mục dữ liệu"), Action::OpenData));
    let _ = menu.append(&item(actions, t("New access token…", "Tạo mã truy cập mới…"), Action::RotateToken));
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&item(actions, t("Quit Dotline", "Thoát Dotline"), Action::Quit));
    menu
}

pub fn run() {
    if let Err(e) = dotline_core::start(&data_dir(), 0) {
        message(&format!("{}\n\n{e}", t("Dotline could not start. It may already be running.", "Dotline không khởi động được. Có thể app đang chạy rồi.")), true, false);
        return;
    }
    let event_loop = EventLoopBuilder::<MenuEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    MenuEvent::set_event_handler(Some(move |e| {
        let _ = proxy.send_event(e);
    }));

    let mut tray: Option<TrayIcon> = None;
    let mut actions = HashMap::new();
    let mut shown = String::new();

    event_loop.run(move |event, _, flow| {
        *flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_secs(4));
        let refresh = |tray: &mut Option<TrayIcon>, actions: &mut HashMap<MenuId, Action>, shown: &mut String| {
            let snap = dotline_core::snapshot();
            let key = menu_key(&snap);
            if key == *shown {
                return;
            }
            *shown = key;
            let menu = build_menu(&snap, actions);
            match tray {
                Some(t) => t.set_menu(Some(Box::new(menu))),
                None => {
                    *tray = TrayIconBuilder::new()
                        .with_menu(Box::new(menu))
                        .with_tooltip("Dotline")
                        .with_icon(icon())
                        .build()
                        .ok()
                }
            }
        };
        match event {
            Event::NewEvents(StartCause::Init) | Event::NewEvents(StartCause::ResumeTimeReached { .. }) => {
                refresh(&mut tray, &mut actions, &mut shown)
            }
            Event::UserEvent(e) => {
                let Some(action) = actions.get(e.id()).cloned() else { return };
                let snap = dotline_core::snapshot();
                match action {
                    Action::CopyToken => copy(snap["token"].as_str().unwrap_or_default()),
                    Action::CopyAddress => copy(&format!("http://127.0.0.1:{}", snap["port"].as_u64().unwrap_or(0))),
                    Action::TestPrint(printer, lang) => {
                        // Off the UI thread: a network queue can take a few seconds.
                        std::thread::spawn(move || {
                            let r = dotline_core::test_print(&printer, lang);
                            if r["state"] != "sent" {
                                message(&format!("{}\n\n{}", t("Test print failed.", "In thử lỗi."), r["detail"].as_str().unwrap_or("")), true, false);
                            }
                        });
                    }
                    Action::OpenData => {
                        let _ = std::process::Command::new("explorer").arg(data_dir()).spawn();
                    }
                    Action::ToggleAutostart => set_autostart(!autostart()),
                    Action::RotateToken => {
                        if message(t("Issue a new access token? Every app using the current token is disconnected until you update it.",
                                     "Tạo mã truy cập mới? Mọi ứng dụng đang dùng mã cũ sẽ bị ngắt cho đến khi cập nhật mã mới."), true, true) {
                            let r = dotline_core::rotate_token();
                            if let Some(token) = r["token"].as_str() {
                                copy(token);
                                message(t("New access token copied to the clipboard.", "Đã sao chép mã truy cập mới."), false, false);
                            }
                        }
                    }
                    Action::Quit => {
                        dotline_core::stop();
                        tray.take();
                        *flow = ControlFlow::Exit;
                        return;
                    }
                }
                shown.clear();
                refresh(&mut tray, &mut actions, &mut shown);
            }
            _ => {}
        }
    });
}
