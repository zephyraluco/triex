import { useCallback, useEffect, useMemo, useRef, type MouseEvent } from "react";
import { ChevronDown, ChevronUp } from "lucide-react";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { ResultItem } from "@/components/launcher/ResultItem";
import { ipc, type SearchResultItem } from "@/lib/ipc";
import { groupLauncherSections } from "@/lib/launcherSections";
import { useLauncherStore } from "@/stores/useLauncherStore";

/** Number of icon cards per row in the ZTools-style launcher grid. */
const GRID_COLUMNS = 9;
/** Default visible rows per section before showing that section's own expand toggle. */
const COLLAPSED_ITEM_LIMIT = GRID_COLUMNS * 2;

interface GridSection {
  key: string;
  heading: string;
  allItems: SearchResultItem[];
  visibleItems: SearchResultItem[];
  canExpand: boolean;
  isExpanded: boolean;
}

interface GridCoord {
  item: SearchResultItem;
  row: number;
  col: number;
}

export function ResultList() {
  const listRef = useRef<HTMLDivElement>(null);
  const gridNavActiveRef = useRef(false);
  const results = useLauncherStore((s) => s.results);
  const query = useLauncherStore((s) => s.query);
  const resultQuery = useLauncherStore((s) => s.resultQuery);
  const selectedId = useLauncherStore((s) => s.selectedId);
  const pinnedItems = useLauncherStore((s) => s.pinnedItems);
  const showDashboard = useLauncherStore((s) => s.showDashboard);
  const expandedSections = useLauncherStore((s) => s.expandedSections);
  const toggleSectionExpanded = useLauncherStore((s) => s.toggleSectionExpanded);
  const setSelectedId = useLauncherStore((s) => s.setSelectedId);
  const setStatusMessage = useLauncherStore((s) => s.setStatusMessage);

  const handleSelect = useCallback(
    async (item: SearchResultItem) => {
      try {
        await ipc.launchItem(item.id, false);
      } catch (err) {
        setStatusMessage(`启动失败: ${String(err)}`);
        window.setTimeout(() => setStatusMessage(null), 2500);
      }
    },
    [setStatusMessage],
  );

  const handleContextMenu = useCallback(
    (item: SearchResultItem, event: MouseEvent<HTMLDivElement>) => {
      event.preventDefault();
      event.stopPropagation();
      setSelectedId(item.id);
      void ipc.showItemContextMenu(item.id);
    },
    [setSelectedId],
  );

  const buildSection = useCallback(
    (key: string, heading: string, items: SearchResultItem[]): GridSection => {
      const canExpand = items.length > COLLAPSED_ITEM_LIMIT;
      const isExpanded = Boolean(expandedSections[key]);
      const visibleItems =
        canExpand && !isExpanded ? items.slice(0, COLLAPSED_ITEM_LIMIT) : items;
      return {
        key,
        heading,
        allItems: items,
        visibleItems,
        canExpand,
        isExpanded,
      };
    },
    [expandedSections],
  );

  const sections = useMemo<GridSection[]>(
    () =>
      groupLauncherSections(results, resultQuery, pinnedItems, showDashboard).map(
        (group) => buildSection(group.key, group.heading, group.items),
      ),
    [results, resultQuery, pinnedItems, showDashboard, buildSection],
  );

  const { flatCoords, rows } = useMemo(() => {
    const coords: GridCoord[] = [];
    const rowMatrix: GridCoord[][] = [];
    let currentRow = 0;

    for (const section of sections) {
      const sectionRowCount = Math.ceil(section.visibleItems.length / GRID_COLUMNS);
      for (let localIdx = 0; localIdx < section.visibleItems.length; localIdx++) {
        const rowOffset = Math.floor(localIdx / GRID_COLUMNS);
        const col = localIdx % GRID_COLUMNS;
        const globalRow = currentRow + rowOffset;
        const coord: GridCoord = {
          item: section.visibleItems[localIdx],
          row: globalRow,
          col,
        };
        coords.push(coord);
        if (!rowMatrix[globalRow]) {
          rowMatrix[globalRow] = [];
        }
        rowMatrix[globalRow].push(coord);
      }
      currentRow += sectionRowCount;
    }

    return { flatCoords: coords, rows: rowMatrix };
  }, [sections]);

  // Keep selectedId aligned with currently visible items (e.g. when a section collapses or recent clears).
  useEffect(() => {
    if (flatCoords.length === 0) {
      if (selectedId !== "") {
        setSelectedId("");
      }
      return;
    }
    const exists = flatCoords.some((c) => c.item.id === selectedId);
    if (!exists) {
      setSelectedId(flatCoords[0].item.id);
    }
  }, [flatCoords, selectedId, setSelectedId]);

  // Scroll ONLY within the inner CommandList container rather than calling element.scrollIntoView,
  // which can drift outer ancestors during dynamic window resizing.
  useEffect(() => {
    if (!selectedId || !listRef.current) return;
    const listEl = listRef.current;
    const escaped = CSS.escape(selectedId);
    const itemEl = listEl.querySelector(`[data-item-id="${escaped}"]`);
    if (!(itemEl instanceof HTMLElement)) return;

    const listRect = listEl.getBoundingClientRect();
    const itemRect = itemEl.getBoundingClientRect();
    if (itemRect.top < listRect.top) {
      listEl.scrollTop -= listRect.top - itemRect.top + 8;
    } else if (itemRect.bottom > listRect.bottom) {
      listEl.scrollTop += itemRect.bottom - listRect.bottom + 8;
    }
  }, [selectedId]);

  // Return ArrowLeft/ArrowRight to text editing whenever the user types or clicks the input.
  useEffect(() => {
    gridNavActiveRef.current = false;
  }, [query]);

  useEffect(() => {
    const onPointerDown = (event: PointerEvent) => {
      if (event.target instanceof HTMLInputElement) {
        gridNavActiveRef.current = false;
      }
    };
    window.addEventListener("pointerdown", onPointerDown, true);
    return () => window.removeEventListener("pointerdown", onPointerDown, true);
  }, []);

  useEffect(() => {
    if (flatCoords.length === 0) return;

    const handleGridKeyDown = (event: KeyboardEvent) => {
      const state = useLauncherStore.getState();
      if (state.activePlugin || state.settingsOpen) return;

      const idx = flatCoords.findIndex((c) => c.item.id === state.selectedId);
      const currentIndex = idx === -1 ? 0 : idx;
      const current = flatCoords[currentIndex];

      if (event.key === "Tab" && !event.ctrlKey && !event.altKey && !event.metaKey) {
        event.preventDefault();
        event.stopPropagation();
        gridNavActiveRef.current = true;
        const nextIndex = event.shiftKey
          ? (currentIndex - 1 + flatCoords.length) % flatCoords.length
          : (currentIndex + 1) % flatCoords.length;
        setSelectedId(flatCoords[nextIndex].item.id);
      } else if (event.key === "ArrowRight") {
        const activeEl = document.activeElement;
        if (
          !gridNavActiveRef.current &&
          activeEl instanceof HTMLInputElement &&
          activeEl.selectionStart !== null &&
          activeEl.selectionStart < activeEl.value.length
        ) {
          return;
        }
        event.preventDefault();
        event.stopPropagation();
        gridNavActiveRef.current = true;
        const next = (currentIndex + 1) % flatCoords.length;
        setSelectedId(flatCoords[next].item.id);
      } else if (event.key === "ArrowLeft") {
        const activeEl = document.activeElement;
        if (
          !gridNavActiveRef.current &&
          activeEl instanceof HTMLInputElement &&
          activeEl.selectionStart !== null &&
          activeEl.selectionStart > 0
        ) {
          return;
        }
        event.preventDefault();
        event.stopPropagation();
        gridNavActiveRef.current = true;
        const prev = (currentIndex - 1 + flatCoords.length) % flatCoords.length;
        setSelectedId(flatCoords[prev].item.id);
      } else if (event.key === "ArrowDown") {
        event.preventDefault();
        event.stopPropagation();
        gridNavActiveRef.current = true;
        const targetRow = rows[current.row + 1];
        if (targetRow && targetRow.length > 0) {
          const targetCol = Math.min(current.col, targetRow.length - 1);
          setSelectedId(targetRow[targetCol].item.id);
        }
      } else if (event.key === "ArrowUp") {
        event.preventDefault();
        event.stopPropagation();
        gridNavActiveRef.current = true;
        if (current.row > 0) {
          const targetRow = rows[current.row - 1];
          if (targetRow && targetRow.length > 0) {
            const targetCol = Math.min(current.col, targetRow.length - 1);
            setSelectedId(targetRow[targetCol].item.id);
          }
        }
      } else if (event.key === "Enter" && !event.altKey && !event.isComposing) {
        event.preventDefault();
        event.stopPropagation();
        void handleSelect(current.item);
      }
    };

    window.addEventListener("keydown", handleGridKeyDown, true);
    return () => window.removeEventListener("keydown", handleGridKeyDown, true);
  }, [flatCoords, rows, setSelectedId, handleSelect]);

  // Collapse cleanly to the 64px search bar whenever there are no items to show
  // (both on empty home with no history/pins and when a search query matches nothing).
  if (sections.length === 0) {
    return null;
  }

  return (
    <ScrollArea
      className="overflow-hidden"
      viewportRef={listRef}
      viewportClassName="h-auto max-h-[468px] w-full px-3 pb-3"
    >
      <div>
        {sections.map((section) => (
          <div key={section.key} className="py-1">
            <div className="flex items-center justify-between px-1.5 pb-2 pt-1">
              <span className="text-xs font-semibold tracking-wide text-muted-foreground">
                {section.heading}
              </span>
              {section.canExpand && (
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  onClick={() => toggleSectionExpanded(section.key)}
                  className="h-6 gap-0.5 rounded-lg px-2 text-[11px] text-muted-foreground hover:bg-surface-hover hover:text-foreground"
                >
                  <span>
                    {section.isExpanded
                      ? "收起"
                      : `展开全部 (${section.allItems.length})`}
                  </span>
                  {section.isExpanded ? (
                    <ChevronUp className="size-3" />
                  ) : (
                    <ChevronDown className="size-3" />
                  )}
                </Button>
              )}
            </div>

            <div className="grid grid-cols-9 gap-1">
              {section.visibleItems.map((item) => (
                <ResultItem
                  key={item.id}
                  item={item}
                  isSelected={item.id === selectedId}
                  onSelect={handleSelect}
                  onContextMenu={handleContextMenu}
                />
              ))}
            </div>
          </div>
        ))}
      </div>
    </ScrollArea>
  );
}
