import { useEffect, type RefObject } from "react";
import { ipc } from "@/lib/ipc";
import { useLauncherStore } from "@/stores/useLauncherStore";

const MIN_WINDOW_HEIGHT = 64;
const MAX_WINDOW_HEIGHT = 500;

/**
 * Dynamically synchronizes the borderless Tauri launcher window height with
 * the rendered React container height when in search mode.
 */
export function useWindowAutoResize(containerRef: RefObject<HTMLDivElement | null>) {
  const activePlugin = useLauncherStore((s) => s.activePlugin);

  useEffect(() => {
    if (activePlugin) return;
    const el = containerRef.current;
    if (!el) return;

    let lastSent = 0;
    const syncHeight = () => {
      const rawHeight = Math.ceil(el.getBoundingClientRect().height);
      const clamped = Math.max(MIN_WINDOW_HEIGHT, Math.min(MAX_WINDOW_HEIGHT, rawHeight));
      if (clamped !== lastSent) {
        lastSent = clamped;
        void ipc.resizeWindow(clamped);
      }
    };

    syncHeight();
    const observer = new ResizeObserver(syncHeight);
    observer.observe(el);

    return () => observer.disconnect();
  }, [containerRef, activePlugin]);
}
