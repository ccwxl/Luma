import { invoke, isTauri } from "@tauri-apps/api/core";

export interface AppInfo {
  name: string;
  path: string;
  icon_path: string | null;
  icon_data?: string | null;
}

export const desktop = isTauri();

export async function getInstalledApps(): Promise<AppInfo[]> {
  if (desktop) return invoke<AppInfo[]>("get_installed_apps");
  if (import.meta.env.DEV) {
    const response = await fetch("/.preview/apps.json");
    if (
      response.ok &&
      response.headers.get("content-type")?.includes("application/json")
    )
      return response.json();
  }
  throw new Error("请在 Luma 桌面版中查看已安装的应用。");
}

export async function launchApp(app: AppInfo) {
  if (desktop) await invoke("launch_app", { appPath: app.path });
}

export async function closeApp() {
  if (desktop) await invoke("close_app");
}

type IconRecord = {
  src?: string | null;
  image?: HTMLImageElement;
  promise: Promise<string | null>;
  resolve: (src: string | null) => void;
  priority: number;
};
const icons = new Map<string, IconRecord>();
const waiting = new Set<string>();
let flushing = false;

export function getCachedIcon(app: AppInfo) {
  return app.icon_data ?? icons.get(app.path)?.src;
}

async function finishIcon(path: string, src: string | null) {
  const record = icons.get(path);
  if (!record) return;
  if (src) {
    // 提前解码并保留图片，滑到下一页时不再阻塞主线程解码。
    const image = new Image();
    image.src = src;
    try {
      await image.decode();
      record.image = image;
    } catch {
      src = null;
    }
  }
  record.src = src;
  record.resolve(src);
}

async function flushIcons() {
  if (flushing) return;
  flushing = true;
  try {
    while (waiting.size) {
      // 当前页优先于相邻页，每批至多 6 张，减少 IPC 并限制原生解码并发。
      const batch = [...waiting]
        .sort((a, b) => icons.get(a)!.priority - icons.get(b)!.priority)
        .slice(0, 6);
      batch.forEach((path) => waiting.delete(path));
      try {
        const result = await invoke<Record<string, string | null>>(
          "get_app_icons",
          { appPaths: batch },
        );
        await Promise.all(
          batch.map((path) => finishIcon(path, result[path] ?? null)),
        );
      } catch {
        await Promise.all(batch.map((path) => finishIcon(path, null)));
      }
    }
  } finally {
    flushing = false;
  }
}

export function getAppIcon(app: AppInfo, priority = 0): Promise<string | null> {
  const cached = icons.get(app.path);
  if (cached) {
    cached.priority = Math.min(priority, cached.priority);
    return cached.promise;
  }
  let resolve!: IconRecord["resolve"];
  const promise = new Promise<string | null>((done) => {
    resolve = done;
  });
  icons.set(app.path, { promise, resolve, priority });
  if (app.icon_data !== undefined || !desktop)
    void finishIcon(app.path, app.icon_data ?? null);
  else {
    waiting.add(app.path);
    queueMicrotask(() => void flushIcons());
  }
  return promise;
}

export function preloadAppIcons(apps: AppInfo[], priority = 1) {
  apps.forEach((app) => {
    void getAppIcon(app, priority);
  });
}

export async function clearIconCache() {
  await Promise.all([...icons.values()].map((record) => record.promise));
  if (desktop) await invoke("clear_icon_cache");
  icons.clear();
}
