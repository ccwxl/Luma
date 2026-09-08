import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
} from "react";
import {
  ArrowClockwiseIcon,
  CaretLeftIcon,
  CaretRightIcon,
  GearSixIcon,
  MagnifyingGlassIcon,
  SquaresFourIcon,
  XIcon,
} from "@phosphor-icons/react";
import AppIcon from "./components/AppIcon";
import SettingsDialog from "./components/SettingsDialog";
import {
  clearIconCache,
  closeApp,
  desktop,
  getInstalledApps,
  launchApp,
  preloadAppIcons,
  type AppInfo,
} from "./lib/apps";
import { readSettings, saveSettings } from "./lib/settings";
import "./App.css";

function App() {
  const [apps, setApps] = useState<AppInfo[]>([]);
  const [search, setSearch] = useState("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [page, setPage] = useState(0);
  const [layout, setLayout] = useState({ columns: 7, rows: 4 });
  const [launching, setLaunching] = useState<string | null>(null);
  const [notice, setNotice] = useState<{
    text: string;
    error?: boolean;
  } | null>(null);
  const [settings, setSettings] = useState(readSettings);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [closed, setClosed] = useState(false);
  const [dragOffset, setDragOffset] = useState(0);
  const [dragging, setDragging] = useState(false);
  const [iconVersion, setIconVersion] = useState(0);
  const searchRef = useRef<HTMLInputElement>(null);
  const gridRef = useRef<HTMLDivElement>(null);
  const firstAppRef = useRef<HTMLButtonElement>(null);
  const wheel = useRef({ x: 0, y: 0, last: 0, lockedUntil: 0 });
  const wheelTimer = useRef<ReturnType<typeof setTimeout> | undefined>(
    undefined,
  );
  const touch = useRef<{ start: number; offset: number } | null>(null);
  const suppressClickUntil = useRef(0);
  const launchPending = useRef(false);
  const requestId = useRef(0);

  const loadApps = useCallback(async () => {
    const id = ++requestId.current;
    setLoading(true);
    setError("");
    try {
      const list = await getInstalledApps();
      if (id !== requestId.current) return;
      setApps(list);
      setPage(0);
    } catch (reason) {
      if (id === requestId.current)
        setError(String(reason instanceof Error ? reason.message : reason));
    } finally {
      if (id === requestId.current) setLoading(false);
    }
  }, []);
  useEffect(() => {
    void loadApps();
  }, [loadApps]);
  useEffect(() => {
    saveSettings(settings);
  }, [settings]);
  useEffect(() => {
    const element = gridRef.current;
    if (!element) return;
    const observer = new ResizeObserver(([entry]) => {
      const { width, height } = entry.contentRect;
      const columns = Math.max(3, Math.min(10, Math.floor(width / 150)));
      const rows = Math.max(
        1,
        Math.min(5, Math.floor(height / (width < 600 ? 108 : 128))),
      );
      setLayout((previous) =>
        previous.columns === columns && previous.rows === rows
          ? previous
          : { columns, rows },
      );
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, [closed]);
  useEffect(() => {
    if (!notice) return;
    const timer = setTimeout(() => setNotice(null), notice.error ? 5000 : 2600);
    return () => clearTimeout(timer);
  }, [notice]);
  useEffect(() => () => clearTimeout(wheelTimer.current), []);

  const filtered = useMemo(
    () =>
      apps.filter((app) =>
        app.name
          .toLocaleLowerCase()
          .includes(search.trim().toLocaleLowerCase()),
      ),
    [apps, search],
  );
  const pageSize = layout.columns * layout.rows;
  const pageCount = Math.max(1, Math.ceil(filtered.length / pageSize));
  const currentPage = Math.min(page, pageCount - 1);
  const pageIndices = Array.from(
    { length: Math.min(pageCount, 7) },
    (_, index) => Math.max(0, Math.min(currentPage - 3, pageCount - 7)) + index,
  );
  const pages = useMemo(
    () =>
      Array.from({ length: pageCount }, (_, index) =>
        filtered.slice(index * pageSize, (index + 1) * pageSize),
      ),
    [filtered, pageSize, pageCount],
  );

  useEffect(() => {
    if (loading || settingsOpen || closed) return;
    preloadAppIcons(pages[currentPage] ?? [], 0);
    preloadAppIcons(
      [...(pages[currentPage + 1] ?? []), ...(pages[currentPage - 1] ?? [])],
      1,
    );
  }, [pages, currentPage, loading, settingsOpen, closed, iconVersion]);

  const changePage = useCallback(
    (next: number) => {
      setPage(Math.max(0, Math.min(pageCount - 1, next)));
      setDragOffset(0);
      setDragging(false);
      // 焦点不能留在已滑出视野的页面里。
      if (document.activeElement?.closest(".app-page"))
        searchRef.current?.focus({ preventScroll: true });
    },
    [pageCount],
  );

  const handleLaunch = useCallback(async (app: AppInfo) => {
    if (launchPending.current || Date.now() < suppressClickUntil.current)
      return;
    if (!desktop) {
      setNotice({ text: "浏览器预览 · 请在桌面版 Luma 中打开应用" });
      return;
    }
    launchPending.current = true;
    setLaunching(app.path);
    try {
      await launchApp(app);
      setNotice({ text: `已打开 ${app.name}` });
    } catch (reason) {
      setNotice({
        text: `无法打开 ${app.name}：${String(reason)}`,
        error: true,
      });
    } finally {
      launchPending.current = false;
      setLaunching(null);
    }
  }, []);

  const handleClose = useCallback(async () => {
    if (!desktop) {
      setSettingsOpen(false);
      setClosed(true);
      return;
    }
    try {
      await closeApp();
    } catch (reason) {
      setNotice({ text: `无法关闭应用：${String(reason)}`, error: true });
    }
  }, []);

  useEffect(() => {
    const onShortcut = (event: globalThis.KeyboardEvent) => {
      if (event.isComposing || event.keyCode === 229) return;
      const modifier = event.metaKey || event.ctrlKey;
      if (event.key === "Escape") {
        event.preventDefault();
        event.stopPropagation();
        void handleClose();
        return;
      }
      if (modifier && event.key === ",") {
        event.preventDefault();
        setSettingsOpen(true);
        return;
      }
      if (settingsOpen || closed) return;
      if (modifier && event.key.toLowerCase() === "f") {
        event.preventDefault();
        searchRef.current?.focus();
        searchRef.current?.select();
        return;
      }
      if (
        !modifier &&
        !event.altKey &&
        (event.key === "ArrowLeft" || event.key === "ArrowRight")
      ) {
        event.preventDefault();
        changePage(currentPage + (event.key === "ArrowRight" ? 1 : -1));
        return;
      }
      if (event.key === "Enter" && search.trim()) {
        event.preventDefault();
        if (!event.repeat && !loading && filtered[0])
          void handleLaunch(filtered[0]);
        return;
      }
      const editing =
        event.target instanceof HTMLInputElement ||
        event.target instanceof HTMLTextAreaElement;
      if (!editing && !modifier && !event.altKey && event.key.length === 1) {
        event.preventDefault();
        setSearch((value) => value + event.key);
        setPage(0);
        searchRef.current?.focus();
      }
    };
    window.addEventListener("keydown", onShortcut, true);
    return () => window.removeEventListener("keydown", onShortcut, true);
  }, [
    currentPage,
    changePage,
    filtered,
    search,
    settingsOpen,
    closed,
    loading,
    handleClose,
    handleLaunch,
  ]);

  function finishSwipe(offset: number) {
    const threshold = Math.min(
      90,
      (gridRef.current?.clientWidth ?? 800) * 0.12,
    );
    const next =
      Math.abs(offset) >= threshold
        ? currentPage - Math.sign(offset)
        : currentPage;
    changePage(next);
  }

  if (closed)
    return (
      <main className="launchpad preview-closed">
        <div className="wallpaper" />
        <button className="text-button" onClick={() => setClosed(false)}>
          重新打开预览
        </button>
      </main>
    );

  return (
    <main
      className={`launchpad${settings.reducedMotion ? " reduced-motion" : ""}`}
      aria-label="应用启动器"
      style={
        { "--wallpaper-blur": `${settings.wallpaperBlur}px` } as CSSProperties
      }
    >
      <div className="wallpaper" aria-hidden="true" />
      <header className="launcher-header">
        <form
          className="search-field"
          role="search"
          onSubmit={(event) => {
            event.preventDefault();
            if (filtered[0] && !loading) void handleLaunch(filtered[0]);
          }}
        >
          <MagnifyingGlassIcon size={18} aria-hidden="true" />
          <input
            ref={searchRef}
            value={search}
            onChange={(event) => {
              setSearch(event.target.value);
              setPage(0);
              setDragOffset(0);
            }}
            onKeyDown={(event) => {
              if (event.key === "ArrowDown" && filtered.length) {
                event.preventDefault();
                firstAppRef.current?.focus({ preventScroll: true });
              }
            }}
            aria-label="搜索应用"
            placeholder="搜索"
            autoComplete="off"
            spellCheck={false}
          />
          {search && (
            <button
              className="clear-search"
              type="button"
              aria-label="清除搜索"
              onClick={() => {
                setSearch("");
                setPage(0);
                searchRef.current?.focus();
              }}
            >
              <XIcon size={14} weight="bold" />
            </button>
          )}
        </form>
        <div className="header-actions">
          <button
            className="header-button"
            aria-label="刷新应用"
            title="刷新应用"
            disabled={loading}
            onClick={() => void loadApps()}
          >
            <ArrowClockwiseIcon
              size={19}
              className={loading ? "spinning" : undefined}
            />
          </button>
          <button
            className="header-button"
            aria-label="打开设置"
            title="设置 · ⌘,"
            onClick={() => setSettingsOpen(true)}
          >
            <GearSixIcon size={19} />
          </button>
        </div>
      </header>
      <div
        ref={gridRef}
        className="apps-viewport"
        aria-busy={loading}
        onWheel={(event) => {
          if (event.ctrlKey || loading || pageCount < 2 || settingsOpen) return;
          const now = Date.now();
          if (now < wheel.current.lockedUntil) return;
          if (now - wheel.current.last > 180) {
            wheel.current.x = 0;
            wheel.current.y = 0;
          }
          wheel.current.last = now;
          if (Math.abs(event.deltaX) > Math.abs(event.deltaY)) {
            wheel.current.x += event.deltaX;
            const max = (gridRef.current?.clientWidth ?? 800) * 0.85;
            let offset = Math.max(-max, Math.min(max, -wheel.current.x));
            if (
              (currentPage === 0 && offset > 0) ||
              (currentPage === pageCount - 1 && offset < 0)
            )
              offset *= 0.18;
            setDragging(true);
            setDragOffset(offset);
            clearTimeout(wheelTimer.current);
            wheelTimer.current = setTimeout(() => {
              finishSwipe(offset);
              wheel.current = {
                x: 0,
                y: 0,
                last: 0,
                lockedUntil: Date.now() + 180,
              };
            }, 100);
          } else {
            wheel.current.y += event.deltaY;
            if (Math.abs(wheel.current.y) > 70) {
              changePage(currentPage + Math.sign(wheel.current.y));
              wheel.current = { x: 0, y: 0, last: now, lockedUntil: now + 440 };
            }
          }
        }}
        onPointerDown={(event) => {
          if (event.pointerType === "touch")
            touch.current = { start: event.clientX, offset: 0 };
        }}
        onPointerMove={(event) => {
          if (!touch.current) return;
          let offset = event.clientX - touch.current.start;
          touch.current.offset = offset;
          if (Math.abs(offset) > 8) {
            if (
              (currentPage === 0 && offset > 0) ||
              (currentPage === pageCount - 1 && offset < 0)
            )
              offset *= 0.18;
            setDragging(true);
            setDragOffset(offset);
          }
        }}
        onPointerUp={() => {
          if (touch.current) {
            if (Math.abs(touch.current.offset) > 8)
              suppressClickUntil.current = Date.now() + 300;
            finishSwipe(touch.current.offset);
            touch.current = null;
          }
        }}
        onPointerCancel={() => {
          touch.current = null;
          setDragging(false);
          setDragOffset(0);
        }}
      >
        {loading ? (
          <div
            className="app-grid loading-grid"
            style={
              {
                "--columns": layout.columns,
                "--rows": layout.rows,
              } as CSSProperties
            }
            aria-label="正在读取应用"
          >
            {Array.from({ length: pageSize }, (_, index) => (
              <div className="loading-app" key={index}>
                <SquaresFourIcon weight="duotone" />
                <span />
              </div>
            ))}
          </div>
        ) : error ? (
          <div className="empty-state" role="alert">
            <SquaresFourIcon size={42} weight="light" />
            <h1>暂时无法读取应用</h1>
            <p>{error}</p>
            <button className="text-button" onClick={() => void loadApps()}>
              重新加载
            </button>
          </div>
        ) : filtered.length === 0 ? (
          <div className="empty-state" role="status">
            <MagnifyingGlassIcon size={42} weight="light" />
            <h1>{search ? "没有找到应用" : "这里还没有应用"}</h1>
            <p>
              {search
                ? `没有与“${search}”匹配的应用，试试其他名称。`
                : "安装应用后，点击右上角刷新。"}
            </p>
            {search && (
              <button
                className="text-button"
                onClick={() => {
                  setSearch("");
                  searchRef.current?.focus();
                }}
              >
                清除搜索
              </button>
            )}
          </div>
        ) : (
          <div
            key={`${search}-${pageSize}`}
            className={`page-track${dragging ? " is-dragging" : ""}`}
            style={{
              transform: `translate3d(calc(${-currentPage * 100}% + ${dragOffset}px), 0, 0)`,
            }}
          >
            {pages.map((items, pageIndex) => (
              <section
                className="app-page"
                key={pageIndex}
                aria-hidden={pageIndex !== currentPage}
                inert={pageIndex !== currentPage}
              >
                <ul
                  className="app-grid"
                  aria-label={`第 ${pageIndex + 1} 页应用`}
                  style={
                    {
                      "--columns": layout.columns,
                      "--rows": layout.rows,
                    } as CSSProperties
                  }
                >
                  {items.map((app, index) => (
                    <li className="app-cell" key={app.path}>
                      <button
                        className={`app-button${launching === app.path ? " launching" : ""}`}
                        ref={
                          pageIndex === currentPage && index === 0
                            ? firstAppRef
                            : undefined
                        }
                        onClick={() => void handleLaunch(app)}
                        disabled={launching === app.path}
                        title={app.name}
                      >
                        <AppIcon
                          key={`${app.path}-${iconVersion}`}
                          app={app}
                          enabled={Math.abs(pageIndex - currentPage) <= 1}
                          priority={pageIndex === currentPage ? 0 : 1}
                        />
                        <span className="app-name">{app.name}</span>
                      </button>
                    </li>
                  ))}
                </ul>
              </section>
            ))}
          </div>
        )}
      </div>
      <nav className="pagination" aria-label="应用分页">
        {!loading && !error && pageCount > 1 && (
          <>
            <button
              className="page-arrow"
              aria-label="上一页"
              disabled={currentPage === 0}
              onClick={() => changePage(currentPage - 1)}
            >
              <CaretLeftIcon size={16} />
            </button>
            {pageIndices.map((index) => (
              <button
                key={index}
                className={`page-dot${index === currentPage ? " active" : ""}`}
                aria-label={`第 ${index + 1} 页`}
                aria-current={index === currentPage ? "page" : undefined}
                onClick={() => changePage(index)}
              >
                <span />
              </button>
            ))}
            <button
              className="page-arrow"
              aria-label="下一页"
              disabled={currentPage === pageCount - 1}
              onClick={() => changePage(currentPage + 1)}
            >
              <CaretRightIcon size={16} />
            </button>
          </>
        )}
        <span className="sr-only" aria-live="polite">
          {!loading &&
            `${filtered.length} 个应用，第 ${currentPage + 1} 页，共 ${pageCount} 页`}
        </span>
      </nav>
      <SettingsDialog
        open={settingsOpen}
        settings={settings}
        onChange={setSettings}
        onClose={() => setSettingsOpen(false)}
        onExit={() => void handleClose()}
        onClearCache={async () => {
          try {
            await clearIconCache();
            setIconVersion((version) => version + 1);
            setNotice({ text: "图标缓存已清理" });
          } catch (reason) {
            setNotice({ text: `清理失败：${String(reason)}`, error: true });
          }
        }}
      />
      {notice && (
        <div
          className={`notice${notice.error ? " notice-error" : ""}`}
          role={notice.error ? "alert" : "status"}
        >
          {notice.text}
          <button aria-label="关闭提示" onClick={() => setNotice(null)}>
            <XIcon size={14} />
          </button>
        </div>
      )}
    </main>
  );
}
export default App;
