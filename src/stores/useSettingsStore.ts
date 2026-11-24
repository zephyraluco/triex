import { create } from "zustand";
import {
  ipc,
  type AppSettings,
  type InstalledPluginRecord,
} from "@/lib/ipc";

interface SettingsState {
  settings: AppSettings | null;
  plugins: InstalledPluginRecord[];
  isLoading: boolean;
  isSaving: boolean;
  error: string | null;

  loadAll: () => Promise<void>;
  patchSettings: (patch: Partial<AppSettings>) => Promise<void>;
  togglePlugin: (pluginId: string, enabled: boolean) => Promise<void>;
}

export const useSettingsStore = create<SettingsState>((set, get) => ({
  settings: null,
  plugins: [],
  isLoading: false,
  isSaving: false,
  error: null,

  loadAll: async () => {
    set({ isLoading: true, error: null });
    try {
      const [settings, plugins] = await Promise.all([
        ipc.getSettings(),
        ipc.listPlugins(),
      ]);
      set({ settings, plugins, isLoading: false });
    } catch (err) {
      set({ error: String(err), isLoading: false });
    }
  },

  patchSettings: async (patch) => {
    const current = get().settings;
    if (!current) return;
    const next: AppSettings = { ...current, ...patch };
    set({ isSaving: true, error: null });
    try {
      const saved = await ipc.updateSettings(next);
      set({ settings: saved, isSaving: false });
    } catch (err) {
      set({ error: String(err), isSaving: false });
    }
  },

  togglePlugin: async (pluginId, enabled) => {
    try {
      await ipc.setPluginEnabled(pluginId, enabled);
      set((state) => ({
        plugins: state.plugins.map((p) =>
          p.id === pluginId ? { ...p, enabled } : p,
        ),
      }));
    } catch (err) {
      set({ error: String(err) });
    }
  },
}));
