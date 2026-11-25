import { useState } from "react";
import { Plus, RefreshCw, Trash2 } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { ipc } from "@/lib/ipc";
import { cn } from "@/lib/utils";
import { useSettingsStore } from "@/stores/useSettingsStore";
import { ContentSection, notifyStatus } from "./SettingsPrimitives";

export function IndexSettings() {
  const settings = useSettingsStore((s) => s.settings);
  const patchSettings = useSettingsStore((s) => s.patchSettings);
  const [newDir, setNewDir] = useState("");
  const [newExcluded, setNewExcluded] = useState("");
  const [rebuilding, setRebuilding] = useState(false);
  const handleRebuild = async () => {
    setRebuilding(true);
    try {
      const status = await ipc.rebuildIndex();
      notifyStatus(`已索引 ${status.totalIndexed} 项`);
    } finally {
      setRebuilding(false);
    }
  };

  const handleAddDir = () => {
    if (!settings) return;
    const trimmed = newDir.trim();
    if (!trimmed || settings.customScanDirs.includes(trimmed)) return;
    setNewDir("");
    void patchSettings({ customScanDirs: [...settings.customScanDirs, trimmed] });
  };

  const handleAddExcluded = () => {
    if (!settings) return;
    const trimmed = newExcluded.trim();
    if (!trimmed || settings.excludedApps.includes(trimmed)) return;
    setNewExcluded("");
    void patchSettings({ excludedApps: [...settings.excludedApps, trimmed] });
  };

  if (!settings) return null;
  return (<ContentSection
              title="本地启动"
              description="管理开始菜单 (.lnk)、UWP / MSIX 应用扫描目录与屏蔽项。"
            >
              <div className="max-w-lg space-y-5">
                <div className="flex items-center justify-between rounded-xl border border-border/70 bg-surface px-4 py-3">
                  <div className="space-y-0.5 pr-4">
                    <div className="text-[13px] font-medium">应用索引与图标缓存</div>
                    <p className="text-xs text-muted-foreground">
                      重新扫描本地应用并刷新图标缓存。
                    </p>
                  </div>
                  <Button
                    size="sm"
                    variant="outline"
                    className="rounded-lg border-border/80 bg-surface hover:bg-surface-hover"
                    disabled={rebuilding}
                    onClick={() => void handleRebuild()}
                  >
                    <RefreshCw className={cn("size-3.5", rebuilding && "animate-spin")} />
                    <span>{rebuilding ? "扫描中..." : "立即重建"}</span>
                  </Button>
                </div>

                <div className="space-y-2">
                  <Label htmlFor="custom-dir" className="text-[13px]">
                    自定义扫描目录
                  </Label>
                  <div className="flex gap-2">
                    <Input
                      id="custom-dir"
                      placeholder="例如 D:\PortableApps"
                      value={newDir}
                      onChange={(e) => setNewDir(e.target.value)}
                      onKeyDown={(e) => e.key === "Enter" && handleAddDir()}
                      className="h-9 rounded-lg border-border/80 bg-surface text-sm"
                    />
                    <Button
                      variant="secondary"
                      className="rounded-lg border border-border/70 bg-surface hover:bg-surface-hover"
                      onClick={handleAddDir}
                    >
                      <Plus className="size-4" />
                      添加
                    </Button>
                  </div>
                  <p className="text-xs text-muted-foreground">
                    将绿色软件目录加入扫描范围，自动索引其中的可执行文件与快捷方式。
                  </p>
                  {settings.customScanDirs.length > 0 && (
                    <div className="space-y-1.5 pt-1">
                      {settings.customScanDirs.map((dir) => (
                        <div
                          key={dir}
                          className="flex items-center justify-between rounded-lg border border-border/70 bg-surface px-3 py-1.5 text-xs"
                        >
                          <span className="truncate font-mono">{dir}</span>
                          <Button
                            type="button"
                            variant="ghost"
                            size="icon"
                            className="size-6 shrink-0 rounded-md text-muted-foreground hover:bg-surface-hover hover:text-destructive"
                            onClick={() =>
                              void patchSettings({
                                customScanDirs: settings.customScanDirs.filter(
                                  (d) => d !== dir,
                                ),
                              })
                            }
                          >
                            <Trash2 className="size-3.5" />
                          </Button>
                        </div>
                      ))}
                    </div>
                  )}
                </div>

                <div className="space-y-2">
                  <Label htmlFor="excluded-app" className="text-[13px]">
                    屏蔽应用列表
                  </Label>
                  <div className="flex gap-2">
                    <Input
                      id="excluded-app"
                      placeholder="输入不希望显示的应用名称..."
                      value={newExcluded}
                      onChange={(e) => setNewExcluded(e.target.value)}
                      onKeyDown={(e) => e.key === "Enter" && handleAddExcluded()}
                      className="h-9 rounded-lg border-border/80 bg-surface text-sm"
                    />
                    <Button
                      variant="secondary"
                      className="rounded-lg border border-border/70 bg-surface hover:bg-surface-hover"
                      onClick={handleAddExcluded}
                    >
                      <Plus className="size-4" />
                      屏蔽
                    </Button>
                  </div>
                  {settings.excludedApps.length > 0 && (
                    <div className="flex flex-wrap gap-1.5 pt-1">
                      {settings.excludedApps.map((name) => (
                        <Badge
                          key={name}
                          variant="secondary"
                          className="gap-1 rounded-lg border border-border/70 bg-surface pr-1 font-normal"
                        >
                          <span>{name}</span>
                          <button
                            type="button"
                            onClick={() =>
                              void patchSettings({
                                excludedApps: settings.excludedApps.filter(
                                  (a) => a !== name,
                                ),
                              })
                            }
                            className="rounded-sm p-0.5 text-muted-foreground hover:text-destructive cursor-pointer"
                          >
                            <Trash2 className="size-3" />
                          </button>
                        </Badge>
                      ))}
                    </div>
                  )}
                </div>
              </div>
            </ContentSection>);
}
