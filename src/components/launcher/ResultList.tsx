import { useMemo } from "react";
import { CommandGroup, CommandList } from "@/components/ui/command";
import { ResultItem } from "@/components/launcher/ResultItem";
import { EmptyState } from "@/components/launcher/EmptyState";
import { ipc, type SearchResultItem } from "@/lib/ipc";
import { useLauncherStore } from "@/stores/useLauncherStore";

export function ResultList() {
  const results = useLauncherStore((s) => s.results);
  const query = useLauncherStore((s) => s.query);
  const setStatusMessage = useLauncherStore((s) => s.setStatusMessage);

  const handleSelect = async (item: SearchResultItem) => {
    try {
      await ipc.launchItem(item.id, false);
    } catch (err) {
      setStatusMessage(`启动失败: ${String(err)}`);
    }
  };

  const grouped = useMemo(() => {
    // When searching, keep strict Rust relevance ranking in a single unified group
    // so the highest-scored item is always first regardless of kind.
    if (query.trim().length > 0) {
      return [{ heading: "最佳匹配", items: results }];
    }

    const apps = results.filter((r) => r.kind === "app" || r.kind === "uwpApp");
    const commands = results.filter((r) => r.kind === "systemCmd");
    const plugins = results.filter((r) => r.kind === "plugin");

    const sections: Array<{ heading: string; items: SearchResultItem[] }> = [];
    if (apps.length) sections.push({ heading: "常用应用", items: apps });
    if (plugins.length) sections.push({ heading: "插件", items: plugins });
    if (commands.length) sections.push({ heading: "系统指令", items: commands });
    return sections;
  }, [query, results]);

  return (
    <CommandList>
      {results.length === 0 ? (
        <EmptyState />
      ) : (
        grouped.map((section) => (
          <CommandGroup key={section.heading} heading={section.heading}>
            {section.items.map((item) => (
              <ResultItem key={item.id} item={item} onSelect={handleSelect} />
            ))}
          </CommandGroup>
        ))
      )}
    </CommandList>
  );
}
