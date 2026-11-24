import { create } from "zustand";
import type {
  ActivePluginSession,
  ClipboardSnapshot,
  SearchResultItem,
} from "@/lib/ipc";

interface LauncherState {
  query: string;
  subInput: string;
  results: SearchResultItem[];
  totalIndexed: number;
  selectedId: string;
  activePlugin: ActivePluginSession | null;
  clipboard: ClipboardSnapshot | null;
  actionMenuOpen: boolean;
  isRebuilding: boolean;
  statusMessage: string | null;

  setQuery: (query: string) => void;
  setSubInput: (value: string) => void;
  setSearchResult: (items: SearchResultItem[], totalIndexed: number) => void;
  setSelectedId: (id: string) => void;
  setActivePlugin: (session: ActivePluginSession | null) => void;
  setSubInputPlaceholder: (placeholder: string) => void;
  setClipboard: (snapshot: ClipboardSnapshot | null) => void;
  setActionMenuOpen: (open: boolean) => void;
  setIsRebuilding: (rebuilding: boolean) => void;
  setStatusMessage: (message: string | null) => void;
  resetSearch: () => void;
}

export const useLauncherStore = create<LauncherState>((set) => ({
  query: "",
  subInput: "",
  results: [],
  totalIndexed: 0,
  selectedId: "",
  activePlugin: null,
  clipboard: null,
  actionMenuOpen: false,
  isRebuilding: false,
  statusMessage: null,

  setQuery: (query) => set({ query, actionMenuOpen: false }),
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
  setActivePlugin: (activePlugin) =>
    set({
      activePlugin,
      subInput: "",
      actionMenuOpen: false,
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
  setActionMenuOpen: (actionMenuOpen) => set({ actionMenuOpen }),
  setIsRebuilding: (isRebuilding) => set({ isRebuilding }),
  setStatusMessage: (statusMessage) => set({ statusMessage }),
  resetSearch: () =>
    set({
      query: "",
      subInput: "",
      actionMenuOpen: false,
    }),
}));
