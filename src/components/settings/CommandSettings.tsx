import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ipc, type SearchResultItem } from "@/lib/ipc";
import { ContentSection, notifyStatus } from "./SettingsPrimitives";

export function CommandSettings() {
  const [allItems, setAllItems] = useState<SearchResultItem[]>([]);
  const [commandFilter, setCommandFilter] = useState("");

  useEffect(() => {
    let cancelled = false;
    void ipc
      .searchItems(commandFilter, Date.now(), 200)
      .then((res) => { if (!cancelled) setAllItems(res.items); })
      .catch(() => {});
    return () => { cancelled = true; };
  }, [commandFilter]);

  return (<ContentSection
              title={`所有指令 (${allItems.length})`}
              description="查看并快速测试当前内存索引中的全部应用、系统指令与插件。"
              action={
                <Input
                  placeholder="筛选指令..."
                  value={commandFilter}
                  onChange={(e) => setCommandFilter(e.target.value)}
                  className="h-8 w-44 rounded-lg border-border/80 bg-surface text-xs"
                />
              }
            >
              <div className="overflow-hidden rounded-xl border border-border/70 bg-surface divide-y divide-border/60">
                {allItems.map((item) => (
                  <div
                    key={item.id}
                    className="flex items-center justify-between gap-3 px-3.5 py-2.5 transition-colors hover:bg-surface-hover"
                  >
                    <div className="min-w-0 flex-1">
                      <div className="truncate text-[13px] font-medium text-foreground">
                        {item.name}
                      </div>
                      <div className="truncate text-xs text-muted-foreground">
                        {item.subtitle || item.execPath}
                      </div>
                    </div>
                    <Button
                      size="sm"
                      variant="outline"
                      className="h-7 shrink-0 rounded-lg border-border/80 bg-surface text-xs hover:bg-surface-active"
                      onClick={() =>
                        void ipc.launchItem(item.id, false).catch((err) => {
                          notifyStatus(`启动失败: ${String(err)}`);
                        })
                      }
                    >
                      启动
                    </Button>
                  </div>
                ))}
              </div>
            </ContentSection>);
}
