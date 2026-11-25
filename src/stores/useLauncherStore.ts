import { create } from "zustand";
import {
  ipc,
  type ActivePluginSession,
  type SearchResultItem,
} from "@/lib/ipc";

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
  activePlugin: ActivePluginSession | null;
  attachment: PastedAttachment | null;
  statusMessage: string | null;

  setQuery: (query: string) => void;
  setSubInput: (value: string) => void;
  setSearchResult: (items: SearchResultItem[], totalIndexed: number) => void;
  setSelectedId: (id: string) => void;
  setPinnedItems: (ids: string[]) => void;
  toggleDashboard: () => void;
  setShowDashboard: (show: boolean) => void;
  toggleSectionExpanded: (sectionKey: string) => void;
  setSettingsOpen: (open: boolean) => void;
  openSettings: () => void;
  closeSettings: () => void;
  toggleSettings: () => void;
  setActivePlugin: (session: ActivePluginSession | null) => void;
  setSubInputPlaceholder: (placeholder: string) => void;
  setAttachment: (attachment: PastedAttachment | null) => void;
  setStatusMessage: (message: string | null) => void;
  resetSearch: () => void;
}

export const useLauncherStore = create<LauncherState>((set) => ({
  query: "",
  subInput: "",
  results: [],
  totalIndexed: 0,
  selectedId: "",
  pinnedItems: [],
  showDashboard: false,
  expandedSections: {},
  settingsOpen: false,
  activePlugin: null,
  attachment: null,
  statusMessage: null,

  setQuery: (query) =>
    set({
      query,
      expandedSections: {},
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
  setSettingsOpen: (settingsOpen) =>
    set((state) => ({
      settingsOpen,
      activePlugin: settingsOpen ? null : state.activePlugin,
    })),
  openSettings: () => {
    void ipc.openSettings();
  },
  closeSettings: () => {
    void ipc.closeSettings();
  },
  toggleSettings: () => {
    void ipc.toggleSettings();
  },
  setActivePlugin: (activePlugin) =>
    set({
      activePlugin,
      settingsOpen: false,
      subInput: "",
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
  setAttachment: (attachment) => set({ attachment }),
  setStatusMessage: (statusMessage) => set({ statusMessage }),
  resetSearch: () =>
    set({
      query: "",
      subInput: "",
      attachment: null,
      showDashboard: false,
      expandedSections: {},
    }),
}));
