import { useRef } from "react";
import { Command } from "@/components/ui/command";
import { SearchHeader } from "@/components/launcher/SearchHeader";
import { ResultList } from "@/components/launcher/ResultList";
import { ActionMenu } from "@/components/launcher/ActionMenu";
import { useSearch } from "@/hooks/useSearch";
import { useWindowAutoResize } from "@/hooks/useWindowAutoResize";
import { useLauncherShortcuts } from "@/hooks/useLauncherShortcuts";
import { useThemeSync } from "@/hooks/useThemeSync";
import { useLauncherStore } from "@/stores/useLauncherStore";

export default function App() {
  const containerRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  const selectedId = useLauncherStore((s) => s.selectedId);
  const setSelectedId = useLauncherStore((s) => s.setSelectedId);
  const activePlugin = useLauncherStore((s) => s.activePlugin);

  useThemeSync();
  useSearch();
  useWindowAutoResize(containerRef);
  useLauncherShortcuts(inputRef);

  return (
    <div
      ref={containerRef}
      className="flex w-full flex-col overflow-hidden rounded-xl border border-border/80 bg-background/85 text-foreground shadow-2xl backdrop-blur-xl"
    >
      <Command
        shouldFilter={false}
        loop
        value={selectedId}
        onValueChange={setSelectedId}
      >
        <SearchHeader inputRef={inputRef} />

        {!activePlugin && (
          <>
            <ResultList />
            <ActionMenu />
          </>
        )}
      </Command>
    </div>
  );
}
