import { Copy, FolderOpen, Play, ShieldAlert } from "lucide-react";
import { ipc } from "@/lib/ipc";
import { useLauncherStore } from "@/stores/useLauncherStore";

export function ActionMenu() {
  const results = useLauncherStore((s) => s.results);
  const selectedId = useLauncherStore((s) => s.selectedId);
  const totalIndexed = useLauncherStore((s) => s.totalIndexed);
  const actionMenuOpen = useLauncherStore((s) => s.actionMenuOpen);
  const setActionMenuOpen = useLauncherStore((s) => s.setActionMenuOpen);
  const statusMessage = useLauncherStore((s) => s.statusMessage);
  const setStatusMessage = useLauncherStore((s) => s.setStatusMessage);

  const selectedItem = results.find((item) => item.id === selectedId);
  const isWin32App = selectedItem?.kind === "app";

  const handleCopyPath = async () => {
    if (!selectedItem) return;
    try {
      await navigator.clipboard.writeText(selectedItem.subtitle || selectedItem.execPath);
      setStatusMessage("已复制路径到剪贴板");
      setActionMenuOpen(false);
      window.setTimeout(() => setStatusMessage(null), 1800);
    } catch {
      setStatusMessage("复制失败");
    }
  };

  return (
    <div className="relative flex h-9 shrink-0 items-center justify-between border-t border-border/70 px-3.5 text-[11px] text-muted-foreground">
      <div className="flex items-center gap-2 truncate">
        {statusMessage ? (
          <span className="font-medium text-foreground">{statusMessage}</span>
        ) : (
          <span>已索引 {totalIndexed} 项</span>
        )}
      </div>

      <div className="flex items-center gap-3">
        <span className="inline-flex items-center gap-1">
          <kbd className="rounded border border-border bg-secondary/80 px-1 py-0.5 font-mono text-[10px]">
            Enter
          </kbd>
          打开
        </span>

        {selectedItem && (
          <button
            type="button"
            onClick={() => setActionMenuOpen(!actionMenuOpen)}
            className="inline-flex items-center gap-1 rounded px-1.5 py-0.5 transition-colors hover:bg-accent hover:text-foreground cursor-pointer"
          >
            <span>更多动作</span>
            <kbd className="rounded border border-border bg-secondary/80 px-1 py-0.5 font-mono text-[10px]">
              Ctrl K
            </kbd>
          </button>
        )}
      </div>

      {actionMenuOpen && selectedItem && (
        <div className="absolute right-3 bottom-10 z-50 w-52 rounded-lg border border-border bg-popover/95 p-1 text-xs text-popover-foreground shadow-lg backdrop-blur-md">
          <button
            type="button"
            onClick={() => {
              setActionMenuOpen(false);
              void ipc.launchItem(selectedItem.id, false);
            }}
            className="flex w-full items-center gap-2 rounded-md px-2.5 py-1.5 text-left hover:bg-accent cursor-pointer"
          >
            <Play className="h-3.5 w-3.5" />
            <span>打开</span>
          </button>

          {isWin32App && (
            <>
              <button
                type="button"
                onClick={() => {
                  setActionMenuOpen(false);
                  void ipc.launchItem(selectedItem.id, true);
                }}
                className="flex w-full items-center gap-2 rounded-md px-2.5 py-1.5 text-left hover:bg-accent cursor-pointer"
              >
                <ShieldAlert className="h-3.5 w-3.5 text-amber-500" />
                <span>以管理员身份运行</span>
              </button>

              <button
                type="button"
                onClick={() => {
                  setActionMenuOpen(false);
                  void ipc.openItemFolder(selectedItem.id);
                }}
                className="flex w-full items-center gap-2 rounded-md px-2.5 py-1.5 text-left hover:bg-accent cursor-pointer"
              >
                <FolderOpen className="h-3.5 w-3.5" />
                <span>打开文件所在位置</span>
              </button>
            </>
          )}

          <button
            type="button"
            onClick={() => void handleCopyPath()}
            className="flex w-full items-center gap-2 rounded-md px-2.5 py-1.5 text-left hover:bg-accent cursor-pointer"
          >
            <Copy className="h-3.5 w-3.5" />
            <span>复制路径 / 标识</span>
          </button>
        </div>
      )}
    </div>
  );
}
