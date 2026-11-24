import { create } from "zustand";
import {
  ipc,
  type ActivePluginSession,
  type ClipboardSnapshot,
  type SearchResultItem,
} from "@/lib/ipc";

export type SettingsNavKey =
  | "general"
  | "appearance"
  | "index"
  | "plugins"
  | "commands";

export interface ContextMenuPosition {
  x: number;
  y: number;
}

export interface PastedAttachment {
  kind: "image" | "text" | "file";
  label: string;
  previewUrl?: string;
  rawText?: string;
}

interface LauncherState {
  query: string;
  subInput: string;
  results: SearchResultItem[];
  totalIndexed: number;
  selectedId: string;
  pinnedItems: string[];
  showDashboard: boolean;
  expandedSections: Record<string, boolean>;
  settingsOpen: boolean;
  settingsTab: SettingsNavKey;
  activePlugin: ActivePluginSession | null;
  clipboard: ClipboardSnapshot | null;
  attachment: PastedAttachment | null;
  actionMenuOpen: boolean;
  actionMenuPos: ContextMenuPosition | null;
  isRebuilding: boolean;
  statusMessage: string | null;

  setQuery: (query: string) => void;
  setSubInput: (value: string) => void;
  setSearchResult: (items: SearchResultItem[], totalIndexed: number) => void;
  setSelectedId: (id: string) => void;
  setPinnedItems: (ids: string[]) => void;
  toggleDashboard: () => void;
  setShowDashboard: (show: boolean) => void;
  toggleSectionExpanded: (sectionKey: string) => void;
  openSettings: (tab?: SettingsNavKey) => void;
  closeSettings: () => void;
  toggleSettings: () => void;
  setSettingsTab: (tab: SettingsNavKey) => void;
  setActivePlugin: (session: ActivePluginSession | null) => void;
  setSubInputPlaceholder: (placeholder: string) => void;
  setClipboard: (snapshot: ClipboardSnapshot | null) => void;
  setAttachment: (attachment: PastedAttachment | null) => void;
  openActionMenu: (pos?: ContextMenuPosition | null) => void;
  closeActionMenu: () => void;
  setIsRebuilding: (rebuilding: boolean) => void;
  setStatusMessage: (message: string | null) => void;
  resetSearch: () => void;
}

export const useLauncherStore = create<LauncherState>((set, get) => ({
  query: "",
  subInput: "",
  results: [],
  totalIndexed: 0,
  selectedId: "",
  pinnedItems: [],
  showDashboard: false,
  expandedSections: {},
  settingsOpen: false,
  settingsTab: "general",
  activePlugin: null,
  clipboard: null,
  attachment: null,
  actionMenuOpen: false,
  actionMenuPos: null,
  isRebuilding: false,
  statusMessage: null,

  setQuery: (query) =>
    set({
      query,
      expandedSections: {},
      actionMenuOpen: false,
      actionMenuPos: null,
    }),
  setSubInput: (subInput) => set({ subInput }),
  setSearchResult: (items, totalIndexed) =>
    set((state) => {
      const stillExists = items.some((item) => item.id === state.selectedId);
      return {
        results: items,
        totalIndexed,
        selectedId: stillExists ? state.selectedId : (items[0]?.id ?? ""),
      };
    }),
  setSelectedId: (selectedId) => set({ selectedId }),
  setPinnedItems: (pinnedItems) => set({ pinnedItems }),
  toggleDashboard: () => set((state) => ({ showDashboard: !state.showDashboard })),
  setShowDashboard: (showDashboard) => set({ showDashboard }),
  toggleSectionExpanded: (sectionKey) =>
    set((state) => ({
      expandedSections: {
        ...state.expandedSections,
        [sectionKey]: !state.expandedSections[sectionKey],
      },
    })),
  openSettings: (tab) => {
    void ipc.setSettingsViewOpen(true);
    set((state) => ({
      settingsOpen: true,
      settingsTab: tab ?? state.settingsTab,
      actionMenuOpen: false,
    }));
  },
  closeSettings: () => {
    void ipc.setSettingsViewOpen(false);
    set({ settingsOpen: false });
  },
  toggleSettings: () => {
    if (get().settingsOpen) {
      get().closeSettings();
    } else {
      get().openSettings();
    }
  },
  setSettingsTab: (settingsTab) => set({ settingsTab }),
  setActivePlugin: (activePlugin) =>
    set({
      activePlugin,
      settingsOpen: false,
      subInput: "",
      actionMenuOpen: false,
      actionMenuPos: null,
    }),
  setSubInputPlaceholder: (placeholder) =>
    set((state) =>
      state.activePlugin
        ? {
            activePlugin: {
              ...state.activePlugin,
              subInputPlaceholder: placeholder,
            },
          }
        : state,
    ),
  setClipboard: (clipboard) => set({ clipboard }),
  setAttachment: (attachment) => set({ attachment }),
  openActionMenu: (pos = null) => set({ actionMenuOpen: true, actionMenuPos: pos }),
  closeActionMenu: () => set({ actionMenuOpen: false, actionMenuPos: null }),
  setIsRebuilding: (isRebuilding) => set({ isRebuilding }),
  setStatusMessage: (statusMessage) => set({ statusMessage }),
  resetSearch: () => {
    if (get().settingsOpen) {
      void ipc.setSettingsViewOpen(false);
    }
    set({
      query: "",
      subInput: "",
      attachment: null,
      showDashboard: false,
      settingsOpen: false,
      expandedSections: {},
      actionMenuOpen: false,
      actionMenuPos: null,
    });
  },
}));
