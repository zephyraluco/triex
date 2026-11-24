import { useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { ipc, type IndexStatus } from "@/lib/ipc";
import { useLauncherStore } from "@/stores/useLauncherStore";

const SEARCH_DEBOUNCE_MS = 16;
const DASHBOARD_FETCH_LIMIT = 300;
const HOME_FETCH_LIMIT = 100;

/**
 * Executes Rust nucleo-matcher + pinyin search with monotonic request IDs
 * to prevent out-of-order IPC responses from overwriting newer results.
 */
export function useSearch() {
  const query = useLauncherStore((s) => s.query);
  const showDashboard = useLauncherStore((s) => s.showDashboard);
  const activePlugin = useLauncherStore((s) => s.activePlugin);
  const setSearchResult = useLauncherStore((s) => s.setSearchResult);
  const requestSeqRef = useRef(0);
  const latestParamsRef = useRef({ query, showDashboard });
  latestParamsRef.current = { query, showDashboard };

  const runSearch = async (text: string, dashboardOpen: boolean) => {
    const requestId = ++requestSeqRef.current;
    const isBlank = text.trim().length === 0;
    const limit = isBlank
      ? dashboardOpen
        ? DASHBOARD_FETCH_LIMIT
        : HOME_FETCH_LIMIT
      : undefined;

    try {
      const response = await ipc.searchItems(text, requestId, limit);
      if (response.requestId === requestSeqRef.current) {
        setSearchResult(response.items, response.totalIndexed);
      }
    } catch (err) {
      console.error("Search failed:", err);
    }
  };

  useEffect(() => {
    if (activePlugin) return;
    const timer = window.setTimeout(() => {
      void runSearch(query, showDashboard);
    }, SEARCH_DEBOUNCE_MS);
    return () => window.clearTimeout(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [query, showDashboard, activePlugin]);

  useEffect(() => {
    const unlistenPromise = listen<IndexStatus>("triex://index-updated", () => {
      if (!useLauncherStore.getState().activePlugin) {
        const { query: q, showDashboard: d } = latestParamsRef.current;
        void runSearch(q, d);
      }
    });
    return () => {
      void unlistenPromise.then((unlisten) => unlisten());
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
}
