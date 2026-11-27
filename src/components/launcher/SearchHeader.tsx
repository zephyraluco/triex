import type {
  ClipboardEvent,
  KeyboardEvent,
  MouseEvent as ReactMouseEvent,
  RefObject,
} from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  ArrowLeft,
  Blocks,
  ExternalLink,
  FileText,
  Image as ImageIcon,
  MoreVertical,
  X,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { ipc } from "@/lib/ipc";
import { cn } from "@/lib/utils";
import { useLauncherStore } from "@/stores/useLauncherStore";

interface SearchHeaderProps {
  inputRef: RefObject<HTMLInputElement | null>;
}

const DRAG_THRESHOLD_PX = 4;

function startNativeWindowDrag() {
  void getCurrentWindow().startDragging();
}

function handleHeaderMouseDown(event: ReactMouseEvent<HTMLDivElement>) {
  if (event.button !== 0) return;
  const target = event.target as HTMLElement | null;
  if (target?.closest("button, input, a, [role='button']")) {
    return;
  }
  startNativeWindowDrag();
}

function handleInputMouseDown(
  event: ReactMouseEvent<HTMLInputElement>,
  currentValue: string,
) {
  if (event.button !== 0 || currentValue.length > 0) return;
  const startX = event.clientX;
  const startY = event.clientY;

  const onMove = (moveEvent: MouseEvent) => {
    const dx = Math.abs(moveEvent.clientX - startX);
    const dy = Math.abs(moveEvent.clientY - startY);
    if (dx >= DRAG_THRESHOLD_PX || dy >= DRAG_THRESHOLD_PX) {
      cleanup();
      startNativeWindowDrag();
    }
  };

  const cleanup = () => {
    window.removeEventListener("mousemove", onMove);
    window.removeEventListener("mouseup", cleanup);
  };

  window.addEventListener("mousemove", onMove);
  window.addEventListener("mouseup", cleanup);
}

function TriexLogoGlyph({ className = "size-4" }: { className?: string }) {
  return (
    <svg viewBox="0 0 24 24" fill="none" className={className} aria-hidden="true">
      <path
        d="M5.5 7h13M12 7v10.5M7.5 11.5h9"
        stroke="currentColor"
        strokeWidth="2.3"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

function TriexBrandBadge() {
  const settingsOpen = useLauncherStore((s) => s.settingsOpen);
  const activePlugin = useLauncherStore((s) => s.activePlugin);
  const viewLoading = useLauncherStore((s) => s.viewLoading);
  const toggleSettings = useLauncherStore((s) => s.toggleSettings);

  const isPluginView = !settingsOpen && Boolean(activePlugin);
  const ringColor = isPluginView ? "text-violet-500" : "text-primary";

  return (
    <div className="relative flex size-9 shrink-0 items-center justify-center">
      {viewLoading && (
        <svg
          viewBox="0 0 44 44"
          className={cn(
            "pointer-events-none absolute -inset-[3px] size-[42px] animate-spin",
            ringColor,
          )}
          aria-hidden="true"
        >
          <circle
            cx="22"
            cy="22"
            r="20"
            fill="none"
            stroke="currentColor"
            strokeWidth="2.2"
            strokeLinecap="round"
            strokeDasharray="36 90"
            className="opacity-90"
          />
        </svg>
      )}

      <Button
        type="button"
        size="icon"
        variant="default"
        onClick={toggleSettings}
        onContextMenu={(event) => {
          event.preventDefault();
          event.stopPropagation();
          void ipc.showBrandContextMenu();
        }}
        aria-label={settingsOpen ? "返回搜索" : "打开设置"}
        className={cn(
          "size-9 shrink-0 rounded-full shadow-xs",
          isPluginView && "bg-violet-600 text-white hover:bg-violet-600/90",
        )}
      >
        {isPluginView ? (
          <Blocks className="size-[18px]" />
        ) : (
          <TriexLogoGlyph className="size-5" />
        )}
      </Button>
    </div>
  );
}

export function SearchHeader({ inputRef }: SearchHeaderProps) {
  const query = useLauncherStore((s) => s.query);
  const setQuery = useLauncherStore((s) => s.setQuery);
  const subInput = useLauncherStore((s) => s.subInput);
  const setSubInput = useLauncherStore((s) => s.setSubInput);
  const settingsOpen = useLauncherStore((s) => s.settingsOpen);
  const closeSettings = useLauncherStore((s) => s.closeSettings);
  const activePlugin = useLauncherStore((s) => s.activePlugin);
  const setActivePlugin = useLauncherStore((s) => s.setActivePlugin);
  const attachment = useLauncherStore((s) => s.attachment);
  const setAttachment = useLauncherStore((s) => s.setAttachment);
  const statusMessage = useLauncherStore((s) => s.statusMessage);

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

  const handlePaste = (event: ClipboardEvent<HTMLInputElement>) => {
    const items = event.clipboardData?.items;
    if (!items) return;

    for (const item of Array.from(items)) {
      if (item.type.startsWith("image/")) {
        event.preventDefault();
        const file = item.getAsFile();
        if (file) {
          const previewUrl = URL.createObjectURL(file);
          setAttachment({
            kind: "image",
            label: file.name || "剪贴板图片",
            previewUrl,
          });
        }
        return;
      }
      if (item.kind === "file") {
        event.preventDefault();
        const file = item.getAsFile();
        if (file) {
          setAttachment({
            kind: "file",
            label: file.name,
          });
        }
        return;
      }
    }

    const pastedText = event.clipboardData.getData("text/plain");
    if (pastedText.length > 60 || pastedText.includes("\n")) {
      event.preventDefault();
      setAttachment({
        kind: "text",
        label: `${pastedText.trim().slice(0, 24)}…`,
        rawText: pastedText,
      });
    }
  };

  const handleInputKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "Backspace" && query.length === 0 && attachment) {
      event.preventDefault();
      setAttachment(null);
    }
  };

  if (settingsOpen) {
    return (
      <div
        onMouseDown={handleHeaderMouseDown}
        className="flex h-16 shrink-0 items-center gap-3 border-b border-border/70 px-4"
      >
        <TriexBrandBadge />

        <div className="flex items-center gap-2.5">
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={() => {
              closeSettings();
              requestAnimationFrame(() => inputRef.current?.focus());
            }}
            className="h-8 gap-1.5 rounded-lg border border-border/70 bg-surface px-2.5 text-xs font-medium text-foreground hover:bg-surface-hover"
          >
            <ArrowLeft className="size-3.5" />
            <span>返回搜索</span>
          </Button>

          <Separator orientation="vertical" className="h-4 bg-border/80" />

          <span className="text-[15px] font-semibold tracking-tight text-foreground">
            偏好设置
          </span>
        </div>

        <div className="h-full flex-1" />

        <div className="flex items-center gap-1.5">
          {statusMessage && (
            <Badge
              variant="secondary"
              className="mr-1 rounded-lg border border-border/70 bg-surface font-normal text-foreground/90"
            >
              {statusMessage}
            </Badge>
          )}

          <Button
            type="button"
            variant="ghost"
            size="icon"
            onClick={() => void ipc.showBrandContextMenu()}
            aria-label="更多操作"
            className="size-8 rounded-lg text-muted-foreground hover:bg-surface-hover hover:text-foreground"
          >
            <MoreVertical className="size-4" />
          </Button>
        </div>
      </div>
    );
  }

  return (
    <div
      onMouseDown={handleHeaderMouseDown}
      className="flex h-16 shrink-0 items-center gap-3 px-4"
    >
      <TriexBrandBadge />

      {activePlugin ? (
        <>
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={handleClosePlugin}
            className="h-8 shrink-0 gap-1.5 rounded-lg border border-border/70 bg-surface px-2.5 text-xs font-medium text-foreground hover:bg-surface-hover"
          >
            <ArrowLeft className="size-3.5" />
            <span>{activePlugin.pluginName}</span>
          </Button>

          <input
            ref={inputRef}
            value={subInput}
            onChange={(e) => handleSubInputChange(e.target.value)}
            onMouseDown={(e) => handleInputMouseDown(e, subInput)}
            placeholder={activePlugin.subInputPlaceholder || "输入插件指令..."}
            autoFocus
            className="h-11 flex-1 bg-transparent text-[20px] font-normal text-foreground outline-none placeholder:text-muted-foreground"
          />

          <Button
            type="button"
            variant="ghost"
            size="icon"
            onClick={handleDetachPlugin}
            aria-label="分离为独立窗口"
            className="size-8 rounded-lg text-muted-foreground hover:bg-surface-hover hover:text-foreground"
          >
            <ExternalLink className="size-4" />
          </Button>

          <Button
            type="button"
            variant="ghost"
            size="icon"
            onClick={() => void ipc.showBrandContextMenu()}
            aria-label="更多操作"
            className="size-8 rounded-lg text-muted-foreground hover:bg-surface-hover hover:text-foreground"
          >
            <MoreVertical className="size-4" />
          </Button>
        </>
      ) : (
        <>
          {attachment && (
            <Badge
              variant="secondary"
              className="h-8 max-w-[200px] shrink-0 gap-1.5 rounded-lg border border-border/70 bg-surface px-2 text-xs font-medium"
            >
              {attachment.kind === "image" && attachment.previewUrl ? (
                <img
                  src={attachment.previewUrl}
                  alt=""
                  className="size-5 rounded object-cover"
                />
              ) : attachment.kind === "image" ? (
                <ImageIcon className="size-3.5 shrink-0 text-primary" />
              ) : (
                <FileText className="size-3.5 shrink-0 text-primary" />
              )}
              <span className="truncate">{attachment.label}</span>
              <button
                type="button"
                onClick={() => setAttachment(null)}
                className="text-muted-foreground hover:text-foreground cursor-pointer"
                aria-label="移除粘贴附件"
              >
                <X className="size-3" />
              </button>
            </Badge>
          )}

          <input
            ref={inputRef}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onPaste={handlePaste}
            onKeyDown={handleInputKeyDown}
            onMouseDown={(e) => handleInputMouseDown(e, query)}
            placeholder={
              attachment
                ? "选择处理方式或继续输入指令..."
                : "搜索应用和指令 / 粘贴文件或图片"
            }
            autoFocus
            className="h-11 flex-1 bg-transparent text-[22px] font-normal tracking-tight text-foreground caret-foreground outline-none placeholder:font-light placeholder:text-muted-foreground/80"
          />

          {statusMessage && (
            <Badge
              variant="secondary"
              className="shrink-0 rounded-lg border border-border/70 bg-surface font-normal text-foreground/90"
            >
              {statusMessage}
            </Badge>
          )}

          {query.length > 0 && (
            <Button
              type="button"
              variant="ghost"
              size="icon"
              onClick={() => {
                setQuery("");
                inputRef.current?.focus();
              }}
              className="size-7 shrink-0 rounded-lg text-muted-foreground hover:bg-surface-hover hover:text-foreground"
              aria-label="清空输入"
            >
              <X className="size-4" />
            </Button>
          )}

          <Button
            type="button"
            variant="ghost"
            size="icon"
            onClick={() => void ipc.showBrandContextMenu()}
            aria-label="更多操作"
            className="size-8 shrink-0 rounded-lg text-muted-foreground hover:bg-surface-hover hover:text-foreground"
          >
            <MoreVertical className="size-4" />
          </Button>
        </>
      )}
    </div>
  );
}
