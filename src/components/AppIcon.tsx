import { memo, useEffect, useState } from "react";
import { SquaresFourIcon } from "@phosphor-icons/react";
import { getAppIcon, getCachedIcon, type AppInfo } from "../lib/apps";

function AppIcon({
  app,
  enabled,
  priority,
}: {
  app: AppInfo;
  enabled: boolean;
  priority: number;
}) {
  // 同步命中已解码的内存缓存，页面重新显示时不会先闪一下占位图标。
  const [src, setSrc] = useState(() => getCachedIcon(app));
  useEffect(() => {
    if (!enabled) return;
    let current = true;
    getAppIcon(app, priority).then((icon) => {
      if (current) setSrc(icon);
    });
    return () => {
      current = false;
    };
  }, [app, enabled, priority]);
  return src ? (
    <img
      className="app-icon"
      src={src}
      alt=""
      draggable={false}
      decoding="async"
      onError={() => setSrc(null)}
    />
  ) : (
    <span className="app-icon fallback-icon" aria-hidden="true">
      <SquaresFourIcon weight="duotone" />
    </span>
  );
}
export default memo(AppIcon);
