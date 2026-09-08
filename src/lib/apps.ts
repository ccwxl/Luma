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
    ) {
      return response.json();
    }
  }
  throw new Error("请在 Luma 桌面版中查看已安装的应用。");
}

export async function launchApp(app: AppInfo) {
  if (desktop) await invoke("launch_app", { appPath: app.path });
}

// 按需读取当前页和 Dock 的图标，同时限制原生图标解码的并发量。
const icons = new Map<string, Promise<string | null>>();
const waiting: (() => void)[] = [];
let active = 0;

export function getAppIcon(app: AppInfo): Promise<string | null> {
  if (app.icon_data !== undefined) return Promise.resolve(app.icon_data);
  if (!desktop) return Promise.resolve(null);
  const cached = icons.get(app.path);
  if (cached) return cached;
  const result = new Promise<string | null>((resolve) => {
    const run = () => {
      active += 1;
      invoke<string | null>("get_app_icon", { appPath: app.path })
        .then(resolve, () => resolve(null))
        .finally(() => {
          active -= 1;
          waiting.shift()?.();
        });
    };
    if (active < 4) run();
    else waiting.push(run);
  });
  icons.set(app.path, result);
  return result;
}
