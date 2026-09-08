use super::{AppInfo, ApplicationApi};
use gio::prelude::*;
use gio::{AppLaunchContext, DesktopAppInfo, FileIcon};
use std::path::Path;

pub(super) struct LinuxPlatform;

impl ApplicationApi for LinuxPlatform {
    fn get_installed_apps(&self) -> Vec<AppInfo> {
        // GIO 按 XDG_DATA_HOME / XDG_DATA_DIRS 枚举 .desktop，处理用户覆盖、
        // 本地化名称、TryExec 以及桌面环境可见性，无需自行解析 Exec。
        gio::AppInfo::all()
            .into_iter()
            .filter_map(|app| app.downcast::<DesktopAppInfo>().ok())
            .filter_map(desktop_app_info)
            .collect()
    }

    fn launch_app(&self, app_path: &Path) -> Result<(), String> {
        if !app_path.is_file() || app_path.extension().is_none_or(|ext| ext != "desktop") {
            return Err("应用路径必须是 .desktop 文件".into());
        }
        let app = DesktopAppInfo::from_filename(app_path)
            .ok_or_else(|| format!("无法读取桌面应用: {}", app_path.display()))?;
        if app.is_hidden() || !app.should_show() {
            return Err("该桌面应用已隐藏或不适用于当前桌面环境".into());
        }

        // GIO 负责 Exec 引号、% 字段、Path、Terminal 和 D-Bus 激活。
        app.launch(&[], None::<&AppLaunchContext>)
            .map_err(|err| format!("打开应用失败: {err}"))
    }
}

fn desktop_app_info(app: DesktopAppInfo) -> Option<AppInfo> {
    if app.is_hidden() || !app.should_show() {
        return None;
    }
    let path = app.filename()?;
    let icon_path = app
        .icon()
        .and_then(|icon| icon.downcast::<FileIcon>().ok())
        .and_then(|icon| icon.file().path())
        .filter(|path| path.is_file())
        .map(|path| path.to_string_lossy().into_owned());
    // ThemedIcon 是主题名称，不是文件路径，不能直接当 icon_path 返回。
    Some(AppInfo {
        name: app.display_name().to_string(),
        path: path.to_string_lossy().into_owned(),
        icon_path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_desktop(dir: &Path, extra: &str) -> std::path::PathBuf {
        let path = dir.join("test.desktop");
        fs::write(
            &path,
            format!("[Desktop Entry]\nType=Application\nName=Test App\nExec=/bin/true\n{extra}"),
        )
        .unwrap();
        path
    }

    #[test]
    fn reads_desktop_metadata_and_absolute_icon_path() {
        let dir = tempfile::tempdir().unwrap();
        let icon = dir.path().join("icon.png");
        fs::write(&icon, []).unwrap();
        let path = write_desktop(dir.path(), &format!("Icon={}\n", icon.display()));
        let app = desktop_app_info(DesktopAppInfo::from_filename(&path).unwrap()).unwrap();
        assert_eq!(app.name, "Test App");
        assert_eq!(app.path, path.to_string_lossy());
        assert_eq!(app.icon_path.as_deref(), icon.to_str());
    }

    #[test]
    fn filters_hidden_and_no_display_entries() {
        for extra in [
            "Hidden=true\n",
            "NoDisplay=true\n",
            "OnlyShowIn=LumaNonexistentDesktop;\n",
        ] {
            let dir = tempfile::tempdir().unwrap();
            let path = write_desktop(dir.path(), extra);
            assert!(desktop_app_info(DesktopAppInfo::from_filename(&path).unwrap()).is_none());
            assert!(LinuxPlatform.launch_app(&path).is_err());
        }
    }

    #[test]
    fn themed_icon_names_are_not_exposed_as_file_paths() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_desktop(dir.path(), "Icon=utilities-terminal\n");
        let app = desktop_app_info(DesktopAppInfo::from_filename(&path).unwrap()).unwrap();
        assert!(app.icon_path.is_none());
    }

    #[test]
    fn rejects_malformed_desktop_files() {
        let file = tempfile::Builder::new()
            .suffix(".desktop")
            .tempfile()
            .unwrap();
        fs::write(file.path(), "not a desktop entry").unwrap();
        assert!(LinuxPlatform.launch_app(file.path()).is_err());
    }
}
