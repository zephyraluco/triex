import { useEffect, useRef, type RefObject } from "react";
import { ipc } from "@/lib/ipc";
import { useLauncherStore } from "@/stores/useLauncherStore";

const MIN_WINDOW_HEIGHT = 64;
const MAX_WINDOW_HEIGHT = 540;
const EMBEDDED_SETTINGS_HEIGHT = 580;

/**
 * Synchronizes the borderless Tauri launcher window height with rendered content.
 * Locks to a generous height when the embedded ZTools-style settings panel is open.
 */
export function useWindowAutoResize(containerRef: RefObject<HTMLDivElement | null>) {
  const activePlugin = useLauncherStore((s) => s.activePlugin);
  const settingsOpen = useLauncherStore((s) => s.settingsOpen);
  const lastHeightRef = useRef<number>(MIN_WINDOW_HEIGHT);

  useEffect(() => {
    if (activePlugin) return;

    if (settingsOpen) {
      if (lastHeightRef.current !== EMBEDDED_SETTINGS_HEIGHT) {
        lastHeightRef.current = EMBEDDED_SETTINGS_HEIGHT;
        void ipc.resizeWindow(EMBEDDED_SETTINGS_HEIGHT);
      }
      return;
    }

    const el = containerRef.current;
    if (!el) return;

    let rafId = 0;

    const scheduleSync = () => {
      if (rafId !== 0) {
        window.cancelAnimationFrame(rafId);
      }
      rafId = window.requestAnimationFrame(() => {
        rafId = 0;
        const measured = Math.ceil(el.scrollHeight);
        const clamped = Math.max(MIN_WINDOW_HEIGHT, Math.min(MAX_WINDOW_HEIGHT, measured));
        if (clamped !== lastHeightRef.current) {
          lastHeightRef.current = clamped;
          void ipc.resizeWindow(clamped);
        }
      });
    };

    scheduleSync();
    const observer = new ResizeObserver(scheduleSync);
    observer.observe(el);

    return () => {
      observer.disconnect();
      if (rafId !== 0) {
        window.cancelAnimationFrame(rafId);
      }
    };
  }, [containerRef, activePlugin, settingsOpen]);
}
