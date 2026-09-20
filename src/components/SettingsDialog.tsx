import { useEffect, useRef, useState } from "react";
import { ArrowClockwiseIcon, XIcon } from "@phosphor-icons/react";
import type { HotCorner, Settings } from "../lib/settings";

export default function SettingsDialog({
  open,
  settings,
  showHotCorner,
  onChange,
  onClose,
  onClearCache,
  onExit,
}: {
  open: boolean;
  settings: Settings;
  showHotCorner: boolean;
  onChange: (settings: Settings) => void;
  onClose: () => void;
  onClearCache: () => Promise<void>;
  onExit: () => void;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  const [clearing, setClearing] = useState(false);
  useEffect(() => {
    const dialog = ref.current;
    if (!dialog) return;
    if (open && !dialog.open) dialog.showModal();
    else if (!open && dialog.open) dialog.close();
  }, [open]);
  return (
    <dialog
      ref={ref}
      className="settings-dialog"
      aria-labelledby="settings-title"
      onClose={onClose}
      onCancel={(event) => {
        event.preventDefault();
        onExit();
      }}
    >
      <div className="settings-header">
        <h1 id="settings-title">设置</h1>
        <button aria-label="关闭设置" onClick={onClose}>
          <XIcon size={20} />
        </button>
      </div>
      <section className="settings-section" aria-label="外观">
        <h2>外观</h2>
        <label className="setting-row">
          <span>
            减少动态效果<small>关闭页面滑动和图标动画</small>
          </span>
          <input
            type="checkbox"
            checked={settings.reducedMotion}
            onChange={(event) =>
              onChange({ ...settings, reducedMotion: event.target.checked })
            }
          />
        </label>
        <label className="setting-row">
          <span>
            背景模糊<small>调整背景的清晰程度</small>
          </span>
          <input
            aria-label="背景模糊"
            type="range"
            min={0}
            max={24}
            step={1}
            value={settings.wallpaperBlur}
            onChange={(event) =>
              onChange({
                ...settings,
                wallpaperBlur: Number(event.target.value),
              })
            }
          />
        </label>
      </section>
      {showHotCorner && (
        <section className="settings-section" aria-label="触发方式">
          <h2>触发方式</h2>
          <label className="setting-row">
            <span>
              屏幕触发角
              <small>鼠标在所选角落停留约 0.15 秒即可显示 Luma</small>
            </span>
            <select
              aria-label="屏幕触发角"
              value={settings.hotCorner}
              onChange={(event) =>
                onChange({
                  ...settings,
                  hotCorner: event.target.value as HotCorner,
                })
              }
            >
              <option value="off">关闭</option>
              <option value="top-left">左上角</option>
              <option value="top-right">右上角</option>
              <option value="bottom-left">左下角</option>
              <option value="bottom-right">右下角</option>
            </select>
          </label>
          {settings.hotCorner !== "off" && (
            <p className="setting-note">
              启用后，关闭 Luma 将改为隐藏到后台；在菜单栏中选择退出可完全结束进程。请避免与系统触发角使用同一位置。
            </p>
          )}
        </section>
      )}
      <section className="settings-section" aria-label="图标缓存">
        <h2>图标缓存</h2>
        <div className="setting-row">
          <span>
            本地图标缓存<small>应用更新后会自动刷新图标</small>
          </span>
          <button
            className="settings-action"
            disabled={clearing}
            onClick={async () => {
              setClearing(true);
              try {
                await onClearCache();
              } finally {
                setClearing(false);
              }
            }}
          >
            <ArrowClockwiseIcon
              size={15}
              className={clearing ? "spinning" : undefined}
            />
            {clearing ? "清理中" : "清理缓存"}
          </button>
        </div>
      </section>
      <section className="settings-section shortcuts" aria-label="快捷键">
        <h2>快捷键</h2>
        <dl>
          <div>
            <dt>上一页 / 下一页</dt>
            <dd>
              <kbd>←</kbd>
              <kbd>→</kbd>
            </dd>
          </div>
          <div>
            <dt>打开第一个搜索结果</dt>
            <dd>
              <kbd>Enter</kbd>
            </dd>
          </div>
          <div>
            <dt>打开设置</dt>
            <dd>
              <kbd>⌘</kbd>
              <kbd>,</kbd>
            </dd>
          </div>
          <div>
            <dt>{settings.hotCorner === "off" ? "关闭应用" : "隐藏应用"}</dt>
            <dd>
              <kbd>Esc</kbd>
            </dd>
          </div>
        </dl>
      </section>
    </dialog>
  );
}
