use super::{AppInfo, ApplicationApi};
use std::collections::HashSet;
use std::ffi::OsString;
use std::fs;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::ptr;
use windows_sys::core::GUID;
use windows_sys::Win32::System::Com::{
    CoInitializeEx, CoTaskMemFree, CoUninitialize, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
};
use windows_sys::Win32::UI::Shell::{
    FOLDERID_CommonPrograms, FOLDERID_Programs, SHGetKnownFolderPath, ShellExecuteExW,
    KF_FLAG_DEFAULT, SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC, SHELLEXECUTEINFOW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

pub(super) struct WindowsPlatform;

impl ApplicationApi for WindowsPlatform {
    fn get_installed_apps(&self) -> Vec<AppInfo> {
        // Known Folder API 支持用户移动/重定向后的开始菜单目录。
        let dirs = [FOLDERID_Programs, FOLDERID_CommonPrograms]
            .iter()
            .filter_map(known_folder)
            .collect();
        scan_start_menu(dirs)
    }

    fn launch_app(&self, app_path: &Path) -> Result<(), String> {
        if !app_path.is_file() || !is_launcher(app_path) {
            return Err("应用路径必须是 .lnk、.exe 或 .appref-ms 文件".into());
        }
        let mut path: Vec<u16> = app_path.as_os_str().encode_wide().collect();
        if path.contains(&0) {
            return Err("应用路径包含空字符".into());
        }
        path.push(0);

        // Tauri 后台线程不保证已初始化 COM；Shell 扩展可能需要它。
        let _com = ComApartment::new()?;
        let mut info = SHELLEXECUTEINFOW {
            cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
            fMask: SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI,
            lpVerb: windows_sys::core::w!("open"),
            lpFile: path.as_ptr(),
            nShow: SW_SHOWNORMAL,
            ..Default::default()
        };
        // SAFETY: info 已初始化，所有字符串均以 NUL 结尾且在调用期间有效。
        // NOASYNC 让 Shell 完成启动请求后返回，不等待被启动的应用退出。
        if unsafe { ShellExecuteExW(&mut info) } == 0 {
            return Err(format!("打开应用失败: {}", std::io::Error::last_os_error()));
        }
        Ok(())
    }
}

struct ComApartment;

impl ComApartment {
    fn new() -> Result<Self, String> {
        // SAFETY: 在当前线程初始化，并由 Drop 在同一线程配对释放。
        let result = unsafe {
            CoInitializeEx(
                ptr::null(),
                (COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) as u32,
            )
        };
        if result < 0 {
            return Err(format!("初始化应用启动环境失败: HRESULT {result:#x}"));
        }
        Ok(Self)
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        // SAFETY: 只为成功的 CoInitializeEx 调用释放一次。
        unsafe { CoUninitialize() };
    }
}

fn known_folder(id: &GUID) -> Option<PathBuf> {
    let mut path = ptr::null_mut();
    // SAFETY: id 和输出指针有效；返回的字符串由 CoTaskMemFree 释放。
    let result =
        unsafe { SHGetKnownFolderPath(id, KF_FLAG_DEFAULT as u32, ptr::null_mut(), &mut path) };
    if path.is_null() {
        return None;
    }
    let folder = if result >= 0 {
        // SAFETY: 成功时 Windows 返回以 NUL 结尾的 UTF-16 字符串。
        unsafe {
            let mut len = 0;
            while *path.add(len) != 0 {
                len += 1;
            }
            Some(PathBuf::from(OsString::from_wide(
                std::slice::from_raw_parts(path, len),
            )))
        }
    } else {
        None
    };
    // SAFETY: path 是 Known Folder API 分配的内存，尚未释放。
    unsafe { CoTaskMemFree(path.cast()) };
    folder
}

fn is_launcher(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            ["lnk", "exe", "appref-ms"]
                .iter()
                .any(|allowed| ext.eq_ignore_ascii_case(allowed))
        })
}

fn scan_start_menu(mut dirs: Vec<PathBuf>) -> Vec<AppInfo> {
    let mut apps = Vec::new();
    let mut visited = HashSet::new();
    while let Some(dir) = dirs.pop() {
        let Ok(canonical) = fs::canonicalize(&dir) else {
            continue;
        };
        // 防止目录 junction / symlink 形成递归循环。
        if !visited.insert(canonical) {
            continue;
        }
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                dirs.push(path);
            } else if path.is_file() && is_launcher(&path) {
                apps.push(AppInfo {
                    name: path
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                    path: path.to_string_lossy().into_owned(),
                    // 快捷方式/可执行文件的图标通常是嵌入资源，尚未导出到本地文件。
                    icon_path: None,
                });
            }
        }
    }
    apps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_nested_start_menu_launchers_and_skips_other_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("工具/More/Nested")).unwrap();
        for relative in [
            "Editor.LNK",
            "工具/More/Nested/Tool.exe",
            "Installer.appref-ms",
            "Readme.txt",
            "Website.url",
        ] {
            fs::write(dir.path().join(relative), []).unwrap();
        }
        let mut apps = scan_start_menu(vec![dir.path().into(), dir.path().into()]);
        apps.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(
            apps.iter().map(|app| app.name.as_str()).collect::<Vec<_>>(),
            ["Editor", "Installer", "Tool"]
        );
    }

    #[test]
    fn rejects_documents_without_opening_them() {
        let file = tempfile::Builder::new().suffix(".txt").tempfile().unwrap();
        assert!(WindowsPlatform.launch_app(file.path()).is_err());
    }
}
