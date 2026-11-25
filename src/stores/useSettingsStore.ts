import { create } from "zustand";
import {
  ipc,
  type AppSettings,
  type InstalledPluginRecord,
} from "@/lib/ipc";

export type SettingsNavKey =
  | "general"
  | "appearance"
  | "index"
  | "plugins"
  | "commands";

interface SettingsState {
  activeTab: SettingsNavKey;
  settings: AppSettings | null;
  plugins: InstalledPluginRecord[];
  isLoading: boolean;
  isSaving: boolean;
  error: string | null;

  setActiveTab: (tab: SettingsNavKey) => void;
  loadAll: () => Promise<void>;
  patchSettings: (patch: Partial<Omit<AppSettings, "pinnedItems">>) => Promise<boolean>;
  togglePlugin: (pluginId: string, enabled: boolean) => Promise<void>;
}

let saveQueue: Promise<unknown> = Promise.resolve();
let pendingSaves = 0;

export const useSettingsStore = create<SettingsState>((set) => ({
  activeTab: "general",
  settings: null,
  plugins: [],
  isLoading: false,
  isSaving: false,
  error: null,

  setActiveTab: (activeTab) => set({ activeTab }),

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

  patchSettings: (patch) => {
    pendingSaves++;
    set({ isSaving: true, error: null });
    const operation = saveQueue.then(async () => {
      try {
        const saved = await ipc.updateSettings(patch);
        set({ settings: saved });
        return true;
      } catch (err) {
        set({ error: String(err) });
        return false;
      } finally {
        pendingSaves--;
        set({ isSaving: pendingSaves > 0 });
      }
    });
    saveQueue = operation;
    return operation;
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
