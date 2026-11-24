import { memo, useState, type MouseEvent } from "react";
import {
  AppWindow,
  Blocks,
  Cpu,
  Folder,
  Lock,
  Moon,
  Power,
  RotateCcw,
  Settings,
  Sliders,
  Terminal,
  Trash2,
} from "lucide-react";
import { CommandItem } from "@/components/ui/command";
import { HighlightText } from "@/components/launcher/HighlightText";
import { getIconUrl } from "@/lib/protocol";
import { cn } from "@/lib/utils";
import type { ItemKind, SearchResultItem } from "@/lib/ipc";

interface ResultItemProps {
  item: SearchResultItem;
  isSelected: boolean;
  onSelect: (item: SearchResultItem) => void;
  onHover: (id: string) => void;
  onContextMenu: (item: SearchResultItem, event: MouseEvent<HTMLDivElement>) => void;
}

/**
 * Renders ZTools-style vivid squircle fallback icons for built-in system commands,
 * plugins, or apps whose cached icon has not yet finished extracting.
 */
function FallbackTileIcon({ item }: { item: SearchResultItem }) {
  if (item.kind === "systemCmd") {
    const { bg, Icon } = resolveSystemIcon(item.execPath);
    return (
      <div
        className={cn(
          "flex h-9 w-9 items-center justify-center rounded-[10px] text-white shadow-xs",
          bg,
        )}
      >
        <Icon className="size-5 text-white" />
      </div>
    );
  }

  if (item.kind === "plugin") {
    return (
      <div className="flex h-9 w-9 items-center justify-center rounded-[10px] bg-violet-600 text-white shadow-xs">
        <Blocks className="size-5 text-white" />
      </div>
    );
  }

  const tileColor: Record<ItemKind, string> = {
    app: "bg-blue-600",
    uwpApp: "bg-sky-600",
    systemCmd: "bg-zinc-700",
    plugin: "bg-violet-600",
  };

  return (
    <div
      className={cn(
        "flex h-9 w-9 items-center justify-center rounded-[10px] text-white shadow-xs",
        tileColor[item.kind],
      )}
    >
      <AppWindow className="size-5 text-white" />
    </div>
  );
}

function resolveSystemIcon(execPath: string) {
  switch (execPath) {
    case "sys:lock":
      return { bg: "bg-amber-500", Icon: Lock };
    case "sys:sleep":
      return { bg: "bg-indigo-500", Icon: Moon };
    case "sys:restart":
      return { bg: "bg-orange-500", Icon: RotateCcw };
    case "sys:shutdown":
      return { bg: "bg-rose-600", Icon: Power };
    case "sys:taskmgr":
      return { bg: "bg-emerald-600", Icon: Cpu };
    case "sys:explorer":
      return { bg: "bg-amber-400", Icon: Folder };
    case "sys:recycle-bin":
      return { bg: "bg-slate-600", Icon: Trash2 };
    case "sys:settings":
    case "triex:settings":
      return { bg: "bg-blue-600", Icon: Settings };
    case "sys:control":
    case "sys:env":
    case "sys:devmgmt":
      return { bg: "bg-cyan-600", Icon: Sliders };
    default:
      return { bg: "bg-blue-600", Icon: Terminal };
  }
}

export const ResultItem = memo(function ResultItem({
  item,
  isSelected,
  onSelect,
  onHover,
  onContextMenu,
}: ResultItemProps) {
  const [imgFailed, setImgFailed] = useState(false);
  const iconUrl = getIconUrl(item.iconKey);

  return (
    <CommandItem
      value={item.id}
      onSelect={() => onSelect(item)}
      onMouseEnter={() => onHover(item.id)}
      onContextMenu={(e) => onContextMenu(item, e)}
      title={item.subtitle ? `${item.name}\n${item.subtitle}` : item.name}
      className={cn(
        "flex h-[88px] w-full flex-col items-center justify-center gap-2 rounded-xl px-1.5 py-2 text-center transition-colors cursor-pointer",
        "data-[selected=true]:bg-black/[0.08] data-[selected=true]:text-foreground dark:data-[selected=true]:bg-white/[0.12]",
        isSelected
          ? "bg-black/[0.08] text-foreground dark:bg-white/[0.12]"
          : "hover:bg-black/[0.05] dark:hover:bg-white/[0.07]",
      )}
    >
      <div className="flex h-10 w-10 shrink-0 items-center justify-center">
        {iconUrl && !imgFailed ? (
          <img
            src={iconUrl}
            alt=""
            width={38}
            height={38}
            loading="lazy"
            onError={() => setImgFailed(true)}
            className="h-[38px] w-[38px] object-contain drop-shadow-[0_1px_2px_rgba(0,0,0,0.12)]"
          />
        ) : (
          <FallbackTileIcon item={item} />
        )}
      </div>

      <span className="w-full truncate text-[12px] font-normal leading-tight text-zinc-800 dark:text-zinc-200">
        <HighlightText text={item.name} indices={item.matchIndices} />
      </span>
    </CommandItem>
  );
});
