import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { useSettingsStore } from "@/stores/useSettingsStore";
import { ContentSection, notifyStatus } from "./SettingsPrimitives";

export function GeneralSettings() {
  const settings = useSettingsStore((s) => s.settings);
  const patchSettings = useSettingsStore((s) => s.patchSettings);
  const isSaving = useSettingsStore((s) => s.isSaving);
  const [hotkeyDraft, setHotkeyDraft] = useState("");
  useEffect(() => {
    if (settings) {
      setHotkeyDraft(settings.hotkey);
    }
  }, [settings]);

  const handleSaveGeneral = async () => {
    if (!settings) return;
    const saved = await patchSettings({
      hotkey: hotkeyDraft.trim() || settings.hotkey,
    });
    if (saved) notifyStatus("设置已保存");
  };

  if (!settings) return null;
  return (<ContentSection
              title="常规"
              description="管理全局呼出快捷键与启动行为。"
            >
              <div className="max-w-lg space-y-5">
                <div className="space-y-2">
                  <Label htmlFor="hotkey" className="text-[13px]">
                    呼出快捷键
                  </Label>
                  <Input
                    id="hotkey"
                    value={hotkeyDraft}
                    onChange={(e) => setHotkeyDraft(e.target.value)}
                    placeholder="Alt+Space"
                    className="w-56 rounded-lg border-border/80 bg-surface font-mono text-sm"
                  />
                  <p className="text-xs text-muted-foreground">
                    用于随时在任意应用上方唤起或隐藏 Triex（例如 Alt+Space、Ctrl+Space）。
                  </p>
                </div>

                <div className="space-y-2.5">
                  <div className="flex items-center justify-between rounded-xl border border-border/70 bg-surface px-4 py-3">
                    <div className="space-y-0.5 pr-4">
                      <Label htmlFor="autostart" className="text-[13px] font-medium">
                        开机自动启动
                      </Label>
                      <p className="text-xs text-muted-foreground">
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

                  <div className="flex items-center justify-between rounded-xl border border-border/70 bg-surface px-4 py-3">
                    <div className="space-y-0.5 pr-4">
                      <Label htmlFor="hide-blur" className="text-[13px] font-medium">
                        失焦自动隐藏
                      </Label>
                      <p className="text-xs text-muted-foreground">
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
                  size="sm"
                  className="h-9 rounded-lg px-4"
                  disabled={
                    isSaving || hotkeyDraft.trim() === settings.hotkey
                  }
                  onClick={() => void handleSaveGeneral()}
                >
                  更新偏好设置
                </Button>
              </div>
            </ContentSection>);
}
