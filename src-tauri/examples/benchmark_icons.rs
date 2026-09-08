//! 本机图标缓存性能检查，不打开任何应用。
use luma_app::{icon_cache::IconCache, platform};
use std::time::Instant;

fn main() {
    let apps = platform::get_installed_apps();
    let directory = tempfile::tempdir().expect("temporary benchmark cache");
    let cache = IconCache::new(directory.path().into());
    let start = Instant::now();
    let count = apps.iter().filter(|app| cache.get(app).is_some()).count();
    println!(
        "Generated {count}/{} icons: {:?}",
        apps.len(),
        start.elapsed()
    );
    let start = Instant::now();
    for app in &apps {
        let _ = cache.get(app);
    }
    println!("Memory cache, {} icons: {:?}", apps.len(), start.elapsed());
    let disk_cache = IconCache::new(directory.path().into());
    let start = Instant::now();
    for app in &apps {
        let _ = disk_cache.get(app);
    }
    println!(
        "Disk cache after new instance, {} icons: {:?}",
        apps.len(),
        start.elapsed()
    );
}
