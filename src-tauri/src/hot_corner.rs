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

const DEFAULT_TRIGGER_DELAY_MS: u64 = 80;
const MIN_TRIGGER_DELAY_MS: u64 = 50;
const MAX_TRIGGER_DELAY_MS: u64 = 1_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HotCornerConfig {
    pub corner: HotCorner,
    pub trigger_delay_ms: u64,
}

impl Default for HotCornerConfig {
    fn default() -> Self {
        Self {
            corner: HotCorner::Off,
            trigger_delay_ms: DEFAULT_TRIGGER_DELAY_MS,
        }
    }
}

#[derive(Default)]
pub struct HotCornerState(Mutex<HotCornerConfig>);

impl HotCornerState {
    pub fn get(&self) -> HotCornerConfig {
        self.0.lock().map(|config| *config).unwrap_or_default()
    }

    pub fn set(&self, corner: HotCorner, trigger_delay_ms: u64) -> Result<(), String> {
        *self.0.lock().map_err(|err| err.to_string())? = HotCornerConfig {
            corner,
            trigger_delay_ms: trigger_delay_ms.clamp(MIN_TRIGGER_DELAY_MS, MAX_TRIGGER_DELAY_MS),
        };
        Ok(())
    }

    pub fn enabled(&self) -> bool {
        self.get().corner != HotCorner::Off
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::{HotCorner, HotCornerConfig, HotCornerState};
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSEvent, NSScreen};
    use std::sync::{Arc, RwLock};
    use std::thread;
    use std::time::{Duration, Instant};
    use tauri::{AppHandle, Manager};

    const POLL_INTERVAL: Duration = Duration::from_millis(50);
    const SCREEN_REFRESH_INTERVAL: Duration = Duration::from_secs(2);
    const CORNER_MARGIN: f64 = 16.0;

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

    pub fn show(app: &AppHandle) {
        let Some(window) = app.get_webview_window("main") else {
            return;
        };
        let _ = app.show();
        let _ = window.show();
        let _ = window.maximize();

        if let Some(mtm) = MainThreadMarker::new() {
            let ns_app = NSApplication::sharedApplication(mtm);
            #[allow(deprecated)]
            ns_app.activateIgnoringOtherApps(true);
        }

        let _ = window.set_focus();
    }

    pub fn start(app: AppHandle) {
        // setup 在 macOS 主线程执行，NSScreen 的坐标和 NSEvent 的鼠标坐标处于同一坐标系。
        let initial_frames = screen_frames();
        if initial_frames.is_empty() {
            eprintln!("无法读取屏幕边界，触发角监听未启动");
            return;
        }
        let frames = Arc::new(RwLock::new(initial_frames));

        let _ = thread::Builder::new()
            .name("luma-hot-corner".into())
            .spawn(move || {
                let mut entered_at: Option<Instant> = None;
                let mut latched = false;
                let mut previous_config = HotCornerConfig::default();
                let mut refreshed_at = Instant::now();

                loop {
                    if refreshed_at.elapsed() >= SCREEN_REFRESH_INTERVAL {
                        refreshed_at = Instant::now();
                        let shared_frames = Arc::clone(&frames);
                        if let Err(err) = app.run_on_main_thread(move || {
                            let updated = screen_frames();
                            if !updated.is_empty() {
                                match shared_frames.write() {
                                    Ok(mut current) => *current = updated,
                                    Err(err) => eprintln!("更新屏幕边界失败: {err}"),
                                }
                            }
                        }) {
                            eprintln!("刷新屏幕边界失败: {err}");
                        }
                    }

                    let config = app.state::<HotCornerState>().get();
                    if config != previous_config {
                        previous_config = config;
                        entered_at = None;
                        latched = false;
                    }

                    if config.corner == HotCorner::Off {
                        thread::sleep(POLL_INTERVAL);
                        continue;
                    }

                    let location = NSEvent::mouseLocation();
                    let inside = frames
                        .read()
                        .map(|frames| {
                            frames.iter().any(|frame| {
                                frame.contains_corner(location.x, location.y, config.corner)
                            })
                        })
                        .unwrap_or(false);

                    if !inside {
                        entered_at = None;
                        latched = false;
                    } else if !latched {
                        let entered = entered_at.get_or_insert_with(Instant::now);
                        if entered.elapsed() >= Duration::from_millis(config.trigger_delay_ms) {
                            latched = true;
                            let handle = app.clone();
                            if let Err(err) = app.run_on_main_thread(move || show(&handle)) {
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
            assert!(!frame().contains_corner(-1420.0, 880.0, HotCorner::TopLeft));
            assert!(!frame().contains_corner(-1440.0, 900.0, HotCorner::Off));
        }
    }
}

#[cfg(target_os = "macos")]
pub use macos::start;

#[cfg(target_os = "macos")]
pub use macos::show;

#[cfg(not(target_os = "macos"))]
pub fn start(_app: tauri::AppHandle) {}

#[cfg(test)]
mod state_tests {
    use super::*;

    #[test]
    fn clamps_configured_delay_to_supported_range() {
        let state = HotCornerState::default();
        state.set(HotCorner::TopLeft, 1).unwrap();
        assert_eq!(state.get().trigger_delay_ms, MIN_TRIGGER_DELAY_MS);

        state.set(HotCorner::BottomRight, 10_000).unwrap();
        assert_eq!(state.get().trigger_delay_ms, MAX_TRIGGER_DELAY_MS);
    }
}
