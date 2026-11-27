import type { SearchResultItem } from "@/lib/ipc";

export interface RawLauncherSection {
  key: string;
  heading: string;
  items: SearchResultItem[];
}

/**
 * Single source of truth for grouping launcher items into display sections
 * (shared by `ResultList` rendering and `useLauncherStore` default selection).
 */
export function groupLauncherSections(
  results: SearchResultItem[],
  resultQuery: string,
  pinnedItems: string[],
  showDashboard: boolean,
): RawLauncherSection[] {
  if (resultQuery.trim().length > 0) {
    return results.length > 0
      ? [{ key: "best", heading: "最佳匹配", items: results }]
      : [];
  }

  const pinnedSet = new Set(pinnedItems);
  const byId = new Map(results.map((r) => [r.id, r]));
  const pinned = pinnedItems
    .map((id) => byId.get(id))
    .filter((item): item is SearchResultItem => item !== undefined);

  // "最近使用" strictly contains items with actual launch history (frecency > 0).
  const recent = results.filter((r) => !pinnedSet.has(r.id) && r.score > 0);

  const groups: RawLauncherSection[] = [];
  if (pinned.length > 0) {
    groups.push({ key: "pinned", heading: "已固定", items: pinned });
  }
  if (recent.length > 0) {
    groups.push({ key: "recent", heading: "最近使用", items: recent });
  }

  if (showDashboard) {
    const shownIds = new Set(groups.flatMap((g) => g.items.map((i) => i.id)));
    const restApps = results.filter(
      (r) => !shownIds.has(r.id) && (r.kind === "app" || r.kind === "uwpApp"),
    );
    const restPlugins = results.filter(
      (r) => !shownIds.has(r.id) && r.kind === "plugin",
    );
    const restCommands = results.filter(
      (r) => !shownIds.has(r.id) && r.kind === "systemCmd",
    );

    if (restApps.length > 0) {
      groups.push({ key: "apps", heading: "应用程序", items: restApps });
    }
    if (restPlugins.length > 0) {
      groups.push({ key: "plugins", heading: "插件工具", items: restPlugins });
    }
    if (restCommands.length > 0) {
      groups.push({ key: "commands", heading: "系统指令", items: restCommands });
    }
  }

  return groups;
}

/**
 * Resolves the ID of the first visible launcher item using the exact same
 * section ordering rendered in `ResultList`.
 */
export function resolveFirstVisibleItemId(
  results: SearchResultItem[],
  resultQuery: string,
  pinnedItems: string[],
  showDashboard: boolean,
): string {
  const sections = groupLauncherSections(results, resultQuery, pinnedItems, showDashboard);
  return sections[0]?.items[0]?.id ?? "";
}
