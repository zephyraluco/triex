import { useEffect, useRef } from "react";
import {
  Copy,
  FolderOpen,
  History,
  Pin,
  PinOff,
  Play,
  RefreshCw,
  Settings,
  ShieldAlert,
} from "lucide-react";
import { ipc } from "@/lib/ipc";
import { useLauncherStore } from "@/stores/useLauncherStore";

/**
 * Floating context menu opened via right-click on any grid card or `Ctrl+K`,
 * keeping the main ZTools-style launcher free of a heavy permanent footer bar.
 */
export function ActionMenu() {
  const results = useLauncherStore((s) => s.results);
  const selectedId = useLauncherStore((s) => s.selectedId);
  const pinnedItems = useLauncherStore((s) => s.pinnedItems);
  const setPinnedItems = useLauncherStore((s) => s.setPinnedItems);
  const actionMenuOpen = useLauncherStore((s) => s.actionMenuOpen);
  const actionMenuPos = useLauncherStore((s) => s.actionMenuPos);
  const closeActionMenu = useLauncherStore((s) => s.closeActionMenu);
  const setStatusMessage = useLauncherStore((s) => s.setStatusMessage);

  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!actionMenuOpen) return;
    const onPointerDown = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        closeActionMenu();
      }
    };
    window.addEventListener("mousedown", onPointerDown);
    return () => window.removeEventListener("mousedown", onPointerDown);
  }, [actionMenuOpen, closeActionMenu]);

  const selectedItem = results.find((item) => item.id === selectedId);
  if (!actionMenuOpen || !selectedItem) {
    return null;
  }

  const isWin32App = selectedItem.kind === "app";
  const isPinned = pinnedItems.includes(selectedItem.id);
  const hasRecentScore = selectedItem.score > 0;

  const handleTogglePin = async () => {
    closeActionMenu();
    try {
      const updated = await ipc.togglePinItem(selectedItem.id);
      setPinnedItems(updated);
      setStatusMessage(isPinned ? "已取消固定" : "已固定到常用");
    } catch {
      setStatusMessage("操作失败");
    } finally {
      window.setTimeout(() => setStatusMessage(null), 1500);
    }
  };

  const handleRemoveRecent = async () => {
    closeActionMenu();
    try {
      await ipc.removeRecentItem(selectedItem.id);
      setStatusMessage("已从最近使用移除");
    } catch {
      setStatusMessage("移除失败");
    } finally {
      window.setTimeout(() => setStatusMessage(null), 1500);
    }
  };

  const handleCopyPath = async () => {
    try {
      await navigator.clipboard.writeText(selectedItem.subtitle || selectedItem.execPath);
      setStatusMessage("已复制路径");
      closeActionMenu();
      window.setTimeout(() => setStatusMessage(null), 1600);
    } catch {
      setStatusMessage("复制失败");
    }
  };

  const handleRebuild = async () => {
    closeActionMenu();
    setStatusMessage("正在重建索引...");
    try {
      const status = await ipc.rebuildIndex();
      setStatusMessage(`已更新 ${status.totalIndexed} 项`);
    } finally {
      window.setTimeout(() => setStatusMessage(null), 1800);
    }
  };

  const left = actionMenuPos ? Math.min(actionMenuPos.x, 570) : undefined;
  const top = actionMenuPos ? Math.min(actionMenuPos.y, 240) : undefined;

  return (
    <div
      ref={menuRef}
      style={
        actionMenuPos
          ? { left: `${left}px`, top: `${top}px` }
          : { right: "16px", top: "64px" }
      }
      className="fixed z-50 w-52 rounded-xl border border-border/80 bg-popover/95 p-1.5 text-xs text-popover-foreground shadow-xl backdrop-blur-xl"
    >
      <div className="mb-1 truncate border-b border-border/60 px-2.5 py-1.5 text-[11px] font-medium text-muted-foreground">
        {selectedItem.name}
      </div>

      <button
        type="button"
        onClick={() => {
          closeActionMenu();
          void ipc.launchItem(selectedItem.id, false);
        }}
        className="flex w-full items-center gap-2 rounded-lg px-2.5 py-1.5 text-left hover:bg-accent cursor-pointer"
      >
        <Play className="h-3.5 w-3.5" />
        <span>打开</span>
      </button>

      <button
        type="button"
        onClick={() => void handleTogglePin()}
        className="flex w-full items-center gap-2 rounded-lg px-2.5 py-1.5 text-left hover:bg-accent cursor-pointer"
      >
        {isPinned ? <PinOff className="h-3.5 w-3.5" /> : <Pin className="h-3.5 w-3.5" />}
        <span>{isPinned ? "取消固定" : "固定到常用"}</span>
      </button>

      {hasRecentScore && (
        <button
          type="button"
          onClick={() => void handleRemoveRecent()}
          className="flex w-full items-center gap-2 rounded-lg px-2.5 py-1.5 text-left hover:bg-accent cursor-pointer"
        >
          <History className="h-3.5 w-3.5" />
          <span>从最近使用移除</span>
        </button>
      )}

      {isWin32App && (
        <>
          <button
            type="button"
            onClick={() => {
              closeActionMenu();
              void ipc.launchItem(selectedItem.id, true);
            }}
            className="flex w-full items-center gap-2 rounded-lg px-2.5 py-1.5 text-left hover:bg-accent cursor-pointer"
          >
            <ShieldAlert className="h-3.5 w-3.5 text-amber-500" />
            <span>以管理员身份运行</span>
          </button>

          <button
            type="button"
            onClick={() => {
              closeActionMenu();
              void ipc.openItemFolder(selectedItem.id);
            }}
            className="flex w-full items-center gap-2 rounded-lg px-2.5 py-1.5 text-left hover:bg-accent cursor-pointer"
          >
            <FolderOpen className="h-3.5 w-3.5" />
            <span>打开文件所在位置</span>
          </button>
        </>
      )}

      <button
        type="button"
        onClick={() => void handleCopyPath()}
        className="flex w-full items-center gap-2 rounded-lg px-2.5 py-1.5 text-left hover:bg-accent cursor-pointer"
      >
        <Copy className="h-3.5 w-3.5" />
        <span>复制路径 / 标识</span>
      </button>

      <div className="my-1 h-px bg-border/60" />

      <button
        type="button"
        onClick={() => void handleRebuild()}
        className="flex w-full items-center gap-2 rounded-lg px-2.5 py-1.5 text-left hover:bg-accent cursor-pointer"
      >
        <RefreshCw className="h-3.5 w-3.5" />
        <span>重建应用索引</span>
      </button>

      <button
        type="button"
        onClick={() => {
          closeActionMenu();
          void ipc.openSettings();
        }}
        className="flex w-full items-center gap-2 rounded-lg px-2.5 py-1.5 text-left hover:bg-accent cursor-pointer"
      >
        <Settings className="h-3.5 w-3.5" />
        <span>偏好设置</span>
      </button>
    </div>
  );
}
