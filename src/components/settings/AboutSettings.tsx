import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { listen } from "@tauri-apps/api/event";
import {
  CheckCircle2,
  Download,
  FolderOpen,
  RefreshCw,
  Sparkles,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { ipc, type UpdateProgress } from "@/lib/ipc";
import { useSettingsStore } from "@/stores/useSettingsStore";
import { ContentSection, notifyStatus } from "./SettingsPrimitives";

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
}

export function AboutSettings() {
  const settings = useSettingsStore((s) => s.settings);
  const patchSettings = useSettingsStore((s) => s.patchSettings);
  const availableUpdate = useSettingsStore((s) => s.availableUpdate);
  const setAvailableUpdate = useSettingsStore((s) => s.setAvailableUpdate);

  const [appVersion, setAppVersion] = useState("0.1.0");
  const [checking, setChecking] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [upToDate, setUpToDate] = useState(false);
  const [updateError, setUpdateError] = useState<string | null>(null);
  const [progress, setProgress] = useState<UpdateProgress | null>(null);

  useEffect(() => {
    void getVersion()
      .then((ver) => setAppVersion(ver))
      .catch(() => {});
  }, []);

  useEffect(() => {
    const sub = listen<UpdateProgress>("triex://update-progress", ({ payload }) => {
      setProgress(payload);
    });
    return () => {
      void sub.then((unlisten) => unlisten());
    };
  }, []);

  if (!settings) return null;

  const handleCheckUpdate = async () => {
    setChecking(true);
    setUpToDate(false);
    setUpdateError(null);
    try {
      const meta = await ipc.checkUpdate();
      setAvailableUpdate(meta);
      if (!meta) {
        setUpToDate(true);
        notifyStatus("当前已是最新版本");
      } else {
        notifyStatus(`发现新版本 v${meta.version}`);
      }
    } catch (err) {
      setUpdateError(String(err));
    } finally {
      setChecking(false);
    }
  };

  const handleInstallUpdate = async () => {
    setInstalling(true);
    setUpdateError(null);
    setProgress({ downloaded: 0, contentLength: null });
    try {
      await ipc.installUpdate();
    } catch (err) {
      setUpdateError(String(err));
      setInstalling(false);
    }
  };

  const handleOpenDataDir = async () => {
    try {
      await ipc.openDataDir();
    } catch (err) {
      setUpdateError(String(err));
    }
  };

  const progressPercent =
    progress && progress.contentLength && progress.contentLength > 0
      ? Math.min(100, Math.round((progress.downloaded / progress.contentLength) * 100))
      : null;

  return (
    <ContentSection
      title="关于与更新"
      description="查看 Triex 版本信息、检查在线更新或管理本地用户数据。"
    >
      <div className="max-w-lg space-y-5">
        {/* Version & Manual Check Card */}
        <div className="rounded-xl border border-border/70 bg-surface p-4 space-y-3.5">
          <div className="flex items-center justify-between gap-3">
            <div className="space-y-1">
              <div className="flex items-center gap-2">
                <span className="text-sm font-semibold text-foreground">Triex</span>
                <Badge variant="secondary" className="font-mono text-[11px]">
                  v{appVersion}
                </Badge>
                {availableUpdate && (
                  <Badge className="gap-1 text-[11px]">
                    <Sparkles className="size-3" />
                    新版本 v{availableUpdate.version}
                  </Badge>
                )}
              </div>
              <p className="text-xs text-muted-foreground">
                轻量、快速的 Windows 桌面效率启动器（Tauri v2 + Rust）。
              </p>
            </div>

            <Button
              type="button"
              size="sm"
              variant="outline"
              disabled={checking || installing}
              onClick={() => void handleCheckUpdate()}
              className="h-8 shrink-0 gap-1.5 rounded-lg"
            >
              <RefreshCw className={`size-3.5 ${checking ? "animate-spin" : ""}`} />
              {checking ? "检查中..." : "检查更新"}
            </Button>
          </div>

          {upToDate && !availableUpdate && !updateError && (
            <div className="flex items-center gap-2 rounded-lg bg-emerald-500/10 px-3 py-2 text-xs text-emerald-600 dark:text-emerald-400">
              <CheckCircle2 className="size-3.5 shrink-0" />
              <span>当前已是最新版本 (v{appVersion})。</span>
            </div>
          )}

          {updateError && (
            <div className="rounded-lg border border-destructive/40 bg-destructive/10 px-3 py-2 text-xs text-destructive break-all">
              {updateError}
            </div>
          )}

          {availableUpdate && (
            <div className="space-y-3 border-t border-border/60 pt-3">
              <div className="flex items-center justify-between text-xs text-muted-foreground">
                <span>
                  可从 <span className="font-mono">v{availableUpdate.currentVersion}</span> 升级到{" "}
                  <span className="font-mono font-medium text-foreground">
                    v{availableUpdate.version}
                  </span>
                </span>
                {availableUpdate.date && (
                  <span className="font-mono text-[11px]">
                    {availableUpdate.date.slice(0, 10)}
                  </span>
                )}
              </div>

              {availableUpdate.body && (
                <div className="max-h-36 overflow-y-auto rounded-lg border border-border/60 bg-background/60 p-2.5 text-xs whitespace-pre-wrap text-muted-foreground">
                  {availableUpdate.body}
                </div>
              )}

              {installing && progress && (
                <div className="space-y-1.5">
                  <div className="flex items-center justify-between text-[11px] text-muted-foreground">
                    <span>正在下载更新包...</span>
                    <span className="font-mono">
                      {formatBytes(progress.downloaded)}
                      {progress.contentLength
                        ? ` / ${formatBytes(progress.contentLength)} (${progressPercent ?? 0}%)`
                        : ""}
                    </span>
                  </div>
                  <div className="h-1.5 w-full overflow-hidden rounded-full bg-border/60">
                    <div
                      className="h-full rounded-full bg-primary transition-all duration-150"
                      style={{ width: `${progressPercent ?? 15}%` }}
                    />
                  </div>
                </div>
              )}

              <Button
                type="button"
                size="sm"
                disabled={installing}
                onClick={() => void handleInstallUpdate()}
                className="h-8 gap-1.5 rounded-lg"
              >
                <Download className="size-3.5" />
                {installing ? "正在下载并准备安装..." : "立即下载并重启更新"}
              </Button>
            </div>
          )}
        </div>

        {/* Auto-check toggle */}
        <div className="flex items-center justify-between rounded-xl border border-border/70 bg-surface px-4 py-3">
          <div className="space-y-0.5 pr-4">
            <Label htmlFor="auto-check-update" className="text-[13px] font-medium">
              启动时自动静默检查更新
            </Label>
            <p className="text-xs text-muted-foreground">
              启动后在后台检查新版本，仅在此页面显示提示，绝不打断搜索操作。
            </p>
          </div>
          <Switch
            id="auto-check-update"
            checked={settings.autoCheckUpdate}
            onCheckedChange={(checked) =>
              void patchSettings({ autoCheckUpdate: checked })
            }
          />
        </div>

        {/* Local data directory */}
        <div className="flex items-center justify-between rounded-xl border border-border/70 bg-surface px-4 py-3">
          <div className="space-y-0.5 pr-4">
            <div className="text-[13px] font-medium text-foreground">本地数据目录</div>
            <p className="text-xs text-muted-foreground">
              数据库、图标缓存与已安装插件均保存在用户数据目录，升级或重装不会丢失。
            </p>
          </div>
          <Button
            type="button"
            size="sm"
            variant="outline"
            onClick={() => void handleOpenDataDir()}
            className="h-8 shrink-0 gap-1.5 rounded-lg"
          >
            <FolderOpen className="size-3.5" />
            打开目录
          </Button>
        </div>
      </div>
    </ContentSection>
  );
}
