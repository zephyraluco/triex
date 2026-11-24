import { useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { ipc, type IndexStatus } from "@/lib/ipc";
import { useLauncherStore } from "@/stores/useLauncherStore";

const SEARCH_DEBOUNCE_MS = 16;

/**
 * Executes Rust nucleo-matcher + pinyin search with monotonic request IDs
 * to prevent out-of-order IPC responses from overwriting newer results.
 */
export function useSearch() {
  const query = useLauncherStore((s) => s.query);
  const activePlugin = useLauncherStore((s) => s.activePlugin);
  const setSearchResult = useLauncherStore((s) => s.setSearchResult);
  const requestSeqRef = useRef(0);
  const queryRef = useRef(query);
  queryRef.current = query;

  const runSearch = async (text: string) => {
    const requestId = ++requestSeqRef.current;
    try {
      const response = await ipc.searchItems(text, requestId);
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
      void runSearch(query);
    }, SEARCH_DEBOUNCE_MS);
    return () => window.clearTimeout(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [query, activePlugin]);

  useEffect(() => {
    const unlistenPromise = listen<IndexStatus>("triex://index-updated", () => {
      if (!useLauncherStore.getState().activePlugin) {
        void runSearch(queryRef.current);
      }
    });
    return () => {
      void unlistenPromise.then((unlisten) => unlisten());
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
}
