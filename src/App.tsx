import { useRef } from "react";
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
      <SearchHeader inputRef={inputRef} />
      {!settingsOpen && !activePlugin && <ResultList />}
    </div>
  );
}
