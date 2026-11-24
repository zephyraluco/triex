import { RefreshCw, SearchX } from "lucide-react";
import { ipc } from "@/lib/ipc";
import { useLauncherStore } from "@/stores/useLauncherStore";

export function EmptyState() {
  const query = useLauncherStore((s) => s.query);
  const isRebuilding = useLauncherStore((s) => s.isRebuilding);
  const setIsRebuilding = useLauncherStore((s) => s.setIsRebuilding);

  const handleRebuild = async () => {
    setIsRebuilding(true);
    try {
      await ipc.rebuildIndex();
    } finally {
      setIsRebuilding(false);
    }
  };

  return (
    <div className="flex flex-col items-center justify-center gap-2 py-10 text-center">
      <SearchX className="h-8 w-8 text-muted-foreground/60" />
      <p className="text-sm font-medium text-foreground">
        {query ? `未找到与 “${query}” 匹配的项目` : "正在加载应用索引..."}
      </p>
      <p className="text-xs text-muted-foreground">
        支持应用中文名、全拼（weixin）及首字母缩写（wx）搜索
      </p>
      <button
        type="button"
        disabled={isRebuilding}
        onClick={() => void handleRebuild()}
        className="mt-2 inline-flex items-center gap-1.5 rounded-md border border-border bg-secondary/70 px-3 py-1.5 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:opacity-50 cursor-pointer"
      >
        <RefreshCw className={`h-3.5 w-3.5 ${isRebuilding ? "animate-spin" : ""}`} />
        <span>{isRebuilding ? "正在重建索引..." : "立即重建应用索引"}</span>
      </button>
    </div>
  );
}
