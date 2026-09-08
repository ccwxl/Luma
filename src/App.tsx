import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent,
} from "react";
import {
  ArrowClockwiseIcon,
  CaretLeftIcon,
  CaretRightIcon,
  MagnifyingGlassIcon,
  SquaresFourIcon,
  XIcon,
} from "@phosphor-icons/react";
import AppIcon from "./components/AppIcon";
import { desktop, getInstalledApps, launchApp, type AppInfo } from "./lib/apps";
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
  const searchRef = useRef<HTMLInputElement>(null);
  const gridRef = useRef<HTMLDivElement>(null);
  const buttonRefs = useRef<(HTMLButtonElement | null)[]>([]);
  const wheel = useRef({ delta: 0, time: 0 });
  const touchStart = useRef<number | null>(null);
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
  }, []);

  useEffect(() => {
    if (!notice) return;
    const timeout = window.setTimeout(
      () => setNotice(null),
      notice.error ? 5000 : 2600,
    );
    return () => window.clearTimeout(timeout);
  }, [notice]);

  useEffect(() => {
    const onShortcut = (event: globalThis.KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "f") {
        event.preventDefault();
        searchRef.current?.focus();
        searchRef.current?.select();
      }
      if (event.key === "Escape") {
        setSearch("");
        setPage(0);
        setNotice(null);
        searchRef.current?.focus();
      }
    };
    window.addEventListener("keydown", onShortcut);
    return () => window.removeEventListener("keydown", onShortcut);
  }, []);

  const filtered = useMemo(() => {
    const query = search.trim().toLocaleLowerCase();
    return apps.filter((app) => app.name.toLocaleLowerCase().includes(query));
  }, [apps, search]);
  const pageSize = layout.columns * layout.rows;
  const pageCount = Math.max(1, Math.ceil(filtered.length / pageSize));
  const currentPage = Math.min(page, pageCount - 1);
  const firstPageDot = Math.max(0, Math.min(currentPage - 3, pageCount - 7));
  const pageIndices = Array.from(
    { length: Math.min(pageCount, 7) },
    (_, index) => firstPageDot + index,
  );
  const visibleApps = filtered.slice(
    currentPage * pageSize,
    (currentPage + 1) * pageSize,
  );

  function changePage(next: number) {
    setPage(Math.max(0, Math.min(pageCount - 1, next)));
  }

  async function handleLaunch(app: AppInfo) {
    if (launchPending.current) return;
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
  }

  function onGridKeyDown(
    event: KeyboardEvent<HTMLButtonElement>,
    index: number,
  ) {
    const offsets: Record<string, number> = {
      ArrowRight: 1,
      ArrowLeft: -1,
      ArrowDown: layout.columns,
      ArrowUp: -layout.columns,
    };
    let next = currentPage * pageSize + index;
    if (event.key in offsets) next += offsets[event.key];
    else if (event.key === "Home") next = 0;
    else if (event.key === "End") next = filtered.length - 1;
    else if (event.key === "PageDown") next += pageSize;
    else if (event.key === "PageUp") next -= pageSize;
    else return;
    event.preventDefault();
    next = Math.max(0, Math.min(filtered.length - 1, next));
    setPage(Math.floor(next / pageSize));
    requestAnimationFrame(() => buttonRefs.current[next % pageSize]?.focus());
  }

  return (
    <main className="launchpad" aria-label="Luma 应用启动器">
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
            }}
            onKeyDown={(event) => {
              if (event.key === "ArrowDown" && visibleApps.length) {
                event.preventDefault();
                buttonRefs.current[0]?.focus();
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
        <button
          className="refresh-button"
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
      </header>

      <div
        ref={gridRef}
        className="apps-viewport"
        aria-busy={loading}
        onWheel={(event) => {
          if (event.ctrlKey || loading || pageCount < 2) return;
          const now = Date.now();
          if (now - wheel.current.time < 650) return;
          wheel.current.delta +=
            Math.abs(event.deltaX) > Math.abs(event.deltaY)
              ? event.deltaX
              : event.deltaY;
          if (Math.abs(wheel.current.delta) > 90) {
            changePage(currentPage + Math.sign(wheel.current.delta));
            wheel.current = { delta: 0, time: now };
          }
        }}
        onTouchStart={(event) => {
          touchStart.current = event.touches[0].clientX;
        }}
        onTouchEnd={(event) => {
          if (touchStart.current === null) return;
          const delta = touchStart.current - event.changedTouches[0].clientX;
          if (Math.abs(delta) > 60) changePage(currentPage + Math.sign(delta));
          touchStart.current = null;
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
          <ul
            className="app-grid"
            key={`${currentPage}-${search}-${pageSize}`}
            aria-label="应用"
            style={
              {
                "--columns": layout.columns,
                "--rows": layout.rows,
              } as CSSProperties
            }
          >
            {visibleApps.map((app, index) => (
              <li className="app-cell" key={app.path}>
                <button
                  className={`app-button${launching === app.path ? " launching" : ""}`}
                  ref={(element) => {
                    buttonRefs.current[index] = element;
                  }}
                  onClick={() => void handleLaunch(app)}
                  onKeyDown={(event) => onGridKeyDown(event, index)}
                  disabled={launching === app.path}
                  title={app.name}
                >
                  <AppIcon app={app} />
                  <span className="app-name">{app.name}</span>
                </button>
              </li>
            ))}
          </ul>
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
