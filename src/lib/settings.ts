export type HotCorner =
  | "off"
  | "top-left"
  | "top-right"
  | "bottom-left"
  | "bottom-right";

export interface Settings {
  reducedMotion: boolean;
  wallpaperBlur: number;
  hotCorner: HotCorner;
}

const hotCorners = new Set<HotCorner>([
  "off",
  "top-left",
  "top-right",
  "bottom-left",
  "bottom-right",
]);

export function readSettings(): Settings {
  try {
    const saved = JSON.parse(
      localStorage.getItem("luma.settings.v1") ?? "null",
    );
    if (
      saved &&
      typeof saved.reducedMotion === "boolean" &&
      Number.isFinite(saved.wallpaperBlur)
    ) {
      return {
        reducedMotion: saved.reducedMotion,
        wallpaperBlur: Math.min(24, Math.max(0, saved.wallpaperBlur)),
        hotCorner: hotCorners.has(saved.hotCorner) ? saved.hotCorner : "off",
      };
    }
  } catch {
    /* 无持久化权限时使用默认值。 */
  }
  return { reducedMotion: false, wallpaperBlur: 8, hotCorner: "off" };
}
export function saveSettings(settings: Settings) {
  try {
    localStorage.setItem("luma.settings.v1", JSON.stringify(settings));
  } catch {
    /* 保留当前会话设置。 */
  }
}
