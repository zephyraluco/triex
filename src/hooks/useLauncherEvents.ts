import { useEffect, type RefObject } from "react";
import { listen } from "@tauri-apps/api/event";
import { ipc, type ActivePluginSession, type AppSettings } from "@/lib/ipc";
import { useLauncherStore } from "@/stores/useLauncherStore";

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
  const setViewLoading = useLauncherStore((s) => s.setViewLoading);
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
          const response = await ipc.searchItems("", Date.now());
          setSearchResult(response.items, response.totalIndexed, "");
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
      listen<boolean>("triex://settings-opened", (e) => {
        setSettingsOpen(true, Boolean(e.payload));
      }),
      listen("triex://settings-closed", () => {
        setSettingsOpen(false, false);
        requestAnimationFrame(() => inputRef.current?.focus());
      }),
      listen("triex://toggle-dashboard", () => {
        toggleDashboard();
      }),
      listen<AppSettings>("triex://settings-updated", (e) => {
        setPinnedItems(e.payload.pinnedItems ?? []);
      }),
      listen<ActivePluginSession>("triex://plugin-opened", (e) => {
        setActivePlugin(e.payload, Boolean(e.payload.loading));
        requestAnimationFrame(() => inputRef.current?.focus());
      }),
      listen("triex://plugin-closed", () => {
        setActivePlugin(null, false);
        requestAnimationFrame(() => inputRef.current?.focus());
      }),
      listen<string>("triex://view-loaded", (e) => {
        const state = useLauncherStore.getState();
        if (
          (e.payload === "settings" && state.settingsOpen) ||
          (state.activePlugin && state.activePlugin.pluginId === e.payload)
        ) {
          setViewLoading(false);
        }
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
    setViewLoading,
    toggleDashboard,
  ]);
}
