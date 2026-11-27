import { useState } from "react";
import { Blocks, Cpu, FolderOpen, Plus, RefreshCw, Trash2 } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { ipc, type InstalledPluginRecord } from "@/lib/ipc";
import { getPluginAssetUrl } from "@/lib/protocol";
import { useSettingsStore } from "@/stores/useSettingsStore";
import { ContentSection, notifyStatus } from "./SettingsPrimitives";

function PluginIcon({ plugin }: { plugin: InstalledPluginRecord }) {
  const [failed, setFailed] = useState(false);
  const logoUrl = getPluginAssetUrl(plugin.id, plugin.manifest.logo);

  if (logoUrl && !failed) {
    return (
      <img
        src={logoUrl}
        alt=""
        onError={() => setFailed(true)}
        className="size-9 shrink-0 rounded-full object-cover shadow-xs"
      />
    );
  }

  return (
    <div className="flex size-9 shrink-0 items-center justify-center rounded-full bg-violet-600 text-white shadow-xs">
      <Blocks className="size-4" />
    </div>
  );
}

export function PluginManager() {
  const plugins = useSettingsStore((s) => s.plugins);
  const isSaving = useSettingsStore((s) => s.isSaving);
  const togglePlugin = useSettingsStore((s) => s.togglePlugin);
  const pickAndInstallPlugin = useSettingsStore((s) => s.pickAndInstallPlugin);
  const uninstallPlugin = useSettingsStore((s) => s.uninstallPlugin);
  const reloadPlugins = useSettingsStore((s) => s.reloadPlugins);

  const handleAddPlugin = async () => {
    const installedName = await pickAndInstallPlugin();
    if (installedName) {
      notifyStatus(`已安装插件 ${installedName}`);
    }
  };

  const handleUninstall = async (plugin: InstalledPluginRecord) => {
    const ok = await uninstallPlugin(plugin.id);
    if (ok) {
      notifyStatus(`已卸载 ${plugin.name}`);
    }
  };

  const handleReload = async () => {
    await reloadPlugins();
    notifyStatus("插件列表已刷新");
  };

  return (
    <ContentSection
      title="已安装插件"
      description="支持 .txz / .zip 插件安装包，安装于 %LocalAppData%\triex\plugins\。"
      action={
        <div className="flex shrink-0 items-center gap-1.5">
          <Button
            type="button"
            variant="ghost"
            size="sm"
            disabled={isSaving}
            onClick={() => void handleReload()}
            className="h-8 shrink-0 gap-1.5 rounded-lg border border-border/70 bg-surface px-2.5 text-xs"
          >
            <RefreshCw className="size-3.5" />
            <span>刷新</span>
          </Button>
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={() => void ipc.openPluginsDir()}
            className="h-8 shrink-0 gap-1.5 rounded-lg border border-border/70 bg-surface px-2.5 text-xs"
          >
            <FolderOpen className="size-3.5" />
            <span>打开目录</span>
          </Button>
          <Button
            type="button"
            size="sm"
            disabled={isSaving}
            onClick={() => void handleAddPlugin()}
            className="h-8 shrink-0 gap-1.5 rounded-lg border border-transparent px-2.5 text-xs"
          >
            <Plus className="size-3.5" />
            <span>添加插件</span>
          </Button>
        </div>
      }
    >
      {plugins.length === 0 ? (
        <div className="rounded-xl border border-dashed border-border/80 bg-surface/60 py-12 text-center">
          <Blocks className="mx-auto size-8 text-muted-foreground/50" />
          <p className="mt-2.5 text-sm font-medium">暂未安装本地插件</p>
          <p className="mx-auto mt-1 max-w-sm text-xs text-muted-foreground">
            点击右上角「添加插件」选择 <code className="rounded bg-surface-active px-1">.txz</code>{" "}
            或 <code className="rounded bg-surface-active px-1">.zip</code> 安装包，也可直接双击{" "}
            <code className="rounded bg-surface-active px-1">.txz</code> 文件安装。
          </p>
        </div>
      ) : (
        <div className="space-y-2.5">
          {plugins.map((plugin) => (
            <div
              key={plugin.id}
              className="flex items-center justify-between gap-3 rounded-xl border border-border/70 bg-surface px-4 py-3"
            >
              <PluginIcon plugin={plugin} />

              <div className="min-w-0 flex-1 space-y-1">
                <div className="flex flex-wrap items-center gap-1.5">
                  <span className="text-[13px] font-medium">{plugin.name}</span>
                  <Badge
                    variant="outline"
                    className="rounded-md border-border/70 text-[11px] font-normal"
                  >
                    v{plugin.version}
                  </Badge>
                  {plugin.manifest.backend && (
                    <Badge
                      variant="secondary"
                      className="gap-1 rounded-md text-[10px] font-normal"
                    >
                      <Cpu className="size-3" />
                      <span>独立后端</span>
                    </Badge>
                  )}
                  <span className="text-[11px] text-muted-foreground">
                    {plugin.manifest.features.length} 个指令
                  </span>
                </div>
                {plugin.description && (
                  <p className="truncate text-xs text-muted-foreground">
                    {plugin.description}
                  </p>
                )}
                {plugin.manifest.permissions.length > 0 && (
                  <div className="flex flex-wrap gap-1 pt-0.5">
                    {plugin.manifest.permissions.map((perm) => (
                      <span
                        key={perm}
                        className="rounded bg-surface-active px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground"
                      >
                        {perm}
                      </span>
                    ))}
                  </div>
                )}
              </div>

              <div className="flex shrink-0 items-center gap-2">
                <Switch
                  checked={plugin.enabled}
                  disabled={isSaving}
                  onCheckedChange={(checked) =>
                    void togglePlugin(plugin.id, checked)
                  }
                />
                <Button
                  type="button"
                  variant="ghost"
                  size="icon"
                  disabled={isSaving}
                  onClick={() => void handleUninstall(plugin)}
                  aria-label={`卸载 ${plugin.name}`}
                  className="size-8 rounded-lg text-muted-foreground hover:bg-destructive/10 hover:text-destructive"
                >
                  <Trash2 className="size-3.5" />
                </Button>
              </div>
            </div>
          ))}
        </div>
      )}
    </ContentSection>
  );
}
