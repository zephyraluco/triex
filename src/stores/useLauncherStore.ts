import { create } from "zustand";
import {
  ipc,
  type ActivePluginSession,
  type SearchResultItem,
} from "@/lib/ipc";
import { resolveFirstVisibleItemId } from "@/lib/launcherSections";

export interface PastedAttachment {
  kind: "image" | "text" | "file";
  label: string;
  previewUrl?: string;
  rawText?: string;
}

interface LauncherState {
  query: string;
  resultQuery: string;
  subInput: string;
  results: SearchResultItem[];
  totalIndexed: number;
  selectedId: string;
  pinnedItems: string[];
  showDashboard: boolean;
  expandedSections: Record<string, boolean>;
  settingsOpen: boolean;
  activePlugin: ActivePluginSession | null;
  viewLoading: boolean;
  attachment: PastedAttachment | null;
  statusMessage: string | null;

  setQuery: (query: string) => void;
  setSubInput: (value: string) => void;
  setSearchResult: (
    items: SearchResultItem[],
    totalIndexed: number,
    resultQuery: string,
    options?: { preserveSelection?: boolean },
  ) => void;
  setSelectedId: (id: string) => void;
  setPinnedItems: (ids: string[]) => void;
  toggleDashboard: () => void;
  setShowDashboard: (show: boolean) => void;
  toggleSectionExpanded: (sectionKey: string) => void;
  setSettingsOpen: (open: boolean, loading?: boolean) => void;
  openSettings: () => void;
  closeSettings: () => void;
  toggleSettings: () => void;
  setActivePlugin: (session: ActivePluginSession | null, loading?: boolean) => void;
  setViewLoading: (loading: boolean) => void;
  setSubInputPlaceholder: (placeholder: string) => void;
  setAttachment: (attachment: PastedAttachment | null) => void;
  setStatusMessage: (message: string | null) => void;
  resetSearch: () => void;
}

export const useLauncherStore = create<LauncherState>((set) => ({
  query: "",
  resultQuery: "",
  subInput: "",
  results: [],
  totalIndexed: 0,
  selectedId: "",
  pinnedItems: [],
  showDashboard: false,
  expandedSections: {},
  settingsOpen: false,
  activePlugin: null,
  viewLoading: false,
  attachment: null,
  statusMessage: null,

  setQuery: (query) => set({ query }),
  setSubInput: (subInput) => set({ subInput }),
  setSearchResult: (items, totalIndexed, resultQuery, options) =>
    set((state) => {
      const preserveSelection = options?.preserveSelection ?? false;
      const stillExists =
        preserveSelection && items.some((item) => item.id === state.selectedId);
      const defaultSelectedId = resolveFirstVisibleItemId(
        items,
        resultQuery,
        state.pinnedItems,
        state.showDashboard,
      );
      return {
        results: items,
        totalIndexed,
        resultQuery,
        expandedSections: preserveSelection ? state.expandedSections : {},
        selectedId: stillExists ? state.selectedId : defaultSelectedId,
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
  setSettingsOpen: (settingsOpen, loading = false) =>
    set((state) => ({
      settingsOpen,
      activePlugin: settingsOpen ? null : state.activePlugin,
      viewLoading: settingsOpen ? loading : false,
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
  setActivePlugin: (activePlugin, loading = false) =>
    set({
      activePlugin,
      settingsOpen: false,
      subInput: "",
      viewLoading: activePlugin ? loading : false,
    }),
  setViewLoading: (viewLoading) => set({ viewLoading }),
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
