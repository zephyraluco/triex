import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import type { ThemeMode, VibrancyMode } from "@/lib/ipc";
import { useSettingsStore } from "@/stores/useSettingsStore";

export function GeneralSettings() {
  const settings = useSettingsStore((s) => s.settings);
  const patchSettings = useSettingsStore((s) => s.patchSettings);
  const isSaving = useSettingsStore((s) => s.isSaving);

  const [hotkeyDraft, setHotkeyDraft] = useState(settings?.hotkey ?? "Alt+Space");

  if (!settings) return null;

  return (
    <div className="space-y-5">
      <div className="flex items-center justify-between gap-4 rounded-lg border border-border p-4">
        <div>
          <div className="text-sm font-medium">全局呼出快捷键</div>
          <div className="text-xs text-muted-foreground">
            例如 Alt+Space、Ctrl+Space、Alt+Q
          </div>
        </div>
        <div className="flex items-center gap-2">
          <Input
            value={hotkeyDraft}
            onChange={(e) => setHotkeyDraft(e.target.value)}
            className="w-36 font-mono text-xs"
          />
          <Button
            size="sm"
            disabled={isSaving || hotkeyDraft.trim() === settings.hotkey}
            onClick={() => void patchSettings({ hotkey: hotkeyDraft.trim() })}
          >
            应用
          </Button>
        </div>
      </div>

      <div className="space-y-3 rounded-lg border border-border p-4">
        <div className="flex items-center justify-between">
          <div>
            <div className="text-sm font-medium">开机自动启动</div>
            <div className="text-xs text-muted-foreground">
              登录 Windows 时自动静默驻留系统托盘
            </div>
          </div>
          <Switch
            checked={settings.launchOnStartup}
            onCheckedChange={(checked) => void patchSettings({ launchOnStartup: checked })}
          />
        </div>

        <div className="h-px bg-border" />

        <div className="flex items-center justify-between">
          <div>
            <div className="text-sm font-medium">失焦自动隐藏</div>
            <div className="text-xs text-muted-foreground">
              点击启动器外部区域时自动隐藏搜索框
            </div>
          </div>
          <Switch
            checked={settings.hideOnBlur}
            onCheckedChange={(checked) => void patchSettings({ hideOnBlur: checked })}
          />
        </div>
      </div>

      <div className="space-y-4 rounded-lg border border-border p-4">
        <div className="flex items-center justify-between">
          <div>
            <div className="text-sm font-medium">外观主题</div>
            <div className="text-xs text-muted-foreground">
              跟随系统或固定浅色 / 深色模式
            </div>
          </div>
          <div className="flex gap-1.5">
            {(["system", "light", "dark"] as ThemeMode[]).map((mode) => (
              <Button
                key={mode}
                size="sm"
                variant={settings.theme === mode ? "default" : "outline"}
                onClick={() => void patchSettings({ theme: mode })}
              >
                {mode === "system" ? "跟随系统" : mode === "light" ? "浅色" : "深色"}
              </Button>
            ))}
          </div>
        </div>

        <div className="h-px bg-border" />

        <div className="flex items-center justify-between">
          <div>
            <div className="text-sm font-medium">窗口材质（Windows 11）</div>
            <div className="text-xs text-muted-foreground">
              Mica 云母、Acrylic 亚克力或纯色背景
            </div>
          </div>
          <div className="flex gap-1.5">
            {(["mica", "acrylic", "none"] as VibrancyMode[]).map((mode) => (
              <Button
                key={mode}
                size="sm"
                variant={settings.vibrancy === mode ? "default" : "outline"}
                onClick={() => void patchSettings({ vibrancy: mode })}
              >
                {mode === "mica" ? "Mica" : mode === "acrylic" ? "Acrylic" : "关闭"}
              </Button>
            ))}
          </div>
        </div>

        <div className="h-px bg-border" />

        <div className="flex items-center justify-between">
          <div>
            <div className="text-sm font-medium">最大搜索返回条数</div>
            <div className="text-xs text-muted-foreground">
              Rust 后端 Top-K 截断数量（建议 20~50）
            </div>
          </div>
          <Input
            type="number"
            min={5}
            max={100}
            value={settings.maxResults}
            onChange={(e) => {
              const num = Number(e.target.value);
              if (Number.isFinite(num) && num >= 5 && num <= 100) {
                void patchSettings({ maxResults: num });
              }
            }}
            className="w-24 text-right"
          />
        </div>
      </div>
    </div>
  );
}
