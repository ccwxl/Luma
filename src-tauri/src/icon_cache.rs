//! 缓存平台图标，避免每次打开窗口或翻页都调用系统图标服务。
use crate::platform::AppInfo;
use std::collections::{hash_map::DefaultHasher, HashMap};
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Mutex,
};

static CACHE_WRITE_ID: AtomicU64 = AtomicU64::new(0);

pub struct IconCache {
    directory: PathBuf,
    memory: Mutex<HashMap<String, String>>,
}

impl IconCache {
    pub fn new(directory: PathBuf) -> Self {
        Self {
            directory,
            memory: Mutex::new(HashMap::new()),
        }
    }

    pub fn get(&self, app: &AppInfo) -> Option<String> {
        self.get_or_generate(app, || crate::platform::get_app_icon(app))
    }

    fn get_or_generate(
        &self,
        app: &AppInfo,
        generate: impl FnOnce() -> Option<String>,
    ) -> Option<String> {
        let key = cache_key(app);
        if let Some(value) = self.memory.lock().ok()?.get(&key).cloned() {
            return Some(value);
        }
        let file = self.directory.join(format!("{key}.icon"));
        let cached = std::fs::metadata(&file)
            .ok()
            .filter(|meta| meta.len() <= 4 * 1024 * 1024)
            .and_then(|_| std::fs::read_to_string(&file).ok())
            .filter(|value| value.starts_with("data:image/"));
        let value = match cached {
            Some(value) => value,
            None => {
                let value = generate()?;
                // 缓存不可写时仍返回图标，不影响应用启动器使用。
                if std::fs::create_dir_all(&self.directory).is_ok() {
                    let temporary = self.directory.join(format!(
                        "{key}.{}.{}.tmp",
                        std::process::id(),
                        CACHE_WRITE_ID.fetch_add(1, Ordering::Relaxed)
                    ));
                    if std::fs::write(&temporary, &value).is_ok() {
                        // 原子替换，避免并发读取到未写完的 data URL。
                        let _ = std::fs::rename(&temporary, file);
                    }
                    let _ = std::fs::remove_file(temporary);
                }
                value
            }
        };
        self.memory.lock().ok()?.insert(key, value.clone());
        Some(value)
    }

    pub fn clear(&self) -> std::io::Result<()> {
        if let Ok(mut memory) = self.memory.lock() {
            memory.clear();
        }
        let entries = match std::fs::read_dir(&self.directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error),
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "icon") && path.is_file() {
                std::fs::remove_file(path)?;
            }
        }
        Ok(())
    }
}

fn cache_key(app: &AppInfo) -> String {
    let mut hash = DefaultHasher::new();
    "luma-icon-256-v1".hash(&mut hash);
    app.path.hash(&mut hash);
    app.icon_path.hash(&mut hash);
    // 应用安装/更新或图标文件变化后自动重新读取。
    for path in std::iter::once(app.path.as_str()).chain(app.icon_path.as_deref()) {
        std::fs::metadata(path)
            .ok()
            .and_then(|meta| meta.modified().ok())
            .hash(&mut hash);
    }
    format!("{:016x}", hash.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> AppInfo {
        AppInfo {
            name: "Test".into(),
            path: "/test.app".into(),
            icon_path: None,
        }
    }

    #[test]
    fn reuses_disk_cache_across_instances_and_regenerates_after_clear() {
        let dir = tempfile::tempdir().unwrap();
        let first = IconCache::new(dir.path().into());
        let icon = "data:image/png;base64,aWNvbg==";
        assert_eq!(
            first
                .get_or_generate(&app(), || Some(icon.into()))
                .as_deref(),
            Some(icon)
        );
        let second = IconCache::new(dir.path().into());
        assert_eq!(
            second
                .get_or_generate(&app(), || panic!("must reuse disk cache"))
                .as_deref(),
            Some(icon)
        );
        second.clear().unwrap();
        assert_eq!(
            second
                .get_or_generate(&app(), || Some("new icon".into()))
                .as_deref(),
            Some("new icon")
        );
    }

    #[test]
    fn changed_icon_source_invalidates_cache_and_clear_preserves_other_files() {
        let dir = tempfile::tempdir().unwrap();
        let cache = IconCache::new(dir.path().into());
        cache.get_or_generate(&app(), || Some("data:image/png;base64,b2xk".into()));
        let mut changed = app();
        changed.icon_path = Some("/updated.png".into());
        assert_eq!(
            cache
                .get_or_generate(&changed, || Some("updated".into()))
                .as_deref(),
            Some("updated")
        );
        let unrelated = dir.path().join("keep.txt");
        std::fs::write(&unrelated, "keep").unwrap();
        cache.clear().unwrap();
        assert!(unrelated.exists());
    }
}
