import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  Blocks,
  FolderSearch,
  List,
  Palette,
  SlidersHorizontal,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { AppearanceSettings } from "@/components/settings/AppearanceSettings";
import { CommandSettings } from "@/components/settings/CommandSettings";
import { GeneralSettings } from "@/components/settings/GeneralSettings";
import { IndexSettings } from "@/components/settings/IndexSettings";
import { PluginManager } from "@/components/settings/PluginManager";
import { useNativeContextMenu } from "@/hooks/useNativeContextMenu";
import { useThemeSync } from "@/hooks/useThemeSync";
import { ipc, type AppSettings } from "@/lib/ipc";
import { cn } from "@/lib/utils";
import { useSettingsStore, type SettingsNavKey } from "@/stores/useSettingsStore";

const NAV_ITEMS: Array<{
  key: SettingsNavKey;
  label: string;
  Icon: typeof SlidersHorizontal;
}> = [
  { key: "general", label: "常规", Icon: SlidersHorizontal },
  { key: "appearance", label: "外观", Icon: Palette },
  { key: "index", label: "本地启动", Icon: FolderSearch },
  { key: "plugins", label: "已安装插件", Icon: Blocks },
  { key: "commands", label: "所有指令", Icon: List },
];

const PAGES = {
  general: GeneralSettings,
  appearance: AppearanceSettings,
  index: IndexSettings,
  plugins: PluginManager,
  commands: CommandSettings,
};

export default function SettingsApp() {
  useThemeSync();
  useNativeContextMenu();

  const activeTab = useSettingsStore((s) => s.activeTab);
  const setActiveTab = useSettingsStore((s) => s.setActiveTab);
  const settings = useSettingsStore((s) => s.settings);
  const isLoading = useSettingsStore((s) => s.isLoading);
  const error = useSettingsStore((s) => s.error);
  const loadAll = useSettingsStore((s) => s.loadAll);
  const SettingsPage = PAGES[activeTab];

  useEffect(() => {
    let disposed = false;
    const subscription = listen<AppSettings>("triex://settings-updated", ({ payload }) => {
      useSettingsStore.setState({ settings: payload });
    });
    void subscription.then(() => {
      if (!disposed) void loadAll();
    });
    return () => {
      disposed = true;
      void subscription.then((unlisten) => unlisten());
    };
  }, [loadAll]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !event.defaultPrevented) {
        event.preventDefault();
        void ipc.closeSettings();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  return (
    <div className="flex h-screen w-full overflow-hidden">
      <aside className="flex w-[180px] shrink-0 flex-col border-r border-border/60 px-3 py-3.5">
        <nav className="flex flex-col space-y-1">
          {NAV_ITEMS.map(({ key, label, Icon }) => {
            const active = activeTab === key;
            return (
              <Button
                key={key}
                type="button"
                variant="ghost"
                onClick={() => setActiveTab(key)}
                className={cn(
                  "h-9 w-full justify-start gap-2.5 rounded-lg px-2.5 text-[13px] font-normal transition-colors",
                  active
                    ? "bg-surface-active font-medium text-foreground ring-1 ring-inset ring-border/70 hover:bg-surface-active"
                    : "text-muted-foreground hover:bg-surface-hover hover:text-foreground",
                )}
              >
                <Icon className="size-4 shrink-0" />
                <span>{label}</span>
              </Button>
            );
          })}
        </nav>

        <div className="mt-auto px-2.5 pt-4 text-[11px] text-muted-foreground/80">
          Triex v0.1.0
        </div>
      </aside>

      <ScrollArea className="h-full min-w-0 flex-1">
        <main className="w-full min-w-0 px-5 pt-4 pb-6">
          {error && (
            <div className="mb-4 rounded-xl border border-destructive/40 bg-destructive/10 px-3 py-2 text-xs text-destructive">
              {error}
            </div>
          )}

          {isLoading || !settings ? (
            <div className="py-16 text-center text-sm text-muted-foreground">
              正在加载配置...
            </div>
          ) : (
            <SettingsPage />
          )}
        </main>
      </ScrollArea>
    </div>
  );
}
