import type { RefObject } from "react";
import { Command as CommandPrimitive } from "cmdk";
import {
  ArrowLeft,
  Clipboard,
  ExternalLink,
  Search,
  Settings,
  X,
} from "lucide-react";
import { ipc } from "@/lib/ipc";
import { useLauncherStore } from "@/stores/useLauncherStore";

interface SearchHeaderProps {
  inputRef: RefObject<HTMLInputElement | null>;
}

export function SearchHeader({ inputRef }: SearchHeaderProps) {
  const query = useLauncherStore((s) => s.query);
  const setQuery = useLauncherStore((s) => s.setQuery);
  const subInput = useLauncherStore((s) => s.subInput);
  const setSubInput = useLauncherStore((s) => s.setSubInput);
  const activePlugin = useLauncherStore((s) => s.activePlugin);
  const setActivePlugin = useLauncherStore((s) => s.setActivePlugin);
  const clipboard = useLauncherStore((s) => s.clipboard);
  const setClipboard = useLauncherStore((s) => s.setClipboard);

  const handleClosePlugin = () => {
    void ipc.closePlugin();
    setActivePlugin(null);
  };

  const handleDetachPlugin = () => {
    void ipc.detachPlugin();
    setActivePlugin(null);
  };

  const handleSubInputChange = (value: string) => {
    setSubInput(value);
    void ipc.forwardSubInputValue(value);
  };

  return (
    <div
      data-tauri-drag-region
      className="flex h-[60px] shrink-0 items-center gap-2.5 border-b border-border/70 px-4"
    >
      {activePlugin ? (
        <>
          <button
            type="button"
            onClick={handleClosePlugin}
            title="返回搜索 (Esc)"
            className="flex h-7 items-center gap-1.5 rounded-md bg-secondary/90 px-2.5 text-xs font-medium text-secondary-foreground transition-colors hover:bg-accent cursor-pointer"
          >
            <ArrowLeft className="h-3.5 w-3.5" />
            <span>{activePlugin.pluginName}</span>
          </button>

          <input
            ref={inputRef}
            value={subInput}
            onChange={(e) => handleSubInputChange(e.target.value)}
            placeholder={activePlugin.subInputPlaceholder || "输入插件指令..."}
            autoFocus
            className="h-10 flex-1 bg-transparent text-base text-foreground outline-none placeholder:text-muted-foreground"
          />

          <button
            type="button"
            onClick={handleDetachPlugin}
            title="分离为独立窗口"
            className="flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-accent hover:text-foreground cursor-pointer"
          >
            <ExternalLink className="h-4 w-4" />
          </button>
        </>
      ) : (
        <>
          <Search className="h-5 w-5 shrink-0 text-muted-foreground" />

          <CommandPrimitive.Input
            ref={inputRef}
            value={query}
            onValueChange={setQuery}
            placeholder="搜索应用、系统命令或插件（支持拼音 / 首字母）..."
            autoFocus
            className="h-10 flex-1 bg-transparent text-[15px] text-foreground outline-none placeholder:text-muted-foreground/80"
          />

          {clipboard && query.length === 0 && (
            <div className="flex max-w-[165px] items-center gap-1 rounded-md border border-border/70 bg-secondary/70 px-2 py-1 text-[11px] text-muted-foreground">
              <Clipboard className="h-3 w-3 shrink-0" />
              <span className="truncate">{clipboard.preview}</span>
              <button
                type="button"
                onClick={() => setClipboard(null)}
                className="ml-0.5 text-muted-foreground/70 hover:text-foreground cursor-pointer"
                aria-label="清除剪贴板提示"
              >
                <X className="h-3 w-3" />
              </button>
            </div>
          )}

          {query.length > 0 && (
            <button
              type="button"
              onClick={() => {
                setQuery("");
                inputRef.current?.focus();
              }}
              className="flex h-6 w-6 items-center justify-center rounded-md text-muted-foreground hover:bg-accent hover:text-foreground cursor-pointer"
              aria-label="清空输入"
            >
              <X className="h-3.5 w-3.5" />
            </button>
          )}

          <button
            type="button"
            onClick={() => void ipc.openSettings()}
            title="Triex 偏好设置"
            className="flex h-7 w-7 shrink-0 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-accent hover:text-foreground cursor-pointer"
          >
            <Settings className="h-4 w-4" />
          </button>
        </>
      )}
    </div>
  );
}
