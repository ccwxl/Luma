import { useEffect, useState } from "react";
import { SquaresFourIcon } from "@phosphor-icons/react";
import { getAppIcon, type AppInfo } from "../lib/apps";

export default function AppIcon({ app }: { app: AppInfo }) {
  const [src, setSrc] = useState<string | null>(app.icon_data ?? null);

  useEffect(() => {
    let current = true;
    getAppIcon(app).then((icon) => {
      if (current) setSrc(icon);
    });
    return () => {
      current = false;
    };
  }, [app]);

  return src ? (
    <img
      className="app-icon"
      src={src}
      alt=""
      draggable={false}
      onError={() => setSrc(null)}
    />
  ) : (
    <span className="app-icon fallback-icon" aria-hidden="true">
      <SquaresFourIcon weight="duotone" />
    </span>
  );
}
