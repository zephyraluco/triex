import { useEffect, type RefObject } from "react";
import { listen } from "@tauri-apps/api/event";
import { ipc, type ActivePluginSession } from "@/lib/ipc";
import { useLauncherStore } from "@/stores/useLauncherStore";

/**
 * Handles launcher global keyboard interactions (Esc hierarchy, Ctrl+K action menu,
 * Alt+Enter admin launch) and Tauri lifecycle events.
 */
export function useLauncherShortcuts(inputRef: RefObject<HTMLInputElement | null>) {
  const setActivePlugin = useLauncherStore((s) => s.setActivePlugin);
  const setSubInputPlaceholder = useLauncherStore((s) => s.setSubInputPlaceholder);
  const setClipboard = useLauncherStore((s) => s.setClipboard);
  const resetSearch = useLauncherStore((s) => s.resetSearch);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const state = useLauncherStore.getState();

      if (event.key === "Escape") {
        event.preventDefault();
        if (state.actionMenuOpen) {
          state.setActionMenuOpen(false);
          return;
        }
        if (state.activePlugin) {
          void ipc.closePlugin();
          state.setActivePlugin(null);
          return;
        }
        if (state.query.length > 0) {
          state.setQuery("");
          return;
        }
        void ipc.hideWindow();
        return;
      }

      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
        if (!state.activePlugin && state.selectedId) {
          event.preventDefault();
          state.setActionMenuOpen(!state.actionMenuOpen);
        }
        return;
      }

      if (event.altKey && event.key === "Enter") {
        if (!state.activePlugin && state.selectedId) {
          event.preventDefault();
          void ipc.launchItem(state.selectedId, true);
        }
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  useEffect(() => {
    const unlisteners = [
      listen("triex://launcher-shown", () => {
        resetSearch();
        requestAnimationFrame(() => {
          inputRef.current?.focus();
          inputRef.current?.select();
        });
        void ipc
          .getClipboardSnapshot()
          .then((snap) => setClipboard(snap))
          .catch(() => setClipboard(null));
      }),
      listen<ActivePluginSession>("triex://plugin-opened", (e) => {
        setActivePlugin(e.payload);
        requestAnimationFrame(() => inputRef.current?.focus());
      }),
      listen("triex://plugin-closed", () => {
        setActivePlugin(null);
        requestAnimationFrame(() => inputRef.current?.focus());
      }),
      listen<string>("triex://sub-input-placeholder", (e) => {
        setSubInputPlaceholder(e.payload);
      }),
    ];

    return () => {
      for (const p of unlisteners) {
        void p.then((unlisten) => unlisten());
      }
    };
  }, [inputRef, resetSearch, setActivePlugin, setClipboard, setSubInputPlaceholder]);
}
