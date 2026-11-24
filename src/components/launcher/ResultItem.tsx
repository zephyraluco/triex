import { memo, useState } from "react";
import { AppWindow, Blocks, Terminal } from "lucide-react";
import { CommandItem } from "@/components/ui/command";
import { Badge } from "@/components/ui/badge";
import { HighlightText } from "@/components/launcher/HighlightText";
import { getIconUrl } from "@/lib/protocol";
import type { ItemKind, SearchResultItem } from "@/lib/ipc";

interface ResultItemProps {
  item: SearchResultItem;
  onSelect: (item: SearchResultItem) => void;
}

const KIND_LABELS: Record<ItemKind, string> = {
  app: "应用",
  uwpApp: "UWP",
  systemCmd: "系统",
  plugin: "插件",
};

function FallbackIcon({ kind }: { kind: ItemKind }) {
  if (kind === "systemCmd") {
    return <Terminal className="h-4 w-4 text-amber-500" />;
  }
  if (kind === "plugin") {
    return <Blocks className="h-4 w-4 text-violet-500" />;
  }
  return <AppWindow className="h-4 w-4 text-blue-500" />;
}

export const ResultItem = memo(function ResultItem({
  item,
  onSelect,
}: ResultItemProps) {
  const [imgFailed, setImgFailed] = useState(false);
  const iconUrl = getIconUrl(item.iconKey);

  return (
    <CommandItem
      value={item.id}
      onSelect={() => onSelect(item)}
      className="group"
    >
      <div className="flex h-8 w-8 shrink-0 items-center justify-center overflow-hidden rounded-lg bg-secondary/70">
        {iconUrl && !imgFailed ? (
          <img
            src={iconUrl}
            alt=""
            width={28}
            height={28}
            loading="lazy"
            onError={() => setImgFailed(true)}
            className="h-7 w-7 object-contain"
          />
        ) : (
          <FallbackIcon kind={item.kind} />
        )}
      </div>

      <div className="min-w-0 flex-1">
        <div className="truncate text-[14px] font-medium leading-tight">
          <HighlightText text={item.name} indices={item.matchIndices} />
        </div>
        {item.subtitle && (
          <div className="mt-0.5 truncate text-[11px] text-muted-foreground">
            {item.subtitle}
          </div>
        )}
      </div>

      <Badge variant="outline" className="shrink-0 text-[10px] opacity-80">
        {KIND_LABELS[item.kind]}
      </Badge>
    </CommandItem>
  );
});
