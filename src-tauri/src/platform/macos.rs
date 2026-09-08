use super::{AppInfo, ApplicationApi};
use objc2::{rc::autoreleasepool, AllocAnyThread};
use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep, NSWorkspace};
use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize, NSString};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub(super) struct MacOsPlatform;

impl ApplicationApi for MacOsPlatform {
    fn get_app_icon(&self, app: &AppInfo) -> Option<String> {
        autoreleasepool(|_| {
            // 系统图标接口也支持仅在 Assets.car 中提供图标的 .app。
            let icon = NSWorkspace::sharedWorkspace().iconForFile(&NSString::from_str(&app.path));
            // 只生成适合 Retina 图标的尺寸，不导出 NSImage 的所有 TIFF 分辨率。
            let mut rect = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(128.0, 128.0));
            // SAFETY: rect 在调用期间有效；不提供上下文或未验证的 hints。
            let image =
                unsafe { icon.CGImageForProposedRect_context_hints(&mut rect, None, None) }?;
            let bitmap = NSBitmapImageRep::initWithCGImage(NSBitmapImageRep::alloc(), &image);
            // SAFETY: 空字典不含类型不匹配的图片编码参数。
            let png = unsafe {
                bitmap.representationUsingType_properties(
                    NSBitmapImageFileType::PNG,
                    &NSDictionary::new(),
                )
            }?;
            Some(super::image_data_url("image/png", &png.to_vec()))
        })
    }

    fn get_installed_apps(&self) -> Vec<AppInfo> {
        let mut search_dirs = vec![
            PathBuf::from("/Applications"),
            PathBuf::from("/System/Applications"),
        ];
        if let Some(home) = std::env::var_os("HOME") {
            search_dirs.push(PathBuf::from(home).join("Applications"));
        }

        let mut apps = Vec::new();
        for dir in search_dirs {
            scan_dir(&dir, 0, &mut apps);
        }
        apps
    }

    fn launch_app(&self, app_path: &Path) -> Result<(), String> {
        if !app_path.is_dir() || app_path.extension().is_none_or(|ext| ext != "app") {
            return Err("应用路径必须是 .app 目录".into());
        }

        // open 很快返回；检查退出码才能识别无效 bundle 等启动失败。
        let output = Command::new("/usr/bin/open")
            .arg(app_path)
            .output()
            .map_err(|err| format!("打开应用失败: {err}"))?;
        if !output.status.success() {
            return Err(format!(
                "打开应用失败 ({}): {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Ok(())
    }
}

fn scan_dir(dir: &Path, depth: usize, apps: &mut Vec<AppInfo>) {
    if depth > 2 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let file_name = entry.file_name();
        if file_name.to_string_lossy().starts_with('.') || !path.is_dir() {
            continue;
        }

        if path.extension().is_some_and(|ext| ext == "app") {
            apps.push(AppInfo {
                name: path
                    .file_stem()
                    .unwrap_or(&file_name)
                    .to_string_lossy()
                    .into_owned(),
                path: path.to_string_lossy().into_owned(),
                icon_path: find_app_icon(&path),
            });
        } else {
            scan_dir(&path, depth + 1, apps);
        }
    }
}

fn find_app_icon(app_path: &Path) -> Option<String> {
    let resources = app_path.join("Contents/Resources");
    let mut icons: Vec<_> = fs::read_dir(resources)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "icns"))
        .collect();
    icons.sort();
    icons
        .first()
        .map(|path| path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_nested_apps_without_entering_bundles_or_hidden_directories() {
        let dir = tempfile::tempdir().unwrap();
        for relative in [
            "Editor.app/Contents/Resources",
            "Editor.app/Contents/Helper.app",
            "Utilities/Tool.app",
            ".hidden/Secret.app",
            "one/two/three/TooDeep.app",
        ] {
            fs::create_dir_all(dir.path().join(relative)).unwrap();
        }
        let icon = dir.path().join("Editor.app/Contents/Resources/editor.icns");
        fs::write(&icon, []).unwrap();

        let mut apps = Vec::new();
        scan_dir(dir.path(), 0, &mut apps);
        apps.sort_by(|a, b| a.name.cmp(&b.name));

        assert_eq!(
            apps.iter().map(|app| app.name.as_str()).collect::<Vec<_>>(),
            ["Editor", "Tool"]
        );
        assert_eq!(apps[0].icon_path.as_deref(), icon.to_str());
        assert!(apps[1].icon_path.is_none());
    }

    #[test]
    fn rejects_files_and_non_app_directories() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("fake.app");
        fs::write(&file, []).unwrap();
        assert!(MacOsPlatform.launch_app(&file).is_err());
        assert!(MacOsPlatform.launch_app(dir.path()).is_err());
    }
}
