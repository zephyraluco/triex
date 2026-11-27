import { useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { ipc, type IndexStatus } from "@/lib/ipc";
import { useLauncherStore } from "@/stores/useLauncherStore";

/**
 * Executes Rust continuous substring + pinyin search immediately on query change
 * with monotonic request IDs to prevent out-of-order IPC responses from overwriting newer results.
 */
export function useSearch() {
  const query = useLauncherStore((s) => s.query);
  const showDashboard = useLauncherStore((s) => s.showDashboard);
  const activePlugin = useLauncherStore((s) => s.activePlugin);
  const setSearchResult = useLauncherStore((s) => s.setSearchResult);
  const requestSeqRef = useRef(0);
  const latestParamsRef = useRef({ query, showDashboard });
  const prevQueryRef = useRef(query);
  const pendingResetSelectionRef = useRef(true);
  latestParamsRef.current = { query, showDashboard };

  const runSearch = async (text: string, preserveSelection: boolean) => {
    const requestId = ++requestSeqRef.current;

    try {
      const response = await ipc.searchItems(text, requestId);
      if (response.requestId === requestSeqRef.current) {
        setSearchResult(response.items, response.totalIndexed, text, {
          preserveSelection,
        });
      }
    } catch (err) {
      console.error("Search failed:", err);
    }
  };

  useEffect(() => {
    if (prevQueryRef.current !== query) {
      prevQueryRef.current = query;
      pendingResetSelectionRef.current = true;
    }
    if (activePlugin) return;
    const preserveSelection = !pendingResetSelectionRef.current;
    pendingResetSelectionRef.current = false;
    void runSearch(query, preserveSelection);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [query, showDashboard, activePlugin]);

  useEffect(() => {
    const unlistenPromise = listen<IndexStatus>("triex://index-updated", () => {
      if (!useLauncherStore.getState().activePlugin) {
        const { query: q } = latestParamsRef.current;
        void runSearch(q, true);
      }
    });
    return () => {
      void unlistenPromise.then((unlisten) => unlisten());
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
}
