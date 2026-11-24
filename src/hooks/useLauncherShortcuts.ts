import { useEffect, type RefObject } from "react";
import { listen } from "@tauri-apps/api/event";
import { ipc, type ActivePluginSession, type AppSettings } from "@/lib/ipc";
import { useLauncherStore } from "@/stores/useLauncherStore";

const HOME_RESET_FETCH_LIMIT = 100;

/**
 * Handles ZTools-style keyboard interactions (Esc hierarchy, Ctrl+M dashboard toggle,
 * Ctrl+K action menu, Alt+Enter admin launch) and Tauri lifecycle events.
 */
export function useLauncherShortcuts(inputRef: RefObject<HTMLInputElement | null>) {
  const setActivePlugin = useLauncherStore((s) => s.setActivePlugin);
  const setSubInputPlaceholder = useLauncherStore((s) => s.setSubInputPlaceholder);
  const setClipboard = useLauncherStore((s) => s.setClipboard);
  const setPinnedItems = useLauncherStore((s) => s.setPinnedItems);
  const setStatusMessage = useLauncherStore((s) => s.setStatusMessage);
  const setSearchResult = useLauncherStore((s) => s.setSearchResult);
  const openSettings = useLauncherStore((s) => s.openSettings);
  const toggleSettings = useLauncherStore((s) => s.toggleSettings);
  const toggleDashboard = useLauncherStore((s) => s.toggleDashboard);
  const resetSearch = useLauncherStore((s) => s.resetSearch);

  useEffect(() => {
    void ipc
      .getSettings()
      .then((settings) => setPinnedItems(settings.pinnedItems ?? []))
      .catch(() => {});
  }, [setPinnedItems]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const state = useLauncherStore.getState();

      if (event.key === "Escape") {
        event.preventDefault();
        if (state.actionMenuOpen) {
          state.closeActionMenu();
          return;
        }
        if (state.settingsOpen) {
          state.closeSettings();
          requestAnimationFrame(() => inputRef.current?.focus());
          return;
        }
        if (state.activePlugin) {
          void ipc.closePlugin();
          state.setActivePlugin(null);
          return;
        }
        if (state.attachment) {
          state.setAttachment(null);
          return;
        }
        if (state.query.length > 0) {
          state.setQuery("");
          return;
        }
        if (state.showDashboard) {
          state.setShowDashboard(false);
          return;
        }
        void ipc.hideWindow();
        return;
      }

      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "m") {
        if (!state.activePlugin && !state.settingsOpen) {
          event.preventDefault();
          state.toggleDashboard();
        }
        return;
      }

      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
        if (!state.activePlugin && !state.settingsOpen && state.selectedId) {
          event.preventDefault();
          void ipc.showItemContextMenu(state.selectedId);
        }
        return;
      }

      if (event.altKey && event.key === "Enter") {
        if (!state.activePlugin && !state.settingsOpen && state.selectedId) {
          event.preventDefault();
          void ipc.launchItem(state.selectedId, true);
        }
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [inputRef]);

  useEffect(() => {
    const unlisteners = [
      listen<number>("triex://launcher-hidden", async (event) => {
        const epoch = event.payload;
        resetSearch();
        setStatusMessage(null);
        try {
          const response = await ipc.searchItems("", Date.now(), HOME_RESET_FETCH_LIMIT);
          setSearchResult(response.items, response.totalIndexed);
        } catch {
          // Ignore errors during background pre-reset.
        }
        requestAnimationFrame(() => {
          requestAnimationFrame(() => {
            requestAnimationFrame(() => {
              void ipc.completeHide(epoch);
            });
          });
        });
      }),
      listen("triex://launcher-shown", () => {
        const state = useLauncherStore.getState();
        if (
          !state.settingsOpen &&
          (state.query.length > 0 || state.attachment || state.showDashboard)
        ) {
          resetSearch();
        }
        requestAnimationFrame(() => {
          inputRef.current?.focus();
          inputRef.current?.select();
        });
        void ipc
          .getClipboardSnapshot()
          .then((snap) => setClipboard(snap))
          .catch(() => setClipboard(null));
      }),
      listen("triex://open-settings", () => {
        openSettings();
      }),
      listen("triex://toggle-settings", () => {
        toggleSettings();
      }),
      listen("triex://toggle-dashboard", () => {
        const state = useLauncherStore.getState();
        if (state.settingsOpen) {
          state.closeSettings();
        }
        toggleDashboard();
      }),
      listen<AppSettings>("triex://settings-updated", (e) => {
        setPinnedItems(e.payload.pinnedItems ?? []);
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
      listen<string>("triex://status-message", (e) => {
        setStatusMessage(e.payload);
        window.setTimeout(() => {
          if (useLauncherStore.getState().statusMessage === e.payload) {
            setStatusMessage(null);
          }
        }, 1800);
      }),
    ];

    return () => {
      for (const p of unlisteners) {
        void p.then((unlisten) => unlisten());
      }
    };
  }, [
    inputRef,
    openSettings,
    resetSearch,
    setActivePlugin,
    setClipboard,
    setPinnedItems,
    setSearchResult,
    setStatusMessage,
    setSubInputPlaceholder,
    toggleDashboard,
    toggleSettings,
  ]);
}
