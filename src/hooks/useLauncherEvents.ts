import { useEffect, type RefObject } from "react";
import { listen } from "@tauri-apps/api/event";
import { ipc, type ActivePluginSession, type AppSettings } from "@/lib/ipc";
import { useLauncherStore } from "@/stores/useLauncherStore";

const HOME_RESET_FETCH_LIMIT = 100;

/**
 * Handles ZTools-style keyboard interactions (Esc hierarchy, Ctrl+M dashboard toggle,
 * Ctrl+K action menu, Alt+Enter admin launch) and Tauri lifecycle events.
 */
export function useLauncherEvents(inputRef: RefObject<HTMLInputElement | null>) {
  const setActivePlugin = useLauncherStore((s) => s.setActivePlugin);
  const setSubInputPlaceholder = useLauncherStore((s) => s.setSubInputPlaceholder);
  const setPinnedItems = useLauncherStore((s) => s.setPinnedItems);
  const setStatusMessage = useLauncherStore((s) => s.setStatusMessage);
  const setSearchResult = useLauncherStore((s) => s.setSearchResult);
  const setSettingsOpen = useLauncherStore((s) => s.setSettingsOpen);
  const toggleDashboard = useLauncherStore((s) => s.toggleDashboard);
  const resetSearch = useLauncherStore((s) => s.resetSearch);

  useEffect(() => {
    void ipc
      .getSettings()
      .then((settings) => setPinnedItems(settings.pinnedItems ?? []))
      .catch(() => {});
  }, [setPinnedItems]);


  useEffect(() => {
    const unlisteners = [
      listen<number>("triex://launcher-hidden", async (event) => {
        const epoch = event.payload;
        const isSettingsOpen = useLauncherStore.getState().settingsOpen;
        resetSearch();
        setStatusMessage(null);
        if (isSettingsOpen) {
          requestAnimationFrame(() => {
            void ipc.completeHide(epoch);
          });
          return;
        }
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
      }),
      listen("triex://settings-opened", () => {
        setSettingsOpen(true);
      }),
      listen("triex://settings-closed", () => {
        setSettingsOpen(false);
        requestAnimationFrame(() => inputRef.current?.focus());
      }),
      listen("triex://toggle-dashboard", () => {
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
    resetSearch,
    setActivePlugin,
    setPinnedItems,
    setSearchResult,
    setSettingsOpen,
    setStatusMessage,
    setSubInputPlaceholder,
    toggleDashboard,
  ]);
}
