import { Label } from "@/components/ui/label";
import { RadioGroup } from "@/components/ui/radio-group";
import {
Select,
SelectContent,
SelectItem,
SelectTrigger,
SelectValue,
} from "@/components/ui/select";
import {
type ThemeMode,
type VibrancyMode
} from "@/lib/ipc";
import { useSettingsStore } from "@/stores/useSettingsStore";

import { ContentSection,ThemePreviewOption } from "./SettingsPrimitives";

export function AppearanceSettings() {
  const settings = useSettingsStore((s) => s.settings);
  const patchSettings = useSettingsStore((s) => s.patchSettings);
  if (!settings) return null;
  return (<ContentSection
              title="外观"
              description="自定义应用的外观主题与 Windows 窗口材质，支持自动跟随系统切换。"
            >
              <div className="max-w-lg space-y-6">
                <div className="space-y-2">
                  <Label htmlFor="vibrancy-select" className="text-[13px]">
                    窗口材质
                  </Label>
                  <Select
                    value={settings.vibrancy}
                    onValueChange={(value) =>
                      void patchSettings({ vibrancy: value as VibrancyMode })
                    }
                  >
                    <SelectTrigger
                      id="vibrancy-select"
                      className="w-56 rounded-lg border-border/80 bg-surface"
                    >
                      <SelectValue placeholder="选择窗口材质" />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="mica">Mica（云母）</SelectItem>
                      <SelectItem value="acrylic">Acrylic（亚克力）</SelectItem>
                      <SelectItem value="none">关闭透明材质</SelectItem>
                    </SelectContent>
                  </Select>
                  <p className="text-xs text-muted-foreground">
                    设置启动器窗体在 Windows 11 上使用的背景材质效果。
                  </p>
                </div>

                <div className="space-y-2">
                  <Label className="text-[13px]">主题</Label>
                  <p className="text-xs text-muted-foreground">
                    选择启动器的浅色、深色或跟随系统主题。
                  </p>

                  <RadioGroup
                    value={settings.theme}
                    onValueChange={(val) =>
                      void patchSettings({ theme: val as ThemeMode })
                    }
                    className="grid max-w-md grid-cols-3 gap-3.5 pt-2"
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
            </ContentSection>);
}
