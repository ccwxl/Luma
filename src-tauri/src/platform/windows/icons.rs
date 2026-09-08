//! 让 Shell 解析快捷方式的自定义图标、目标程序资源及系统图标。
use super::{is_launcher, ComApartment};
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use windows::core::PCWSTR;
use windows::Win32::Foundation::SIZE;
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, BITMAP, BITMAPINFO,
    BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP, HDC,
};
use windows::Win32::UI::Shell::{
    IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_ICONONLY,
};

const ICON_SIZE: i32 = 256;

pub(super) fn get_app_icon(path: &Path) -> Option<String> {
    if !path.is_absolute() || !path.is_file() || !is_launcher(path) {
        return None;
    }
    let mut path: Vec<u16> = path.as_os_str().encode_wide().collect();
    if path.contains(&0) {
        return None;
    }
    path.push(0);
    let _com = ComApartment::new().ok()?;
    // SAFETY: 当前线程已初始化 COM，path 在调用期间保持有效并以 NUL 结尾。
    // 传入 .lnk 本身，由 Shell 保留其自定义 IconLocation，而非只读取目标 exe。
    let factory: IShellItemImageFactory =
        unsafe { SHCreateItemFromParsingName(PCWSTR(path.as_ptr()), None) }.ok()?;
    // SAFETY: factory 在当前线程创建；仅请求图标，不生成文档缩略图。
    // 此函数由 Tauri 后台线程调用，磁盘和 Shell 扩展不会阻塞 UI。
    let bitmap = OwnedBitmap(
        unsafe {
            factory.GetImage(
                SIZE {
                    cx: ICON_SIZE,
                    cy: ICON_SIZE,
                },
                SIIGBF_ICONONLY,
            )
        }
        .ok()?,
    );
    let (width, height, pixels) = bitmap_rgba(&bitmap)?;
    Some(crate::platform::image_data_url(
        "image/png",
        &encode_png(width, height, &pixels)?,
    ))
}

// 所有提前返回路径也必须释放 GDI 句柄；COM 接口由 windows crate 自动释放。
struct OwnedBitmap(HBITMAP);

impl Drop for OwnedBitmap {
    fn drop(&mut self) {
        // SAFETY: 此句柄由 GetImage 创建，未选入 DC，且只释放一次。
        let _ = unsafe { DeleteObject(self.0.into()) };
    }
}

struct OwnedDc(HDC);

impl Drop for OwnedDc {
    fn drop(&mut self) {
        // SAFETY: 此 DC 由 CreateCompatibleDC 创建，且只释放一次。
        let _ = unsafe { DeleteDC(self.0) };
    }
}

fn bitmap_rgba(bitmap: &OwnedBitmap) -> Option<(u32, u32, Vec<u8>)> {
    let mut object = BITMAP::default();
    // SAFETY: object 的类型、大小和输出指针一致，bitmap 在整个调用期间有效。
    if unsafe {
        GetObjectW(
            bitmap.0.into(),
            std::mem::size_of::<BITMAP>() as i32,
            Some((&mut object as *mut BITMAP).cast()),
        )
    } == 0
    {
        return None;
    }
    let width = object.bmWidth;
    let height = object.bmHeight.checked_abs()?;
    if !(1..=ICON_SIZE).contains(&width) || !(1..=ICON_SIZE).contains(&height) {
        return None;
    }
    let mut pixels = vec![0; (width * height * 4) as usize];
    let mut info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            // 负高度要求自顶向下输出，避免图标上下颠倒。
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    // SAFETY: None 创建与当前屏幕兼容的内存 DC。
    let dc = OwnedDc(unsafe { CreateCompatibleDC(None) });
    if dc.0.is_invalid() {
        return None;
    }
    // SAFETY: pixels 足够容纳 width * height 个 32 位像素；bitmap 未选入任何 DC。
    let rows = unsafe {
        GetDIBits(
            dc.0,
            bitmap.0,
            0,
            height as u32,
            Some(pixels.as_mut_ptr().cast()),
            &mut info,
            DIB_RGB_COLORS,
        )
    };
    if rows != height {
        return None;
    }
    // ICONONLY 的图标像素按 BGRA 输出；交换红蓝通道，保留原始透明度。
    // 不再次反预乘颜色，否则快捷方式的半透明 PNG 图标会变亮、失真。
    let has_alpha = object.bmBitsPixel == 32;
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.swap(0, 2);
        if !has_alpha {
            pixel[3] = 255;
        }
        if pixel[3] == 0 {
            pixel.fill(0);
        }
    }
    Some((width as u32, height as u32, pixels))
}

fn encode_png(width: u32, height: u32, pixels: &[u8]) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(pixels).ok()?;
        writer.finish().ok()?;
    }
    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{engine::general_purpose::STANDARD, Engine};
    use windows::core::{Interface, HSTRING};
    use windows::Win32::System::Com::{CoCreateInstance, IPersistFile, CLSCTX_INPROC_SERVER};
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

    #[test]
    fn extracts_custom_shortcut_icon_with_color_orientation_and_transparency() {
        let directory = tempfile::tempdir().unwrap();
        let icon_path = directory.path().join("自定义图标.ico");
        let link_path = directory.path().join("快捷方式.lnk");
        // 非对称图片可同时检测 BGRA 顺序、上下翻转和透明度处理。
        let mut pixels = vec![0; 256 * 256 * 4];
        for y in 0..256 {
            for x in 0..256 {
                let color = if y < 128 {
                    [220, 40, 20, 255]
                } else if x < 128 {
                    [20, 80, 220, 128]
                } else {
                    [0, 0, 0, 0]
                };
                pixels[(y * 256 + x) * 4..(y * 256 + x + 1) * 4].copy_from_slice(&color);
            }
        }
        let png = encode_png(256, 256, &pixels).unwrap();
        // ICO 的 0 宽/高字节表示 256，图像从 22 字节后开始。
        let mut ico = vec![0, 0, 1, 0, 1, 0, 0, 0, 0, 0, 1, 0, 32, 0];
        ico.extend_from_slice(&(png.len() as u32).to_le_bytes());
        ico.extend_from_slice(&22_u32.to_le_bytes());
        ico.extend_from_slice(&png);
        std::fs::write(&icon_path, ico).unwrap();
        {
            let _com = ComApartment::new().unwrap();
            // SAFETY: 在已初始化的线程创建 ShellLink；仅保存临时快捷方式，不启动应用。
            unsafe {
                let link: IShellLinkW =
                    CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).unwrap();
                link.SetPath(&HSTRING::from(std::env::current_exe().unwrap().as_os_str()))
                    .unwrap();
                link.SetIconLocation(&HSTRING::from(icon_path.as_os_str()), 0)
                    .unwrap();
                let file: IPersistFile = link.cast().unwrap();
                file.Save(&HSTRING::from(link_path.as_os_str()), true)
                    .unwrap();
            }
        }
        let app = crate::platform::AppInfo {
            name: "快捷方式".into(),
            path: link_path.to_string_lossy().into_owned(),
            icon_path: None,
        };
        let data = crate::platform::get_app_icon(&app).expect("Windows must extract a real icon");
        let png = STANDARD
            .decode(data.strip_prefix("data:image/png;base64,").unwrap())
            .unwrap();
        let mut reader = png::Decoder::new(std::io::Cursor::new(png))
            .read_info()
            .unwrap();
        let mut decoded = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut decoded).unwrap();
        assert_eq!(
            (info.width, info.height, info.color_type),
            (256, 256, png::ColorType::Rgba)
        );
        for (x, y, expected) in [
            (64, 64, [220_u8, 40, 20, 255]),
            (64, 192, [20, 80, 220, 128]),
            (192, 192, [0, 0, 0, 0]),
        ] {
            let offset = (y * 256 + x) * 4;
            for (actual, expected) in decoded[offset..offset + 4].iter().zip(expected) {
                assert!(
                    actual.abs_diff(expected) <= 2,
                    "pixel ({x}, {y}): {:?}",
                    &decoded[offset..offset + 4]
                );
            }
        }
    }

    #[test]
    fn missing_or_non_launcher_paths_have_no_icon() {
        let directory = tempfile::tempdir().unwrap();
        assert!(get_app_icon(&directory.path().join("missing.lnk")).is_none());
        let document = directory.path().join("readme.txt");
        std::fs::write(&document, "test").unwrap();
        assert!(get_app_icon(&document).is_none());
        assert!(get_app_icon(directory.path()).is_none());
    }
}
