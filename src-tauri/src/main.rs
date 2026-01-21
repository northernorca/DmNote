#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app_state;
mod commands;
mod cursor;
mod defaults;
mod keyboard;
mod keyboard_daemon;
#[cfg(any(target_os = "windows", target_os = "linux"))]
mod keyboard_labels;
mod ipc;
mod models;
mod services;
mod store;

use anyhow::Result;
use log::LevelFilter;
use std::{thread, time::Duration};

#[cfg(target_os = "windows")]
use std::{fs, path::PathBuf};

use tauri::{ipc::CapabilityBuilder, LogicalSize, Manager, PhysicalPosition, Position};

use app_state::AppState;
use store::AppStore;

fn main() {
    #[cfg(target_os = "windows")]
    {
        // GPU/하드웨어 가속 강제 활성화 및 렌더링 최적화 플래그
        let gpu_flags = [
            "--disable-blink-features=VSync",           // VSync 비활성화 (입력 지연 감소)
            "--enable-gpu-rasterization",               // GPU 래스터화 강제 활성화
            "--enable-zero-copy",                       // 제로 카피 래스터라이저 활성화
            "--ignore-gpu-blocklist",                   // GPU 블랙리스트 무시 (강제 GPU 사용)
        ];
        for flag in gpu_flags {
            apply_webview2_additional_args(flag);
        }

        // 렌더러 설정 적용 (store.json에서 읽어옴)
        apply_renderer_settings();
    }

    if std::env::args().any(|arg| arg == "--keyboard-daemon") {
        if let Err(err) = keyboard_daemon::run() {
            eprintln!("keyboard daemon error: {err:?}");
            std::process::exit(1);
        }
        return;
    }

    #[cfg(target_os = "linux")]
    unsafe {
        // WebKitGTK의 DMA-BUF renderer가 NVIDIA GPU의 explicit sync와 맞물리지 않아, 크래시가
        // 발생하거나 창 redraw가 올바르게 작동하지 않습니다. 다만, DMA-BUF renderer가 꺼져있을
        // 경우 redraw 이슈가 발생하므로 이 부분을 일부터 킵니다.
        // 
        // # Safety
        // Rust 2024 이후로는 std::env::set_var가 unsafe 함수인데, 이는 Unix 계열 OS의 멀티스레딩
        // 환경에서 getenv와 setenv가 사용되면 레이스가 발생할 수 있기 때문입니다.
        // 현재 이 위치는 "스레드"는 하나뿐이기 때문에 안전합니다.
        std::env::set_var("__NV_DISABLE_EXPLICIT_SYNC", "1");
        std::env::remove_var("WEBKIT_DISABLE_DMABUF_RENDERER");
    }

    if let Err(err) = setup_logging() {
        eprintln!("Failed to initialize logging: {err}");
    }

    let context = tauri::generate_context!();

    tauri::Builder::default()
        .setup(|app| {
            register_dev_capability(app)?;
            let resolver = app.path();
            let store = AppStore::initialize(&resolver)
                .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
            let app_state = AppState::initialize(store)
                .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
            app.manage(app_state);
            let handle = app.handle();
            {
                let state = app.state::<AppState>();
                state
                    .initialize_runtime(&handle)
                    .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
            }
            configure_main_window(&app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::app_bootstrap,
            commands::settings::settings_get,
            commands::settings::settings_update,
            commands::keys::keys_get,
            commands::keys::positions_get,
            commands::keys::keys_update,
            commands::keys::positions_update,
            commands::keys::keys_set_mode,
            commands::keys::keys_reset_all,
            commands::keys::keys_reset_mode,
            commands::keys::keys_reset_counters,
            commands::keys::keys_reset_counters_mode,
            commands::keys::keys_reset_single_counter,
            commands::keys::keys_set_counters,
            commands::keys::raw_input_subscribe,
            commands::keys::raw_input_unsubscribe,
            commands::keys::custom_tabs_list,
            commands::keys::custom_tabs_create,
            commands::keys::custom_tabs_delete,
            commands::keys::custom_tabs_select,
            commands::css::css_get,
            commands::css::css_get_use,
            commands::css::css_toggle,
            commands::css::css_reset,
            commands::css::css_set_content,
            commands::css::css_load,
            commands::css::css_tab_get_all,
            commands::css::css_tab_get,
            commands::css::css_tab_load,
            commands::css::css_tab_clear,
            commands::css::css_tab_set,
            commands::css::css_tab_toggle,
            commands::js::js_get,
            commands::js::js_get_use,
            commands::js::js_toggle,
            commands::js::js_reset,
            commands::js::js_set_content,
            commands::js::js_load,
            commands::js::js_reload,
            commands::js::js_remove_plugin,
            commands::js::js_set_plugin_enabled,
            commands::preset::preset_save,
            commands::preset::preset_load,
            commands::overlay::overlay_get,
            commands::overlay::overlay_set_visible,
            commands::overlay::overlay_set_lock,
            commands::overlay::overlay_set_anchor,
            commands::overlay::overlay_resize,
            commands::bridge::plugin_bridge_send,
            commands::bridge::plugin_bridge_send_to,
            commands::plugin_storage::plugin_storage_get,
            commands::plugin_storage::plugin_storage_set,
            commands::plugin_storage::plugin_storage_remove,
            commands::plugin_storage::plugin_storage_clear,
            commands::plugin_storage::plugin_storage_keys,
            commands::plugin_storage::plugin_storage_has_data,
            commands::plugin_storage::plugin_storage_clear_by_prefix,
            commands::system::window_minimize,
            commands::system::window_close,
            commands::system::app_open_external,
            commands::system::app_restart,
            commands::system::window_open_devtools_all,
            commands::system::get_cursor_settings,
        ])
        .run(context)
        .expect("error while running tauri application");
}

#[cfg(target_os = "windows")]
fn apply_webview2_additional_args(arg: &str) {
    use std::env;
    const KEY: &str = "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS";
    // Append the argument if it's not already present to preserve user-provided args.
    let existing = env::var(KEY).unwrap_or_default();
    let already_present = existing
        .split_whitespace()
        .any(|token| token.eq_ignore_ascii_case(arg));
    if already_present {
        return;
    }
    let new_value = if existing.trim().is_empty() {
        arg.to_string()
    } else {
        format!("{existing} {arg}")
    };
    env::set_var(KEY, new_value);
}

/// store.json에서 렌더러 설정(angleMode)을 읽어 WebView2 플래그로 적용
#[cfg(target_os = "windows")]
fn apply_renderer_settings() {
    // Tauri 초기화 전이므로 직접 경로를 찾아야 함
    let store_path = get_store_path();
    
    let angle_mode = if let Some(path) = store_path {
        read_angle_mode_from_store(&path).unwrap_or_else(|| "d3d11".to_string())
    } else {
        "d3d11".to_string()
    };

    // ANGLE 백엔드 또는 Skia 렌더러 설정 적용
    match angle_mode.as_str() {
        "d3d11" => {
            apply_webview2_additional_args("--use-angle=d3d11");
        }
        "d3d9" => {
            apply_webview2_additional_args("--use-angle=d3d9");
        }
        "gl" => {
            apply_webview2_additional_args("--use-angle=gl");
        }
        "skia" => {
            // Skia 렌더러 활성화 (고성능 2D 렌더링)
            apply_webview2_additional_args("--enable-features=UseSkiaRenderer");
            apply_webview2_additional_args("--use-angle=d3d11"); // Skia + D3D11 조합
        }
        _ => {
            // 기본값: D3D11
            apply_webview2_additional_args("--use-angle=d3d11");
        }
    }
}

/// 앱 데이터 디렉토리에서 store.json 경로 찾기
#[cfg(target_os = "windows")]
fn get_store_path() -> Option<PathBuf> {
    // Windows: %APPDATA%/com.dmnote.desktop/store.json
    dirs_next::config_dir().map(|config| config.join("com.dmnote.desktop").join("store.json"))
}

/// store.json에서 angleMode 값 읽기
#[cfg(target_os = "windows")]
fn read_angle_mode_from_store(path: &PathBuf) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    json.get("angleMode")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn setup_logging() -> Result<()> {
    // 개발 모드에서는 Debug, 릴리즈에서는 Info
    let level = if cfg!(debug_assertions) {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    };

    let _ = fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{level}][{target}] {message}",
                level = record.level(),
                target = record.target(),
                message = message
            ))
        })
        .level(level)
        .chain(std::io::stdout())
        .apply();
    Ok(())
}

fn configure_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if let Err(err) = apply_main_window_configuration(app, window) {
            log::warn!("failed to configure main window: {err}");
        }
        return;
    }

    let handle = app.clone();
    thread::spawn(move || {
        for attempt in 0..15 {
            if let Some(window) = handle.get_webview_window("main") {
                if let Err(err) = apply_main_window_configuration(&handle, window) {
                    log::warn!("failed to configure main window: {err}");
                }
                return;
            }

            thread::sleep(Duration::from_millis(25));
            if attempt == 14 {
                log::warn!("main window was not available for configuration");
            }
        }
    });
}

fn apply_main_window_configuration(
    app: &tauri::AppHandle,
    window: tauri::WebviewWindow,
) -> Result<()> {
    let size = LogicalSize::new(902.0, 488.0);

    if let Err(err) = window.hide() {
        log::debug!("failed to hide main window before configuration: {err}");
    }

    if cfg!(target_os = "windows") {
        if let Err(err) = window.set_decorations(false) {
            log::warn!("failed to disable decorations: {err}");
        }
    } else if cfg!(target_os = "macos") {
        if let Err(err) = window.set_decorations(true) {
            log::warn!("failed to enable decorations: {err}");
        }
    }
    if let Err(err) = window.set_resizable(false) {
        log::warn!("failed to disable resizing: {err}");
    }
    if let Err(err) = window.set_maximizable(false) {
        log::warn!("failed to disable maximize: {err}");
    }
    if let Err(err) = window.set_min_size(Some(tauri::Size::Logical(size))) {
        log::warn!("failed to set min size: {err}");
    }
    if let Err(err) = window.set_max_size(Some(tauri::Size::Logical(size))) {
        log::warn!("failed to set max size: {err}");
    }
    if let Err(err) = window.set_size(tauri::Size::Logical(size)) {
        log::warn!("failed to set size: {err}");
    }
    if let Err(err) = window.set_shadow(true) {
        log::warn!("failed to enable shadow: {err}");
    }

    let positioned = app.primary_monitor().ok().flatten().and_then(|monitor| {
        let work_area = monitor.work_area();
        window.outer_size().ok().map(|size| {
            let width = size.width as f64;
            let height = size.height as f64;
            let origin_x = work_area.position.x as f64;
            let origin_y = work_area.position.y as f64;
            let available_width = work_area.size.width as f64;
            let available_height = work_area.size.height as f64;

            let desired_x = origin_x + (available_width - width) / 2.0;
            let desired_y = origin_y + (available_height - height) / 2.0;

            let max_x = origin_x + (available_width - width).max(0.0);
            let max_y = origin_y + (available_height - height).max(0.0);

            (
                desired_x.clamp(origin_x, max_x),
                desired_y.clamp(origin_y, max_y),
            )
        })
    });

    if let Some((x, y)) = positioned {
        if let Err(err) = window.set_position(Position::Physical(PhysicalPosition::new(
            x.round() as i32,
            y.round() as i32,
        ))) {
            log::warn!("failed to set main window position: {err}");
            if let Err(err) = window.center() {
                log::warn!("failed to center window: {err}");
            }
        }
    } else if let Err(err) = window.center() {
        log::warn!("failed to center window: {err}");
    }

    if let Err(err) = window.show() {
        log::warn!("failed to show main window after configuration: {err}");
    }
    Ok(())
}

fn register_dev_capability(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    const DEV_URLS: &[&str] = &[
        "http://localhost:3400",
        "http://127.0.0.1:3400",
        "http://localhost:3000",
        "http://127.0.0.1:3000",
        "tauri://localhost",
    ];

    let builder = DEV_URLS.iter().fold(
        CapabilityBuilder::new("dmnote-dev")
            .local(true)
            .windows(["main", "overlay"])
            .webviews(["main", "overlay"])
            .permission("dmnote-allow-all"),
        |acc, url| acc.remote((*url).to_string()),
    );

    app.add_capability(builder)
        .map_err(|err| -> Box<dyn std::error::Error> { err.into() })
}
