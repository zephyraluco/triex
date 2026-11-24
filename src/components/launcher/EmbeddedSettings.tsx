import { useEffect, useState } from "react";
import {
  Blocks,
  FolderSearch,
  List,
  Palette,
  Plus,
  RefreshCw,
  SlidersHorizontal,
  Trash2,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Separator } from "@/components/ui/separator";
import { Switch } from "@/components/ui/switch";
import {
  ipc,
  type SearchResultItem,
  type ThemeMode,
  type VibrancyMode,
} from "@/lib/ipc";
import { cn } from "@/lib/utils";
import {
  useLauncherStore,
  type SettingsNavKey,
} from "@/stores/useLauncherStore";
import { useSettingsStore } from "@/stores/useSettingsStore";

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

/**
 * Embedded settings view modeled after `satnaing/shadcn-admin`:
 * borderless sidebar nav + ContentSection header (title, description, Separator) +
 * vertical form controls & visual theme preview cards using official shadcn/ui components.
 */
export function EmbeddedSettings() {
  const settingsTab = useLauncherStore((s) => s.settingsTab);
  const setSettingsTab = useLauncherStore((s) => s.setSettingsTab);
  const setStatusMessage = useLauncherStore((s) => s.setStatusMessage);

  const settings = useSettingsStore((s) => s.settings);
  const plugins = useSettingsStore((s) => s.plugins);
  const isLoading = useSettingsStore((s) => s.isLoading);
  const isSaving = useSettingsStore((s) => s.isSaving);
  const error = useSettingsStore((s) => s.error);
  const loadAll = useSettingsStore((s) => s.loadAll);
  const patchSettings = useSettingsStore((s) => s.patchSettings);
  const togglePlugin = useSettingsStore((s) => s.togglePlugin);

  const [hotkeyDraft, setHotkeyDraft] = useState("");
  const [maxResultsDraft, setMaxResultsDraft] = useState("30");
  const [newDir, setNewDir] = useState("");
  const [newExcluded, setNewExcluded] = useState("");
  const [rebuilding, setRebuilding] = useState(false);
  const [allItems, setAllItems] = useState<SearchResultItem[]>([]);
  const [commandFilter, setCommandFilter] = useState("");

  useEffect(() => {
    void loadAll();
  }, [loadAll]);

  useEffect(() => {
    if (settings) {
      setHotkeyDraft(settings.hotkey);
      setMaxResultsDraft(String(settings.maxResults));
    }
  }, [settings]);

  useEffect(() => {
    if (settingsTab !== "commands") return;
    void ipc
      .searchItems(commandFilter, Date.now(), 200)
      .then((res) => setAllItems(res.items))
      .catch(() => {});
  }, [settingsTab, commandFilter]);

  const handleSaveGeneral = async () => {
    if (!settings) return;
    const parsedMax = Number(maxResultsDraft);
    const maxResults =
      Number.isFinite(parsedMax) && parsedMax >= 5 && parsedMax <= 100
        ? Math.round(parsedMax)
        : settings.maxResults;
    await patchSettings({
      hotkey: hotkeyDraft.trim() || settings.hotkey,
      maxResults,
    });
    setStatusMessage("设置已保存");
    window.setTimeout(() => setStatusMessage(null), 1600);
  };

  const handleRebuild = async () => {
    setRebuilding(true);
    try {
      const status = await ipc.rebuildIndex();
      setStatusMessage(`已索引 ${status.totalIndexed} 项`);
      window.setTimeout(() => setStatusMessage(null), 1800);
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

  return (
    <div className="flex h-[516px] w-full gap-6 overflow-hidden px-5 py-4">
      {/* Left Sidebar Navigation (shadcn-admin borderless style) */}
      <aside className="flex w-[168px] shrink-0 flex-col">
        <nav className="flex flex-col space-y-1">
          {NAV_ITEMS.map(({ key, label, Icon }) => {
            const active = settingsTab === key;
            return (
              <Button
                key={key}
                type="button"
                variant="ghost"
                onClick={() => setSettingsTab(key)}
                className={cn(
                  "h-9 w-full justify-start gap-2.5 px-3 text-sm font-normal",
                  active
                    ? "bg-muted font-medium text-foreground hover:bg-muted"
                    : "text-muted-foreground hover:bg-transparent hover:text-foreground hover:underline",
                )}
              >
                <Icon className="size-4 shrink-0" />
                <span>{label}</span>
              </Button>
            );
          })}
        </nav>

        <div className="mt-auto px-3 pt-4 text-[11px] text-muted-foreground">
          Triex v0.1.0
        </div>
      </aside>

      {/* Right Content Section */}
      <ScrollArea className="h-full min-w-0 flex-1">
        <main className="w-full min-w-0 pr-3 pb-6">
          {error && (
            <div className="mb-4 rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-xs text-destructive">
              {error}
            </div>
          )}

          {isLoading || !settings ? (
            <div className="py-16 text-center text-sm text-muted-foreground">
              正在加载配置...
            </div>
          ) : settingsTab === "general" ? (
            <ContentSection
              title="常规"
              description="管理全局呼出快捷键、启动行为与搜索展示数量。"
            >
              <div className="max-w-lg space-y-6">
                <div className="space-y-2">
                  <Label htmlFor="hotkey">呼出快捷键</Label>
                  <Input
                    id="hotkey"
                    value={hotkeyDraft}
                    onChange={(e) => setHotkeyDraft(e.target.value)}
                    placeholder="Alt+Space"
                    className="w-56 font-mono text-sm"
                  />
                  <p className="text-[0.8rem] text-muted-foreground">
                    用于随时在任意应用上方唤起或隐藏 Triex（例如 Alt+Space、Ctrl+Space）。
                  </p>
                </div>

                <div className="space-y-2">
                  <Label htmlFor="max-results">最大搜索返回条数</Label>
                  <Input
                    id="max-results"
                    type="number"
                    min={5}
                    max={100}
                    value={maxResultsDraft}
                    onChange={(e) => setMaxResultsDraft(e.target.value)}
                    className="w-32"
                  />
                  <p className="text-[0.8rem] text-muted-foreground">
                    每次模糊匹配最多展示的结果数量（范围 5 ~ 100）。
                  </p>
                </div>

                <div className="space-y-3">
                  <div className="flex items-center justify-between rounded-lg border border-border px-4 py-3">
                    <div className="space-y-0.5 pr-4">
                      <Label htmlFor="autostart" className="text-sm font-medium">
                        开机自动启动
                      </Label>
                      <p className="text-[0.8rem] text-muted-foreground">
                        登录 Windows 时自动静默驻留系统托盘。
                      </p>
                    </div>
                    <Switch
                      id="autostart"
                      checked={settings.launchOnStartup}
                      onCheckedChange={(checked) =>
                        void patchSettings({ launchOnStartup: checked })
                      }
                    />
                  </div>

                  <div className="flex items-center justify-between rounded-lg border border-border px-4 py-3">
                    <div className="space-y-0.5 pr-4">
                      <Label htmlFor="hide-blur" className="text-sm font-medium">
                        失焦自动隐藏
                      </Label>
                      <p className="text-[0.8rem] text-muted-foreground">
                        点击启动器外部区域时自动隐藏搜索窗口。
                      </p>
                    </div>
                    <Switch
                      id="hide-blur"
                      checked={settings.hideOnBlur}
                      onCheckedChange={(checked) =>
                        void patchSettings({ hideOnBlur: checked })
                      }
                    />
                  </div>
                </div>

                <Button
                  type="button"
                  disabled={
                    isSaving ||
                    (hotkeyDraft.trim() === settings.hotkey &&
                      maxResultsDraft === String(settings.maxResults))
                  }
                  onClick={() => void handleSaveGeneral()}
                >
                  更新偏好设置
                </Button>
              </div>
            </ContentSection>
          ) : settingsTab === "appearance" ? (
            <ContentSection
              title="外观"
              description="自定义应用的外观主题与 Windows 窗口材质，支持自动跟随系统切换。"
            >
              <div className="max-w-lg space-y-7">
                <div className="space-y-2">
                  <Label htmlFor="vibrancy-select">窗口材质</Label>
                  <Select
                    value={settings.vibrancy}
                    onValueChange={(value) =>
                      void patchSettings({ vibrancy: value as VibrancyMode })
                    }
                  >
                    <SelectTrigger id="vibrancy-select" className="w-56">
                      <SelectValue placeholder="选择窗口材质" />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="mica">Mica（云母）</SelectItem>
                      <SelectItem value="acrylic">Acrylic（亚克力）</SelectItem>
                      <SelectItem value="none">关闭透明材质</SelectItem>
                    </SelectContent>
                  </Select>
                  <p className="text-[0.8rem] text-muted-foreground">
                    设置启动器窗体在 Windows 11 上使用的背景材质效果。
                  </p>
                </div>

                <div className="space-y-2">
                  <Label>主题</Label>
                  <p className="text-[0.8rem] text-muted-foreground">
                    选择启动器的浅色、深色或跟随系统主题。
                  </p>

                  <RadioGroup
                    value={settings.theme}
                    onValueChange={(val) =>
                      void patchSettings({ theme: val as ThemeMode })
                    }
                    className="grid max-w-md grid-cols-3 gap-4 pt-2"
                  >
                    <ThemePreviewOption
                      value="light"
                      label="浅色"
                      selected={settings.theme === "light"}
                      variant="light"
                    />
                    <ThemePreviewOption
                      value="dark"
                      label="深色"
                      selected={settings.theme === "dark"}
                      variant="dark"
                    />
                    <ThemePreviewOption
                      value="system"
                      label="跟随系统"
                      selected={settings.theme === "system"}
                      variant="system"
                    />
                  </RadioGroup>
                </div>
              </div>
            </ContentSection>
          ) : settingsTab === "index" ? (
            <ContentSection
              title="本地启动"
              description="管理开始菜单 (.lnk)、UWP / MSIX 应用扫描目录与屏蔽项。"
            >
              <div className="max-w-lg space-y-6">
                <div className="flex items-center justify-between rounded-lg border border-border px-4 py-3">
                  <div className="space-y-0.5 pr-4">
                    <div className="text-sm font-medium">应用索引与图标缓存</div>
                    <p className="text-[0.8rem] text-muted-foreground">
                      重新扫描本地应用并刷新图标缓存。
                    </p>
                  </div>
                  <Button
                    size="sm"
                    variant="outline"
                    disabled={rebuilding}
                    onClick={() => void handleRebuild()}
                  >
                    <RefreshCw className={cn("size-3.5", rebuilding && "animate-spin")} />
                    <span>{rebuilding ? "扫描中..." : "立即重建"}</span>
                  </Button>
                </div>

                <div className="space-y-2">
                  <Label htmlFor="custom-dir">自定义扫描目录</Label>
                  <div className="flex gap-2">
                    <Input
                      id="custom-dir"
                      placeholder="例如 D:\PortableApps"
                      value={newDir}
                      onChange={(e) => setNewDir(e.target.value)}
                      onKeyDown={(e) => e.key === "Enter" && handleAddDir()}
                      className="h-9 text-sm"
                    />
                    <Button variant="secondary" onClick={handleAddDir}>
                      <Plus className="size-4" />
                      添加
                    </Button>
                  </div>
                  <p className="text-[0.8rem] text-muted-foreground">
                    将绿色软件目录加入扫描范围，自动索引其中的可执行文件与快捷方式。
                  </p>
                  {settings.customScanDirs.length > 0 && (
                    <div className="space-y-1.5 pt-1">
                      {settings.customScanDirs.map((dir) => (
                        <div
                          key={dir}
                          className="flex items-center justify-between rounded-md border border-border px-3 py-1.5 text-xs"
                        >
                          <span className="truncate font-mono">{dir}</span>
                          <Button
                            type="button"
                            variant="ghost"
                            size="icon"
                            className="size-6 shrink-0 text-muted-foreground hover:text-destructive"
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
                  <Label htmlFor="excluded-app">屏蔽应用列表</Label>
                  <div className="flex gap-2">
                    <Input
                      id="excluded-app"
                      placeholder="输入不希望显示的应用名称..."
                      value={newExcluded}
                      onChange={(e) => setNewExcluded(e.target.value)}
                      onKeyDown={(e) => e.key === "Enter" && handleAddExcluded()}
                      className="h-9 text-sm"
                    />
                    <Button variant="secondary" onClick={handleAddExcluded}>
                      <Plus className="size-4" />
                      屏蔽
                    </Button>
                  </div>
                  {settings.excludedApps.length > 0 && (
                    <div className="flex flex-wrap gap-1.5 pt-1">
                      {settings.excludedApps.map((name) => (
                        <Badge key={name} variant="secondary" className="gap-1 pr-1">
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
            </ContentSection>
          ) : settingsTab === "plugins" ? (
            <ContentSection
              title="已安装插件"
              description="管理位于 %LocalAppData%\triex\plugins\ 下的本地插件包。"
            >
              {plugins.length === 0 ? (
                <div className="rounded-lg border border-dashed border-border py-12 text-center">
                  <Blocks className="mx-auto size-8 text-muted-foreground/50" />
                  <p className="mt-2.5 text-sm font-medium">暂未安装本地插件</p>
                  <p className="mx-auto mt-1 max-w-sm text-xs text-muted-foreground">
                    将包含 <code className="rounded bg-muted px-1">plugin.json</code>{" "}
                    的插件目录放入本地插件文件夹后重建索引即可启用。
                  </p>
                </div>
              ) : (
                <div className="space-y-3">
                  {plugins.map((plugin) => (
                    <div
                      key={plugin.id}
                      className="flex items-center justify-between gap-4 rounded-lg border border-border px-4 py-3"
                    >
                      <div className="min-w-0 flex-1 space-y-1">
                        <div className="flex items-center gap-2">
                          <span className="text-sm font-medium">{plugin.name}</span>
                          <Badge variant="outline">v{plugin.version}</Badge>
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
            </ContentSection>
          ) : (
            <ContentSection
              title={`所有指令 (${allItems.length})`}
              description="查看并快速测试当前内存索引中的全部应用、系统指令与插件。"
              action={
                <Input
                  placeholder="筛选指令..."
                  value={commandFilter}
                  onChange={(e) => setCommandFilter(e.target.value)}
                  className="h-8 w-44 text-xs"
                />
              }
            >
              <div className="divide-y divide-border">
                {allItems.map((item) => (
                  <div
                    key={item.id}
                    className="flex items-center justify-between gap-3 py-2.5"
                  >
                    <div className="min-w-0 flex-1">
                      <div className="truncate text-sm font-medium text-foreground">
                        {item.name}
                      </div>
                      <div className="truncate text-xs text-muted-foreground">
                        {item.subtitle || item.execPath}
                      </div>
                    </div>
                    <Button
                      size="sm"
                      variant="outline"
                      className="h-7 shrink-0 text-xs"
                      onClick={() => void ipc.launchItem(item.id, false)}
                    >
                      启动
                    </Button>
                  </div>
                ))}
              </div>
            </ContentSection>
          )}
        </main>
      </ScrollArea>
    </div>
  );
}

/**
 * Standard `shadcn-admin` ContentSection wrapper: Title + Description + horizontal Separator.
 */
function ContentSection({
  title,
  description,
  action,
  children,
}: {
  title: string;
  description: string;
  action?: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <div className="flex flex-col">
      <div className="flex items-start justify-between gap-4">
        <div className="min-w-0">
          <h3 className="text-lg font-medium tracking-tight">{title}</h3>
          <p className="text-sm text-muted-foreground">{description}</p>
        </div>
        {action && <div className="shrink-0">{action}</div>}
      </div>
      <Separator className="my-4" />
      {children}
    </div>
  );
}

/**
 * Visual theme preview card modeled directly after `satnaing/shadcn-admin` Appearance settings.
 */
function ThemePreviewOption({
  value,
  label,
  selected,
  variant,
}: {
  value: ThemeMode;
  label: string;
  selected: boolean;
  variant: "light" | "dark" | "system";
}) {
  const id = `theme-option-${value}`;

  return (
    <Label htmlFor={id} className="flex cursor-pointer flex-col items-center gap-2 font-normal">
      <RadioGroupItem id={id} value={value} className="sr-only" />
      <div
        className={cn(
          "w-full rounded-md border-2 p-1 transition-colors",
          selected ? "border-primary" : "border-muted hover:border-accent",
        )}
      >
        {variant === "light" ? (
          <div className="space-y-1.5 rounded-sm bg-[#ecedef] p-2">
            <div className="space-y-1.5 rounded-md bg-white p-2 shadow-xs">
              <div className="h-1.5 w-12 rounded-lg bg-[#ecedef]" />
              <div className="h-1.5 w-16 rounded-lg bg-[#ecedef]" />
            </div>
            <div className="flex items-center gap-1.5 rounded-md bg-white p-2 shadow-xs">
              <div className="size-3 rounded-full bg-[#ecedef]" />
              <div className="h-1.5 w-14 rounded-lg bg-[#ecedef]" />
            </div>
          </div>
        ) : variant === "dark" ? (
          <div className="space-y-1.5 rounded-sm bg-slate-950 p-2">
            <div className="space-y-1.5 rounded-md bg-slate-800 p-2 shadow-xs">
              <div className="h-1.5 w-12 rounded-lg bg-slate-400" />
              <div className="h-1.5 w-16 rounded-lg bg-slate-400" />
            </div>
            <div className="flex items-center gap-1.5 rounded-md bg-slate-800 p-2 shadow-xs">
              <div className="size-3 rounded-full bg-slate-400" />
              <div className="h-1.5 w-14 rounded-lg bg-slate-400" />
            </div>
          </div>
        ) : (
          <div className="grid grid-cols-2 overflow-hidden rounded-sm">
            <div className="space-y-1.5 bg-[#ecedef] p-2 pr-1">
              <div className="space-y-1.5 rounded-l-md bg-white p-2 shadow-xs">
                <div className="h-1.5 w-6 rounded-lg bg-[#ecedef]" />
                <div className="h-1.5 w-8 rounded-lg bg-[#ecedef]" />
              </div>
              <div className="flex items-center gap-1 rounded-l-md bg-white p-2 shadow-xs">
                <div className="size-3 rounded-full bg-[#ecedef]" />
              </div>
            </div>
            <div className="space-y-1.5 bg-slate-950 p-2 pl-1">
              <div className="space-y-1.5 rounded-r-md bg-slate-800 p-2 shadow-xs">
                <div className="h-1.5 w-6 rounded-lg bg-slate-400" />
                <div className="h-1.5 w-8 rounded-lg bg-slate-400" />
              </div>
              <div className="flex items-center gap-1 rounded-r-md bg-slate-800 p-2 shadow-xs">
                <div className="h-1.5 w-6 rounded-lg bg-slate-400" />
              </div>
            </div>
          </div>
        )}
      </div>
      <span className="text-xs font-medium">{label}</span>
    </Label>
  );
}
