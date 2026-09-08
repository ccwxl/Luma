# Luma

基于 Tauri 2、React 和 TypeScript 的桌面应用启动器，支持 Windows、Linux 和 macOS。

## 界面与快捷键

界面采用模糊背景、居中搜索和应用图标网格，保留底部分页，不显示 Dock 或页内品牌文字。
页面通过 GPU `translate3d` 横向滑动；触控板和触摸滑动可跟随手势，鼠标滚轮也可翻页。

| 快捷键 | 操作 |
| --- | --- |
| `←` / `→` | 上一页 / 下一页 |
| `Enter` | 打开第一个搜索结果，输入法确认文字时不触发 |
| `⌘ ,`（Windows/Linux 也支持 `Ctrl ,`） | 打开设置 |
| `Esc` | 退出 Luma |
| `⌘ F` / `Ctrl F` | 聚焦搜索 |

设置中可调整背景模糊、减少动态效果以及清理图标缓存。外观设置保存在本机。

图标使用前端内存缓存、图片预解码和相邻页预加载；后端按批读取，并把图标缓存在应用缓存目录的
`icons/` 中。应用或图标文件更新后会自动失效。macOS 通过系统图标接口生成 WebView 可显示的 PNG；
没有可用图标的平台入口显示通用应用图标。

## 平台接口

`src-tauri/src/app.rs` 是应用入口，负责 Tauri 命令和应用初始化。应用管理能力由
`src-tauri/src/platform/mod.rs` 提供统一接口，使用 `cfg(target_os)` 在编译时选择实现。

| 平台 | 应用来源 | 启动方式 |
| --- | --- | --- |
| macOS | `/Applications`、`/System/Applications`、`~/Applications` 中的 `.app`，最多下探两层子目录 | `/usr/bin/open`，检查退出状态 |
| Windows | 当前用户和所有用户的开始菜单 Programs 目录，递归扫描 `.lnk`、`.exe`、`.appref-ms` | 原生 `ShellExecuteExW`，保留快捷方式参数和工作目录 |
| Linux | GIO 按 `XDG_DATA_HOME`、`XDG_DATA_DIRS` 枚举已注册的 `.desktop` | GIO `DesktopAppInfo::launch`，处理 `Exec`、`Terminal`、工作目录和 D-Bus 激活 |

前端 API：

- `invoke<AppInfo[]>("get_installed_apps")`：返回按名称排序、按路径去重的应用列表。
- `invoke("launch_app", { appPath: app.path })`：启动所选应用；失败时 Promise reject。
- `invoke("get_app_icons", { appPaths })`：批量返回图标 data URL，只接受当前扫描列表中的应用，单批最多 12 个。
- `invoke("clear_icon_cache")`：清理 Luma 自身生成的图标缓存。
- `invoke("close_app")`：退出应用。
- `AppInfo` 字段为 `name: string`、`path: string`、`icon_path: string | null`。

`path` 是平台启动入口的绝对路径，前端应原样传回。`icon_path` 仅在已取得本地图标文件时返回；
Windows 嵌入图标和 Linux 主题图标目前返回 `null`。Windows 列表覆盖开始菜单中注册的应用，
没有开始菜单入口的便携应用或商店应用不在扫描范围内。Linux 遵循桌面环境的隐藏规则及用户级覆盖规则。

扫描和启动运行在 Tauri 后台线程。新增平台能力时，先扩展 `ApplicationApi`，再补齐各平台实现。

## 开发与验证

先安装 [Tauri 对应系统依赖](https://v2.tauri.app/start/prerequisites/)、Rust、Node.js 22.12+ 和 pnpm。

```sh
pnpm install
pnpm tauri dev
```

```sh
pnpm build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/Cargo.toml --lib --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings
```

需要在浏览器中预览本机应用时，先运行 `cargo run --manifest-path src-tauri/Cargo.toml --example export_preview`，
再运行 `pnpm dev`。生成的 `.preview/` 只保存在本机，不进入版本控制或生产包；浏览器预览不会启动本机应用。

图标缓存性能检查：`cargo run --manifest-path src-tauri/Cargo.toml --example benchmark_icons`。
该检查分别测量首次生成、内存读取和新缓存实例的磁盘读取，不包含前端渲染或 IPC 耗时。

GitHub Actions 在三个系统上分别构建前端、编译 Rust 并运行对应平台的测试。
测试使用临时目录，覆盖扫描、过滤、去重和无效启动入口，不会打开真实应用。
实际桌面环境中的应用启动仍需在对应系统进行验证。

## 更新日志

### v0.1.0
- 🎉 **首个版本发布**：基于 Tauri 2 + React + TypeScript 构建的现代化桌面应用启动器。
- 🖥️ **跨平台支持**：全面支持 macOS、Windows 与 Linux 桌面平台。
- 🔍 **应用智能扫描**：自动扫描系统已安装应用，支持实时搜索与过滤。
- 🚀 **一键快速启动**：点击应用图标即可直接调起本地对应桌面应用程序。
- 🎨 **Launchpad 原生美学**：极简毛玻璃界面、自适应网格布局与流畅的多页手势/键盘滑动切换。
