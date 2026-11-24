import { Blocks } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Switch } from "@/components/ui/switch";
import { useSettingsStore } from "@/stores/useSettingsStore";

export function PluginManager() {
  const plugins = useSettingsStore((s) => s.plugins);
  const togglePlugin = useSettingsStore((s) => s.togglePlugin);

  if (plugins.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center rounded-lg border border-dashed border-border py-14 text-center">
        <Blocks className="h-10 w-10 text-muted-foreground/50" />
        <p className="mt-3 text-sm font-medium">暂未安装本地插件</p>
        <p className="mt-1 max-w-md text-xs text-muted-foreground">
          将包含 <code className="rounded bg-secondary px-1 py-0.5">plugin.json</code>{" "}
          与前端静态资源的插件目录放入{" "}
          <code className="rounded bg-secondary px-1 py-0.5">%LocalAppData%\triex\plugins\</code>{" "}
          后重建索引即可加载。
        </p>
      </div>
    );
  }

  return (
    <div className="space-y-3">
      {plugins.map((plugin) => (
        <div
          key={plugin.id}
          className="flex items-start justify-between gap-4 rounded-lg border border-border p-4"
        >
          <div className="space-y-1">
            <div className="flex items-center gap-2">
              <span className="text-sm font-medium">{plugin.name}</span>
              <Badge variant="outline">v{plugin.version}</Badge>
            </div>
            {plugin.description && (
              <p className="text-xs text-muted-foreground">{plugin.description}</p>
            )}
            {plugin.manifest.permissions.length > 0 && (
              <div className="flex flex-wrap gap-1 pt-1">
                {plugin.manifest.permissions.map((perm) => (
                  <Badge key={perm} variant="secondary" className="text-[10px]">
                    {perm}
                  </Badge>
                ))}
              </div>
            )}
          </div>

          <Switch
            checked={plugin.enabled}
            onCheckedChange={(checked) => void togglePlugin(plugin.id, checked)}
          />
        </div>
      ))}
    </div>
  );
}
