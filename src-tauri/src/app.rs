//! Luma 应用入口：注册 Tauri 命令并初始化桌面应用。

pub mod icon_cache;
pub mod platform;

use icon_cache::IconCache;
pub use platform::AppInfo;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tauri::Manager;

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
    cache: tauri::State<'_, Arc<IconCache>>,
) -> Result<Option<String>, String> {
    let app = catalog
        .0
        .lock()
        .map_err(|err| err.to_string())?
        .get(&app_path)
        .cloned()
        .ok_or("应用不在当前扫描列表中")?;
    let cache = Arc::clone(&cache);
    tauri::async_runtime::spawn_blocking(move || cache.get(&app))
        .await
        .map_err(|err| format!("读取应用图标失败: {err}"))
}

#[tauri::command]
async fn get_app_icons(
    app_paths: Vec<String>,
    catalog: tauri::State<'_, AppCatalog>,
    cache: tauri::State<'_, Arc<IconCache>>,
) -> Result<HashMap<String, Option<String>>, String> {
    if app_paths.len() > 12 {
        return Err("每批最多读取 12 个应用图标".into());
    }
    let apps: Vec<_> = {
        let catalog = catalog.0.lock().map_err(|err| err.to_string())?;
        app_paths
            .iter()
            .map(|path| {
                catalog
                    .get(path)
                    .cloned()
                    .ok_or_else(|| format!("应用不在当前扫描列表中: {path}"))
            })
            .collect::<Result<_, _>>()?
    };
    let jobs: Vec<_> = apps
        .into_iter()
        .map(|app| {
            let cache = Arc::clone(&cache);
            tauri::async_runtime::spawn_blocking(move || {
                let icon = cache.get(&app);
                (app.path, icon)
            })
        })
        .collect();
    let mut result = HashMap::new();
    for job in jobs {
        let (path, icon) = job.await.map_err(|err| err.to_string())?;
        result.insert(path, icon);
    }
    Ok(result)
}

#[tauri::command]
async fn clear_icon_cache(cache: tauri::State<'_, Arc<IconCache>>) -> Result<(), String> {
    let cache = Arc::clone(&cache);
    tauri::async_runtime::spawn_blocking(move || cache.clear())
        .await
        .map_err(|err| err.to_string())?
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn close_app(app: tauri::AppHandle) {
    app.exit(0);
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
        .setup(|app| {
            app.manage(Arc::new(IconCache::new(
                app.path().app_cache_dir()?.join("icons"),
            )));

            if let Some(window) = app.get_webview_window("main") {
                #[cfg(target_os = "macos")]
                {
                    use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial};
                    if let Err(err) = apply_vibrancy(&window, NSVisualEffectMaterial::FullScreenUI, None, Some(20.0)) {
                        eprintln!("应用 macOS 毛玻璃效果失败: {err}");
                    }
                }

                #[cfg(target_os = "windows")]
                {
                    use window_vibrancy::apply_mica;
                    let _ = apply_mica(&window, None);
                }
            }

            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_installed_apps,
            get_app_icon,
            get_app_icons,
            clear_icon_cache,
            close_app,
            launch_app
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
