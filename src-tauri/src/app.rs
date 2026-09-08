//! Luma 应用入口：注册 Tauri 命令并初始化桌面应用。

pub mod platform;

pub use platform::AppInfo;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

#[derive(Default)]
struct AppCatalog(Mutex<HashMap<String, AppInfo>>);

/// Tauri 只负责 IPC；平台扫描放在后台线程，避免阻塞界面。
#[tauri::command]
async fn get_installed_apps(catalog: tauri::State<'_, AppCatalog>) -> Result<Vec<AppInfo>, String> {
    let apps = tauri::async_runtime::spawn_blocking(platform::get_installed_apps)
        .await
        .map_err(|err| format!("扫描应用任务失败: {err}"))?;
    *catalog.0.lock().map_err(|err| err.to_string())? = apps
        .iter()
        .cloned()
        .map(|app| (app.path.clone(), app))
        .collect();
    Ok(apps)
}

#[tauri::command]
async fn get_app_icon(
    app_path: String,
    catalog: tauri::State<'_, AppCatalog>,
) -> Result<Option<String>, String> {
    let app = catalog
        .0
        .lock()
        .map_err(|err| err.to_string())?
        .get(&app_path)
        .cloned()
        .ok_or("应用不在当前扫描列表中")?;
    tauri::async_runtime::spawn_blocking(move || platform::get_app_icon(&app))
        .await
        .map_err(|err| format!("读取应用图标失败: {err}"))
}

#[tauri::command]
async fn launch_app(app_path: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || platform::launch_app(Path::new(&app_path)))
        .await
        .map_err(|err| format!("启动应用任务失败: {err}"))?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppCatalog::default())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_installed_apps,
            get_app_icon,
            launch_app
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
