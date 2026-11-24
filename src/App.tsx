import { useRef } from "react";
import { Command } from "@/components/ui/command";
import { TooltipProvider } from "@/components/ui/tooltip";
import { SearchHeader } from "@/components/launcher/SearchHeader";
import { ResultList } from "@/components/launcher/ResultList";
import { EmbeddedSettings } from "@/components/launcher/EmbeddedSettings";
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
  const settingsOpen = useLauncherStore((s) => s.settingsOpen);

  useThemeSync();
  useSearch();
  useWindowAutoResize(containerRef);
  useLauncherShortcuts(inputRef);

  return (
    <TooltipProvider delayDuration={350}>
      <div
        ref={containerRef}
        className="flex w-full flex-col overflow-clip text-foreground"
      >
        <Command
          shouldFilter={false}
          loop
          value={selectedId}
          onValueChange={setSelectedId}
          className="h-auto overflow-clip rounded-none bg-transparent text-foreground"
        >
          <SearchHeader inputRef={inputRef} />

          {settingsOpen ? (
            <EmbeddedSettings />
          ) : (
            !activePlugin && <ResultList />
          )}
        </Command>
      </div>
    </TooltipProvider>
  );
}
