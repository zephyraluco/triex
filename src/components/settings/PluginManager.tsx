import { Badge } from "@/components/ui/badge";
import { Switch } from "@/components/ui/switch";
import { useSettingsStore } from "@/stores/useSettingsStore";
import {
Blocks
} from "lucide-react";

import { ContentSection } from "./SettingsPrimitives";

export function PluginManager() {
  const plugins = useSettingsStore((s) => s.plugins);
  const togglePlugin = useSettingsStore((s) => s.togglePlugin);
  return (<ContentSection
              title="已安装插件"
              description="管理位于 %LocalAppData%\triex\plugins\ 下的本地插件包。"
            >
              {plugins.length === 0 ? (
                <div className="rounded-xl border border-dashed border-border/80 bg-surface/60 py-12 text-center">
                  <Blocks className="mx-auto size-8 text-muted-foreground/50" />
                  <p className="mt-2.5 text-sm font-medium">暂未安装本地插件</p>
                  <p className="mx-auto mt-1 max-w-sm text-xs text-muted-foreground">
                    将包含 <code className="rounded bg-surface-active px-1">plugin.json</code>{" "}
                    的插件目录放入本地插件文件夹后重建索引即可启用。
                  </p>
                </div>
              ) : (
                <div className="space-y-2.5">
                  {plugins.map((plugin) => (
                    <div
                      key={plugin.id}
                      className="flex items-center justify-between gap-4 rounded-xl border border-border/70 bg-surface px-4 py-3"
                    >
                      <div className="min-w-0 flex-1 space-y-1">
                        <div className="flex items-center gap-2">
                          <span className="text-[13px] font-medium">{plugin.name}</span>
                          <Badge
                            variant="outline"
                            className="rounded-md border-border/70 text-[11px] font-normal"
                          >
                            v{plugin.version}
                          </Badge>
                        </div>
                        {plugin.description && (
                          <p className="truncate text-xs text-muted-foreground">
                            {plugin.description}
                          </p>
                        )}
                      </div>
                      <Switch
                        checked={plugin.enabled}
                        onCheckedChange={(checked) =>
                          void togglePlugin(plugin.id, checked)
                        }
                      />
                    </div>
                  ))}
                </div>
              )}
            </ContentSection>);
}
