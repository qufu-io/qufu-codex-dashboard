pub mod data;
pub mod limits;
pub mod pricing;
pub mod quota;
mod updates;
use serde_json::{json, Value};
use std::sync::Mutex;
use tauri::{Manager, State};

#[derive(Default)]
struct Appearance(Mutex<Option<f64>>);

fn restore_opacity(window: &tauri::WebviewWindow) {
    let alpha = window.state::<Appearance>().0.lock().ok().and_then(|v| *v);
    if let Some(alpha) = alpha {
        let _ = opacity(window.clone(), alpha);
    }
}

fn show_main(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        restore_opacity(&window);
    }
}

fn tray_menu(
    app: &tauri::AppHandle,
    language: &str,
) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    use tauri::menu::{Menu, MenuItem};
    let english = language == "en";
    let show = MenuItem::with_id(
        app,
        "show",
        if english {
            "Open Qufu Dashboard"
        } else {
            "Qufu Dashboard’u aç"
        },
        true,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(
        app,
        "quit",
        if english { "Quit" } else { "Çıkış" },
        true,
        None::<&str>,
    )?;
    Menu::with_items(app, &[&show, &quit])
}
#[tauri::command]
fn menu_language(app: tauri::AppHandle, language: String) -> Result<(), String> {
    if let Some(tray) = app.tray_by_id("qufu") {
        tray.set_menu(Some(tray_menu(&app, &language).map_err(|e| e.to_string())?))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn snapshot(
    reader: State<'_, Mutex<data::Reader>>,
    feed: State<'_, quota::Feed>,
    estimator: State<'_, Mutex<limits::Estimator>>,
    selected: Option<String>,
) -> Result<Value, String> {
    let mut value = reader
        .lock()
        .map_err(|_| "Veri okuyucu kilidi açılamadı".to_string())?
        .snapshot(selected.as_deref())?;
    if let Some(q) = feed.snapshot() {
        if q["at"].as_i64() > value["quota"]["at"].as_i64() {
            value["quota"] = q;
        }
    }
    let estimates = estimator
        .lock()
        .map_err(|e| e.to_string())?
        .update(&value["quota"]);
    attach_estimates(&mut value, &estimates);
    value["pricing"] = pricing::info();
    Ok(value)
}
fn attach_estimates(value: &mut Value, estimates: &Value) {
    for key in ["primary", "secondary"] {
        if value["quota"]["value"][key].is_object() {
            value["quota"]["value"][key]["token_estimate"] = estimates[key].clone();
        }
    }
}
fn settings_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    app.path()
        .app_config_dir()
        .map(|p| p.join("settings.json"))
        .map_err(|e| e.to_string())
}
#[tauri::command]
fn load_settings(app: tauri::AppHandle) -> Result<Value, String> {
    let p = settings_path(&app)?;
    Ok(std::fs::read(p)
        .ok()
        .and_then(|s| serde_json::from_slice(&s).ok())
        .unwrap_or(json!({})))
}
#[tauri::command]
fn save_settings(app: tauri::AppHandle, settings: Value) -> Result<(), String> {
    let p = settings_path(&app)?;
    std::fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
    let tmp = p.with_extension("tmp");
    std::fs::write(
        &tmp,
        serde_json::to_vec(&settings).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    std::fs::rename(tmp, p).map_err(|e| e.to_string())
}
#[tauri::command]
fn opacity(window: tauri::WebviewWindow, value: f64) -> Result<(), String> {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::*;
        let hwnd = window.hwnd().map_err(|e| e.to_string())?.0;
        let style = GetWindowLongW(hwnd, GWL_EXSTYLE);
        if style & WS_EX_LAYERED as i32 == 0 {
            SetWindowLongW(hwnd, GWL_EXSTYLE, style | WS_EX_LAYERED as i32);
        }
        if SetLayeredWindowAttributes(hwnd, 0, (value.clamp(0.15, 1.) * 255.) as u8, LWA_ALPHA) == 0
        {
            return Err("Opaklık ayarlanamadı".into());
        }
    }
    #[cfg(target_os = "linux")]
    {
        use gtk::prelude::WidgetExt;
        window
            .gtk_window()
            .map_err(|e| e.to_string())?
            .set_opacity(value.clamp(0.15, 1.));
    }
    *window
        .state::<Appearance>()
        .0
        .lock()
        .map_err(|e| e.to_string())? = Some(value);
    Ok(())
}
#[tauri::command]
fn window_preferences(
    window: tauri::WebviewWindow,
    topmost: Option<bool>,
    alpha: Option<f64>,
    width: Option<f64>,
    height: Option<f64>,
) -> Result<(), String> {
    let position = window.outer_position().ok();
    if let Some(topmost) = topmost {
        window
            .set_always_on_top(topmost)
            .map_err(|e| e.to_string())?;
    }
    if let Some(alpha) = alpha {
        opacity(window.clone(), alpha)?;
    }
    if height.is_some() || width.is_some() {
        let size = window.inner_size().map_err(|e| e.to_string())?;
        let scale = window.scale_factor().map_err(|e| e.to_string())?;
        window
            .set_size(tauri::LogicalSize::new(
                width.unwrap_or(size.width as f64 / scale).max(300.),
                height.unwrap_or(size.height as f64 / scale).max(200.),
            ))
            .map_err(|e| e.to_string())?;
    }
    if let Some(mut position) = position.filter(|_| height.is_some() || width.is_some()) {
        if height.is_some() || width.is_some() {
            if let (Ok(Some(monitor)), Ok(size)) = (window.current_monitor(), window.outer_size()) {
                let area = monitor.work_area();
                position.x = position.x.clamp(
                    area.position.x,
                    (area.position.x + area.size.width as i32 - size.width as i32)
                        .max(area.position.x),
                );
                position.y = position.y.clamp(
                    area.position.y,
                    (area.position.y + area.size.height as i32 - size.height as i32)
                        .max(area.position.y),
                );
            }
        }
        let _ = window.set_position(position);
    }
    let restore = window.clone();
    window
        .run_on_main_thread(move || restore_opacity(&restore))
        .map_err(|e| e.to_string())?;
    Ok(())
}
pub fn diagnose() -> Result<Value, String> {
    let mut r = data::Reader::new();
    let mut v = r.snapshot(None)?;
    let ids: Vec<String> = v["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t["id"].as_str().map(str::to_owned))
        .collect();
    let mut details = vec![];
    for id in ids {
        details.push(r.snapshot(Some(&id))?["detail"].clone());
    }
    v["details"] = json!(details);
    let mut estimator = limits::Estimator::default();
    let estimates = estimator.update_blocking(&v["quota"]);
    v["calibrationDiagnostics"] = estimator.diagnostics(&v["quota"]);
    attach_estimates(&mut v, &estimates);
    Ok(v)
}
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main(app);
        }))
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .manage(updates::UpdateState::default())
        .manage(Mutex::new(data::Reader::new()))
        .manage(quota::Feed::default())
        .manage(Mutex::new(limits::Estimator::default()))
        .manage(Appearance::default())
        .invoke_handler(tauri::generate_handler![
            snapshot,
            load_settings,
            save_settings,
            opacity,
            window_preferences,
            menu_language,
            updates::check_update,
            updates::install_update,
            updates::open_release_page
        ])
        .setup(|app| {
            app.state::<quota::Feed>().start();
            pricing::start(app.path().app_config_dir()?);
            use tauri::tray::TrayIconBuilder;
            let menu = tray_menu(app.handle(), "en")?;
            TrayIconBuilder::with_id("qufu")
                .icon(tauri::image::Image::from_bytes(include_bytes!(
                    "../icons/tray.png"
                ))?)
                .tooltip("Qufu Codex Dashboard")
                .menu(&menu)
                .on_menu_event(|app, event| {
                    if event.id.as_ref() == "quit" {
                        app.exit(0);
                    } else {
                        show_main(app);
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if matches!(event, tauri::tray::TrayIconEvent::DoubleClick { .. }) {
                        show_main(tray.app_handle());
                    }
                })
                .build(app)?;
            if let Some(window) = app.get_webview_window("main") {
                window.show()?;
            }
            Ok(())
        })
        .on_window_event(|w, event| {
            if matches!(event, tauri::WindowEvent::Focused(true)) {
                if let Some(window) = w.app_handle().get_webview_window(w.label()) {
                    restore_opacity(&window);
                }
            }
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = w.hide();
            }
        })
        .build(tauri::generate_context!())
        .expect("Qufu Dashboard başlatılamadı")
        .run(|app, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                app.state::<quota::Feed>().stop();
            }
        });
}
