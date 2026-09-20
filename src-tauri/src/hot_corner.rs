use serde::Deserialize;
use std::sync::Mutex;

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum HotCorner {
    #[default]
    Off,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Default)]
pub struct HotCornerState(Mutex<HotCorner>);

impl HotCornerState {
    pub fn get(&self) -> HotCorner {
        self.0.lock().map(|corner| *corner).unwrap_or_default()
    }

    pub fn set(&self, corner: HotCorner) -> Result<(), String> {
        *self.0.lock().map_err(|err| err.to_string())? = corner;
        Ok(())
    }

    pub fn enabled(&self) -> bool {
        self.get() != HotCorner::Off
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::{HotCorner, HotCornerState};
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSEvent, NSScreen};
    use std::thread;
    use std::time::{Duration, Instant};
    use tauri::{AppHandle, Manager};

    const POLL_INTERVAL: Duration = Duration::from_millis(50);
    const TRIGGER_DELAY: Duration = Duration::from_millis(150);
    const CORNER_MARGIN: f64 = 6.0;

    #[derive(Clone, Copy, Debug)]
    struct ScreenFrame {
        min_x: f64,
        min_y: f64,
        max_x: f64,
        max_y: f64,
    }

    impl ScreenFrame {
        fn contains_corner(self, x: f64, y: f64, corner: HotCorner) -> bool {
            let near_left = (x - self.min_x).abs() <= CORNER_MARGIN;
            let near_right = (x - self.max_x).abs() <= CORNER_MARGIN;
            let near_bottom = (y - self.min_y).abs() <= CORNER_MARGIN;
            let near_top = (y - self.max_y).abs() <= CORNER_MARGIN;

            match corner {
                HotCorner::TopLeft => near_left && near_top,
                HotCorner::TopRight => near_right && near_top,
                HotCorner::BottomLeft => near_left && near_bottom,
                HotCorner::BottomRight => near_right && near_bottom,
                HotCorner::Off => false,
            }
        }
    }

    fn screen_frames() -> Vec<ScreenFrame> {
        let Some(mtm) = MainThreadMarker::new() else {
            return Vec::new();
        };
        let screens = NSScreen::screens(mtm);
        screens
            .iter()
            .map(|screen| {
                let frame = screen.frame();
                ScreenFrame {
                    min_x: frame.origin.x,
                    min_y: frame.origin.y,
                    max_x: frame.origin.x + frame.size.width,
                    max_y: frame.origin.y + frame.size.height,
                }
            })
            .collect()
    }

    fn show_luma(app: &AppHandle) {
        let Some(window) = app.get_webview_window("main") else {
            return;
        };
        let _ = window.show();
        let _ = window.maximize();
        let _ = window.set_focus();
    }

    pub fn start(app: AppHandle) {
        // setup 在 macOS 主线程执行，NSScreen 的坐标和 NSEvent 的鼠标坐标处于同一坐标系。
        let frames = screen_frames();
        if frames.is_empty() {
            eprintln!("无法读取屏幕边界，触发角监听未启动");
            return;
        }

        let _ = thread::Builder::new()
            .name("luma-hot-corner".into())
            .spawn(move || {
                let mut entered_at: Option<Instant> = None;
                let mut latched = false;
                let mut previous_corner = HotCorner::Off;

                loop {
                    let corner = app.state::<HotCornerState>().get();
                    if corner != previous_corner {
                        previous_corner = corner;
                        entered_at = None;
                        latched = false;
                    }

                    if corner == HotCorner::Off {
                        thread::sleep(POLL_INTERVAL);
                        continue;
                    }

                    let location = NSEvent::mouseLocation();
                    let inside = frames
                        .iter()
                        .any(|frame| frame.contains_corner(location.x, location.y, corner));

                    if !inside {
                        entered_at = None;
                        latched = false;
                    } else if !latched {
                        let entered = entered_at.get_or_insert_with(Instant::now);
                        if entered.elapsed() >= TRIGGER_DELAY {
                            latched = true;
                            let handle = app.clone();
                            if let Err(err) = app.run_on_main_thread(move || show_luma(&handle)) {
                                eprintln!("触发角显示 Luma 失败: {err}");
                            }
                        }
                    }

                    thread::sleep(POLL_INTERVAL);
                }
            });
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn frame() -> ScreenFrame {
            ScreenFrame {
                min_x: -1440.0,
                min_y: 0.0,
                max_x: 0.0,
                max_y: 900.0,
            }
        }

        #[test]
        fn detects_each_corner_in_appkit_coordinates() {
            assert!(frame().contains_corner(-1440.0, 900.0, HotCorner::TopLeft));
            assert!(frame().contains_corner(0.0, 900.0, HotCorner::TopRight));
            assert!(frame().contains_corner(-1440.0, 0.0, HotCorner::BottomLeft));
            assert!(frame().contains_corner(0.0, 0.0, HotCorner::BottomRight));
        }

        #[test]
        fn rejects_points_outside_the_corner_margin() {
            assert!(!frame().contains_corner(-1430.0, 890.0, HotCorner::TopLeft));
            assert!(!frame().contains_corner(-1440.0, 900.0, HotCorner::Off));
        }
    }
}

#[cfg(target_os = "macos")]
pub use macos::start;

#[cfg(not(target_os = "macos"))]
pub fn start(_app: tauri::AppHandle) {}
