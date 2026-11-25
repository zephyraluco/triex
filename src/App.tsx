import { useRef } from "react";
import { Command } from "@/components/ui/command";
import { SearchHeader } from "@/components/launcher/SearchHeader";
import { ResultList } from "@/components/launcher/ResultList";
import { useLauncherEvents } from "@/hooks/useLauncherEvents";
import { useSearch } from "@/hooks/useSearch";
import { useWindowAutoResize } from "@/hooks/useWindowAutoResize";
import { useLauncherShortcuts } from "@/hooks/useLauncherShortcuts";
import { useNativeContextMenu } from "@/hooks/useNativeContextMenu";
import { useThemeSync } from "@/hooks/useThemeSync";
import { useLauncherStore } from "@/stores/useLauncherStore";

export default function App() {
  const containerRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  const selectedId = useLauncherStore((s) => s.selectedId);
  const setSelectedId = useLauncherStore((s) => s.setSelectedId);
  const activePlugin = useLauncherStore((s) => s.activePlugin);
  const settingsOpen = useLauncherStore((s) => s.settingsOpen);

  useThemeSync();
  useNativeContextMenu();
  useSearch();
  useWindowAutoResize(containerRef);
  useLauncherShortcuts();
  useLauncherEvents(inputRef);

  return (
    <div
      ref={containerRef}
      className="flex w-full flex-col overflow-clip text-foreground"
    >
      <Command
        shouldFilter={false}
        disablePointerSelection
        loop
        value={selectedId}
        onValueChange={setSelectedId}
        className="h-auto overflow-clip rounded-none bg-transparent text-foreground"
      >
        <SearchHeader inputRef={inputRef} />

        {!settingsOpen && !activePlugin && <ResultList />}
      </Command>
    </div>
  );
}
