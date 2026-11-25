import { useCallback, useEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { File, Folder, FolderOpen } from "lucide-react";
import { Button } from "@/components/ui/button";
import "@/index.css";

const PAGE_SIZE = 50;
const DETACHED = getCurrentWebview().label === "triex-builtin-everything-detached";

const SORTS = [
  ["nameAscending", "名称 ↑"],
  ["nameDescending", "名称 ↓"],
  ["pathAscending", "路径 ↑"],
  ["pathDescending", "路径 ↓"],
  ["sizeAscending", "大小 ↑"],
  ["sizeDescending", "大小 ↓"],
  ["modifiedAscending", "修改时间 ↑"],
  ["modifiedDescending", "修改时间 ↓"],
] as const;
type Sort = (typeof SORTS)[number][0];

interface Row {
  id: string;
  name: string;
  path: string;
  kind: "file" | "folder";
  sizeBytes: number | null;
  modifiedAt: number | null;
}

interface Page {
  rows: Row[];
  total: number;
  nextOffset: number | null;
}

type Status = "ready" | "indexing" | "notRunning" | "unsupportedPlatform" | "backendUnavailable";

interface EverythingBridge {
  status(): Promise<Status>;
  clear(sequence: number): Promise<void>;
  search(request: { query: string; sort: Sort; offset: number; limit: number }, sequence: number): Promise<Page>;
  openResult(id: string, action: "open" | "reveal"): Promise<void>;
}

declare global {
  interface Window {
    __TRIEX_THEME__?: "light" | "dark" | "system";
    triex: { everything: EverythingBridge };
  }
}

const everything = window.triex.everything;

const STATUS_TEXT: Record<Status, string> = {
  ready: "输入文件名或 Everything 查询，例如 ext:pdf 年报",
  indexing: "Everything 正在建立索引，请稍后重试。",
  notRunning: "请启动 Everything 1.4.1 或更新版本，然后重试。",
  unsupportedPlatform: "Everything 搜索仅支持 Windows。",
  backendUnavailable: "无法连接兼容的 Everything 实例。",
};

const ERROR_TEXT: Record<string, string> = {
  not_running: STATUS_TEXT.notRunning,
  indexing: STATUS_TEXT.indexing,
  unsupported_platform: STATUS_TEXT.unsupportedPlatform,
  backend_unavailable: STATUS_TEXT.backendUnavailable,
  busy: "Everything 正忙，请稍后重试。",
  timeout: "Everything 响应超时，请重试。",
  stale_result: "搜索结果已失效，请重新查询。",
  invalid_request: "查询过长或包含无效字符。",
  file_unavailable: "文件已移动、删除或无法访问。",
  open_failed: "Windows 无法执行该文件操作。",
  permission_denied: "本地搜索会话已结束。",
};

function describe(error: unknown) {
  const code = (error as { code?: string } | null)?.code;
  return (code && ERROR_TEXT[code]) || "无法完成 Everything 操作，请重试。";
}

function EverythingSearch() {
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState<Sort>("nameAscending");
  const [offset, setOffset] = useState(0);
  const [retry, setRetry] = useState(0);
  const [selected, setSelected] = useState(0);
  const [page, setPage] = useState<Page | null>(null);
  const [status, setStatus] = useState<Status | null>(null);
  const [error, setError] = useState("");
  const rowRefs = useRef<(HTMLButtonElement | null)[]>([]);
  const epoch = useRef(0);

  const resetPage = useCallback(() => {
    epoch.current += 1;
    setPage(null);
    setSelected(0);
    setError("");
  }, []);

  const changeQuery = useCallback((value: string) => {
    resetPage();
    setOffset(0);
    setQuery(value);
  }, [resetPage]);

  useEffect(() => {
    if (DETACHED) return;
    function onInput(event: Event) {
      changeQuery((event as CustomEvent<string>).detail);
    }
    window.addEventListener("triex:sub-input-change", onInput);
    return () => window.removeEventListener("triex:sub-input-change", onInput);
  }, [changeQuery]);

  useEffect(() => {
    const generation = ++epoch.current;
    const cleared = everything.clear(generation).then(() => true, (failure) => {
      if (epoch.current === generation) setError(describe(failure));
      return false;
    });
    const timer = window.setTimeout(async () => {
      if (!(await cleared) || epoch.current !== generation) return;
      try {
        if (!query.trim()) {
          const state = await everything.status();
          if (epoch.current === generation) setStatus(state);
          return;
        }
        const data = await everything.search({ query, sort, offset, limit: PAGE_SIZE }, generation);
        if (epoch.current === generation) setPage(data);
      } catch (failure) {
        if (epoch.current === generation) setError(describe(failure));
      }
    }, query.trim() ? 180 : 0);
    return () => {
      epoch.current += 1;
      window.clearTimeout(timer);
    };
  }, [query, sort, offset, retry]);

  const perform = useCallback(async (row: Row, action: "open" | "reveal") => {
    const generation = epoch.current;
    try {
      await everything.openResult(row.id, action);
    } catch (failure) {
      if (epoch.current === generation) setError(describe(failure));
    }
  }, []);

  const handleKey = useCallback((key: string) => {
    const rows = page?.rows;
    if (!rows?.length) return false;
    if (key === "Enter" && rows[selected]) {
      void perform(rows[selected], "open");
      return true;
    }
    if (key === "ArrowDown" || key === "ArrowUp") {
      const next = Math.max(0, Math.min(rows.length - 1, selected + (key === "ArrowDown" ? 1 : -1)));
      setSelected(next);
      rowRefs.current[next]?.scrollIntoView({ block: "nearest" });
      return true;
    }
    return false;
  }, [page, perform, selected]);

  useEffect(() => {
    function onHeaderKey(event: Event) {
      handleKey((event as CustomEvent<{ key: string }>).detail.key);
    }
    function onKey(event: KeyboardEvent) {
      if (event.isComposing || event.altKey || event.ctrlKey || event.metaKey) return;
      const target = event.target;
      if (target instanceof Element && target.closest("select, button[data-control]")) return;
      if (handleKey(event.key)) event.preventDefault();
    }
    window.addEventListener("triex:sub-input-keydown", onHeaderKey);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("triex:sub-input-keydown", onHeaderKey);
      window.removeEventListener("keydown", onKey);
    };
  }, [handleKey]);

  function changePage(next: number) {
    resetPage();
    setOffset(next);
  }

  const message = error || (page
    ? page.rows.length ? `共 ${page.total} 项` : "未找到文件"
    : query.trim() ? "正在搜索…" : status ? STATUS_TEXT[status] : "正在检查 Everything…");

  return (
    <main className="flex h-dvh min-h-0 flex-col bg-background text-foreground">
      {DETACHED && (
        <input
          aria-label="搜索文件"
          autoFocus
          value={query}
          onChange={(event) => changeQuery(event.target.value)}
          placeholder="输入文件名或 Everything 查询"
          className="mx-4 mt-4 rounded-md border border-border bg-background px-3 py-2 outline-none focus-visible:ring-2"
        />
      )}
      <div className="flex min-h-12 items-center justify-between gap-3 border-b border-border px-4 py-2">
        <p role="status" className="text-sm text-muted-foreground">{message}</p>
        <div className="flex shrink-0 items-center gap-2">
          {(error || (!query.trim() && status && status !== "ready")) && (
            <Button data-control variant="outline" size="sm" onClick={() => {
              resetPage();
              setStatus(null);
              setRetry((value) => value + 1);
            }}>重试</Button>
          )}
          <select
            aria-label="排序"
            value={sort}
            onChange={(event) => {
              resetPage();
              setOffset(0);
              setSort(event.target.value as Sort);
            }}
            className="rounded-md border border-border bg-background px-2 py-1 text-sm"
          >
            {SORTS.map(([value, title]) => <option key={value} value={value}>{title}</option>)}
          </select>
        </div>
      </div>
      <ul aria-label="搜索结果" className="min-h-0 flex-1 overflow-auto px-2 py-1">
        {page?.rows.map((row, index) => (
          <li key={row.id} className={`flex items-center gap-2 rounded-md px-2 ${selected === index ? "bg-accent" : ""}`}>
            <button
              ref={(element) => { rowRefs.current[index] = element; }}
              type="button"
              onFocus={() => setSelected(index)}
              onClick={() => setSelected(index)}
              onDoubleClick={() => void perform(row, "open")}
              title={row.path}
              className="flex min-w-0 flex-1 items-center gap-3 rounded-md py-2 text-left focus-visible:outline-2"
            >
              {row.kind === "folder" ? <Folder className="size-4 shrink-0" /> : <File className="size-4 shrink-0" />}
              <span className="min-w-0">
                <span className="block truncate text-sm">{row.name}</span>
                <span className="block truncate text-xs text-muted-foreground">{row.path}</span>
              </span>
            </button>
            <Button data-control aria-label={`打开 ${row.name}`} type="button" variant="ghost" size="sm" onClick={() => void perform(row, "open")}>打开</Button>
            <Button data-control aria-label={`定位 ${row.name}`} title="打开所在文件夹" type="button" variant="ghost" size="icon" onClick={() => void perform(row, "reveal")}>
              <FolderOpen className="size-4" />
            </Button>
          </li>
        ))}
      </ul>
      {(offset > 0 || page?.nextOffset != null) && (
        <nav aria-label="搜索结果分页" className="flex items-center justify-between border-t border-border px-4 py-2">
          <Button data-control variant="outline" size="sm" disabled={offset === 0} onClick={() => changePage(Math.max(0, offset - PAGE_SIZE))}>上一页</Button>
          <span className="text-sm text-muted-foreground">第 {Math.floor(offset / PAGE_SIZE) + 1} 页</span>
          <Button data-control variant="outline" size="sm" disabled={page?.nextOffset == null} onClick={() => {
            if (page?.nextOffset != null) changePage(page.nextOffset);
          }}>下一页</Button>
        </nav>
      )}
    </main>
  );
}

const theme = window.__TRIEX_THEME__;
const systemDark = window.matchMedia("(prefers-color-scheme: dark)");
function applyTheme() {
  document.documentElement.classList.toggle("dark", theme === "dark" || (theme === "system" && systemDark.matches));
}
applyTheme();
systemDark.addEventListener("change", applyTheme);

createRoot(document.getElementById("root")!).render(<EverythingSearch />);
