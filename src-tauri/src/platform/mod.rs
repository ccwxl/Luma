use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
use linux::LinuxPlatform as NativePlatform;
#[cfg(target_os = "macos")]
use macos::MacOsPlatform as NativePlatform;
#[cfg(target_os = "windows")]
use windows::WindowsPlatform as NativePlatform;

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
compile_error!("Luma currently supports Windows, Linux, and macOS only");

/// 前端共用的应用信息。path 是平台启动入口，不一定是可执行文件：
/// macOS 为 .app，Windows 为开始菜单快捷方式，Linux 为 .desktop。
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppInfo {
    pub name: String,
    pub path: String,
    /// 仅在能获得本地图标文件时返回路径。
    pub icon_path: Option<String>,
}

/// 所有桌面平台都必须实现的应用管理接口。
trait ApplicationApi {
    fn get_installed_apps(&self) -> Vec<AppInfo>;
    fn launch_app(&self, app_path: &Path) -> Result<(), String>;

    fn get_app_icon(&self, app: &AppInfo) -> Option<String> {
        let path = Path::new(app.icon_path.as_ref()?);
        let mime = match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "svg" => "image/svg+xml",
            "ico" => "image/x-icon",
            "webp" => "image/webp",
            _ => return None,
        };
        // 图标不应是任意大小的文件；只读取平台扫描得到的本地图标。
        if std::fs::metadata(path).ok()?.len() > 4 * 1024 * 1024 {
            return None;
        }
        Some(image_data_url(mime, &std::fs::read(path).ok()?))
    }
}

/// 返回 WebView 可直接显示的图标，避免暴露整个文件系统给前端。
pub fn get_app_icon(app: &AppInfo) -> Option<String> {
    NativePlatform.get_app_icon(app)
}

fn image_data_url(mime: &str, bytes: &[u8]) -> String {
    format!("data:{mime};base64,{}", STANDARD.encode(bytes))
}

/// 根据编译目标选择平台实现，统一排序、去重。
pub fn get_installed_apps() -> Vec<AppInfo> {
    normalize_apps(NativePlatform.get_installed_apps())
}

pub fn launch_app(app_path: &Path) -> Result<(), String> {
    if !app_path.is_absolute() || !app_path.exists() {
        return Err(format!(
            "应用路径不存在或不是绝对路径: {}",
            app_path.display()
        ));
    }
    NativePlatform.launch_app(app_path)
}

fn normalize_apps(mut apps: Vec<AppInfo>) -> Vec<AppInfo> {
    // 先去重再排序，避免同一路径的不同名称在排序后不相邻。
    let mut seen = HashSet::new();
    apps.retain(|app| seen.insert(app.path.clone()));
    apps.sort_by_cached_key(|app| (app.name.to_lowercase(), app.path.clone()));
    apps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorts_names_and_deduplicates_non_adjacent_paths() {
        let apps = normalize_apps(
            [
                ("Zulu", "/same"),
                ("alpha", "/alpha"),
                ("Alias", "/same"),
                ("Beta", "/beta"),
            ]
            .into_iter()
            .map(|(name, path)| AppInfo {
                name: name.into(),
                path: path.into(),
                icon_path: None,
            })
            .collect(),
        );

        assert_eq!(
            apps.iter().map(|app| app.name.as_str()).collect::<Vec<_>>(),
            ["alpha", "Beta", "Zulu"]
        );
    }

    #[test]
    fn rejects_invalid_launch_paths_before_calling_the_os() {
        assert!(launch_app(Path::new("")).is_err());
        assert!(launch_app(Path::new("relative.app")).is_err());
        let dir = tempfile::tempdir().unwrap();
        assert!(launch_app(&dir.path().join("missing.app")).is_err());
    }
}
