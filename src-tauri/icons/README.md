# Luma 应用图标

原图保存在 `source/app-icon.png`（1024 × 1024，RGBA），来自用户指定的：

`/Users/wxl/opensource/Launchpad/Icons/Launchpad Icon.icon/Assets/icon_1024x1024-dark.png`

使用深色版本作为三平台统一应用图标。此目录中的桌面图标由 Tauri CLI 生成：

- `icon.icns`：macOS。
- `icon.ico` 和 `Square*Logo.png`、`StoreLogo.png`：Windows。
- 各尺寸 PNG：Linux、Tauri 窗口图标。
- `public/assets/luma.png`：与 `32x32.png` 相同的浏览器标签页图标。

重新生成时，在项目根目录运行 `pnpm tauri icon src-tauri/icons/source/app-icon.png`。
然后将生成的 `32x32.png` 同步复制到 `public/assets/luma.png`。
