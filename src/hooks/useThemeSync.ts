import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { ipc, type AppSettings, type ThemeMode } from "@/lib/ipc";

function applyThemeClass(mode: ThemeMode) {
  const root = document.documentElement;
  const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
  const shouldBeDark = mode === "dark" || (mode === "system" && prefersDark);
  root.classList.toggle("dark", shouldBeDark);
}

/**
 * Keeps the Tailwind `.dark` class synchronized with persisted Triex settings
 * and the Windows OS dark mode preference.
 */
export function useThemeSync() {
  useEffect(() => {
    let currentTheme: ThemeMode = "system";

    void ipc
      .getSettings()
      .then((settings) => {
        currentTheme = settings.theme;
        applyThemeClass(currentTheme);
      })
      .catch(() => applyThemeClass("system"));

    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const onMediaChange = () => {
      if (currentTheme === "system") {
        applyThemeClass("system");
      }
    };
    media.addEventListener("change", onMediaChange);

    const unlistenPromise = listen<AppSettings>("triex://settings-updated", (event) => {
      currentTheme = event.payload.theme;
      applyThemeClass(currentTheme);
    });

    return () => {
      media.removeEventListener("change", onMediaChange);
      void unlistenPromise.then((unlisten) => unlisten());
    };
  }, []);
}
