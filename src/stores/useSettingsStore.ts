import { create } from "zustand";
import {
  ipc,
  type AppSettings,
  type InstalledPluginRecord,
  type UpdateMetadata,
} from "@/lib/ipc";

export type SettingsNavKey =
  | "general"
  | "appearance"
  | "index"
  | "plugins"
  | "commands"
  | "about";

interface SettingsState {
  activeTab: SettingsNavKey;
  settings: AppSettings | null;
  plugins: InstalledPluginRecord[];
  availableUpdate: UpdateMetadata | null;
  isLoading: boolean;
  isSaving: boolean;
  error: string | null;

  setActiveTab: (tab: SettingsNavKey) => void;
  setAvailableUpdate: (update: UpdateMetadata | null) => void;
  loadAll: () => Promise<void>;
  patchSettings: (patch: Partial<Omit<AppSettings, "pinnedItems">>) => Promise<boolean>;
  togglePlugin: (pluginId: string, enabled: boolean) => Promise<void>;
  installPluginDir: (sourceDir: string) => Promise<boolean>;
  uninstallPlugin: (pluginId: string) => Promise<boolean>;
  reloadPlugins: () => Promise<void>;
}

let saveQueue: Promise<unknown> = Promise.resolve();
let pendingSaves = 0;

export const useSettingsStore = create<SettingsState>((set) => ({
  activeTab: "general",
  settings: null,
  plugins: [],
  availableUpdate: null,
  isLoading: false,
  isSaving: false,
  error: null,

  setActiveTab: (activeTab) => set({ activeTab }),
  setAvailableUpdate: (availableUpdate) => set({ availableUpdate }),

  loadAll: async () => {
    set({ isLoading: true, error: null });
    try {
      const [settings, plugins, availableUpdate] = await Promise.all([
        ipc.getSettings(),
        ipc.listPlugins(),
        ipc.getCachedUpdate().catch(() => null),
      ]);
      set({ settings, plugins, availableUpdate, isLoading: false });
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

  installPluginDir: async (sourceDir) => {
    set({ isSaving: true, error: null });
    try {
      await ipc.installPluginDir(sourceDir);
      const plugins = await ipc.listPlugins();
      set({ plugins, isSaving: false });
      return true;
    } catch (err) {
      set({ error: String(err), isSaving: false });
      return false;
    }
  },

  uninstallPlugin: async (pluginId) => {
    set({ isSaving: true, error: null });
    try {
      await ipc.uninstallPlugin(pluginId);
      set((state) => ({
        plugins: state.plugins.filter((p) => p.id !== pluginId),
        isSaving: false,
      }));
      return true;
    } catch (err) {
      set({ error: String(err), isSaving: false });
      return false;
    }
  },

  reloadPlugins: async () => {
    set({ isSaving: true, error: null });
    try {
      const plugins = await ipc.reloadPlugins();
      set({ plugins, isSaving: false });
    } catch (err) {
      set({ error: String(err), isSaving: false });
    }
  },
}));
