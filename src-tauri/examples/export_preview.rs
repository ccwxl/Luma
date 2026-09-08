//! 只为本机浏览器预览导出应用数据；.preview 不进入生产包或版本控制。
use luma_app::platform;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let apps: Vec<_> = platform::get_installed_apps()
        .into_iter()
        .map(|app| {
            let icon_data = platform::get_app_icon(&app);
            let mut value = serde_json::to_value(app).expect("AppInfo is serializable");
            value["icon_data"] = icon_data.into();
            value
        })
        .collect();
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.preview");
    std::fs::create_dir_all(&directory)?;
    std::fs::write(directory.join("apps.json"), serde_json::to_vec(&apps)?)?;
    println!("已导出 {} 个应用到本机 .preview/apps.json", apps.len());
    Ok(())
}
