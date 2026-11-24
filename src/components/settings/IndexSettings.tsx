import { useState } from "react";
import { Plus, RefreshCw, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ipc } from "@/lib/ipc";
import { useSettingsStore } from "@/stores/useSettingsStore";

export function IndexSettings() {
  const settings = useSettingsStore((s) => s.settings);
  const patchSettings = useSettingsStore((s) => s.patchSettings);

  const [newDir, setNewDir] = useState("");
  const [newExcluded, setNewExcluded] = useState("");
  const [rebuilding, setRebuilding] = useState(false);
  const [lastCount, setLastCount] = useState<number | null>(null);

  if (!settings) return null;

  const handleRebuild = async () => {
    setRebuilding(true);
    try {
      const status = await ipc.rebuildIndex();
      setLastCount(status.totalIndexed);
    } finally {
      setRebuilding(false);
    }
  };

  const handleAddDir = () => {
    const trimmed = newDir.trim();
    if (!trimmed || settings.customScanDirs.includes(trimmed)) return;
    setNewDir("");
    void patchSettings({
      customScanDirs: [...settings.customScanDirs, trimmed],
    });
  };

  const handleRemoveDir = (target: string) => {
    void patchSettings({
      customScanDirs: settings.customScanDirs.filter((d) => d !== target),
    });
  };

  const handleAddExcluded = () => {
    const trimmed = newExcluded.trim();
    if (!trimmed || settings.excludedApps.includes(trimmed)) return;
    setNewExcluded("");
    void patchSettings({
      excludedApps: [...settings.excludedApps, trimmed],
    });
  };

  const handleRemoveExcluded = (target: string) => {
    void patchSettings({
      excludedApps: settings.excludedApps.filter((a) => a !== target),
    });
  };

  return (
    <div className="space-y-5">
      <div className="flex items-center justify-between rounded-lg border border-border p-4">
        <div>
          <div className="text-sm font-medium">应用索引与图标缓存</div>
          <div className="text-xs text-muted-foreground">
            自动扫描开始菜单 (.lnk)、UWP / MSIX 应用及系统指令
            {lastCount !== null && `（当前已索引 ${lastCount} 项）`}
          </div>
        </div>
        <Button size="sm" disabled={rebuilding} onClick={() => void handleRebuild()}>
          <RefreshCw className={`h-3.5 w-3.5 ${rebuilding ? "animate-spin" : ""}`} />
          <span>{rebuilding ? "扫描中..." : "立即重建索引"}</span>
        </Button>
      </div>

      <div className="space-y-3 rounded-lg border border-border p-4">
        <div>
          <div className="text-sm font-medium">自定义扫描目录</div>
          <div className="text-xs text-muted-foreground">
            添加绿色软件目录以索引其中的 .exe 与 .lnk 快捷方式
          </div>
        </div>

        <div className="flex gap-2">
          <Input
            placeholder="例如 D:\PortableApps"
            value={newDir}
            onChange={(e) => setNewDir(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && handleAddDir()}
          />
          <Button size="sm" variant="secondary" onClick={handleAddDir}>
            <Plus className="h-4 w-4" />
            添加
          </Button>
        </div>

        {settings.customScanDirs.length > 0 && (
          <ul className="space-y-1.5">
            {settings.customScanDirs.map((dir) => (
              <li
                key={dir}
                className="flex items-center justify-between rounded-md bg-secondary/60 px-3 py-1.5 text-xs"
              >
                <span className="truncate font-mono">{dir}</span>
                <button
                  type="button"
                  onClick={() => handleRemoveDir(dir)}
                  className="text-muted-foreground hover:text-destructive cursor-pointer"
                >
                  <Trash2 className="h-3.5 w-3.5" />
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>

      <div className="space-y-3 rounded-lg border border-border p-4">
        <div>
          <div className="text-sm font-medium">屏蔽应用列表</div>
          <div className="text-xs text-muted-foreground">
            输入不希望出现在搜索结果中的应用名称
          </div>
        </div>

        <div className="flex gap-2">
          <Input
            placeholder="输入应用名称..."
            value={newExcluded}
            onChange={(e) => setNewExcluded(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && handleAddExcluded()}
          />
          <Button size="sm" variant="secondary" onClick={handleAddExcluded}>
            <Plus className="h-4 w-4" />
            屏蔽
          </Button>
        </div>

        {settings.excludedApps.length > 0 && (
          <div className="flex flex-wrap gap-1.5">
            {settings.excludedApps.map((appName) => (
              <span
                key={appName}
                className="inline-flex items-center gap-1 rounded-md bg-secondary px-2 py-1 text-xs"
              >
                {appName}
                <button
                  type="button"
                  onClick={() => handleRemoveExcluded(appName)}
                  className="text-muted-foreground hover:text-destructive cursor-pointer"
                >
                  <Trash2 className="h-3 w-3" />
                </button>
              </span>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
