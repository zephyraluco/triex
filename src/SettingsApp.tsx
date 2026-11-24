import { useEffect } from "react";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { GeneralSettings } from "@/components/settings/GeneralSettings";
import { IndexSettings } from "@/components/settings/IndexSettings";
import { PluginManager } from "@/components/settings/PluginManager";
import { useThemeSync } from "@/hooks/useThemeSync";
import { useSettingsStore } from "@/stores/useSettingsStore";

export default function SettingsApp() {
  useThemeSync();

  const loadAll = useSettingsStore((s) => s.loadAll);
  const isLoading = useSettingsStore((s) => s.isLoading);
  const error = useSettingsStore((s) => s.error);

  useEffect(() => {
    void loadAll();
  }, [loadAll]);

  return (
    <div className="min-h-screen bg-background p-6 text-foreground">
      <div className="mx-auto max-w-2xl space-y-5">
        <header>
          <h1 className="text-xl font-semibold tracking-tight">Triex 偏好设置</h1>
          <p className="mt-1 text-xs text-muted-foreground">
            配置全局快捷键、Windows 外观材质、应用扫描目录与本地插件
          </p>
        </header>

        {error && (
          <div className="rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-xs text-destructive">
            {error}
          </div>
        )}

        {isLoading ? (
          <div className="py-12 text-center text-sm text-muted-foreground">
            正在加载配置...
          </div>
        ) : (
          <Tabs defaultValue="general">
            <TabsList>
              <TabsTrigger value="general">常规与外观</TabsTrigger>
              <TabsTrigger value="index">应用索引</TabsTrigger>
              <TabsTrigger value="plugins">插件管理</TabsTrigger>
            </TabsList>

            <TabsContent value="general">
              <GeneralSettings />
            </TabsContent>

            <TabsContent value="index">
              <IndexSettings />
            </TabsContent>

            <TabsContent value="plugins">
              <PluginManager />
            </TabsContent>
          </Tabs>
        )}
      </div>
    </div>
  );
}
