import { useEffect } from "react";
import { ipc } from "@/lib/ipc";
import { useLauncherStore } from "@/stores/useLauncherStore";

/**
 * Handles ZTools-style keyboard shortcuts on the main launcher shell:
 * layered Escape exit, `Ctrl+M` dashboard toggle, `Ctrl+K` item context menu,
 * and `Alt+Enter` elevated administrator launch.
 */
export function useLauncherShortcuts() {
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const state = useLauncherStore.getState();

      if (event.key === "Tab" && !event.ctrlKey && !event.altKey && !event.metaKey) {
        event.preventDefault();
        if (state.activePlugin) {
          void ipc.forwardSubInputKeydown("Tab", event.shiftKey);
        }
        return;
      }

      if (event.key === "Escape") {
        event.preventDefault();
        if (state.settingsOpen) {
          void ipc.closeSettings();
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
  }, []);
}
