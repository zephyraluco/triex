import { useEffect } from "react";
import { ipc } from "@/lib/ipc";

type EditableElement = HTMLInputElement | HTMLTextAreaElement;

function isEditableElement(node: EventTarget | null): node is EditableElement {
  if (node instanceof HTMLTextAreaElement) {
    return !node.disabled && !node.readOnly;
  }
  if (node instanceof HTMLInputElement) {
    const nonTextTypes = new Set([
      "button",
      "checkbox",
      "color",
      "file",
      "hidden",
      "image",
      "radio",
      "range",
      "reset",
      "submit",
    ]);
    return !node.disabled && !node.readOnly && !nonTextTypes.has(node.type);
  }
  return false;
}

/**
 * Updates an input/textarea value using the native prototype setter so React's
 * controlled input `onChange` handler always picks up the pasted text.
 */
function insertTextAtSelection(el: EditableElement, text: string) {
  let start = el.value.length;
  let end = el.value.length;
  try {
    if (el.selectionStart !== null && el.selectionEnd !== null) {
      start = el.selectionStart;
      end = el.selectionEnd;
    }
  } catch {
    // Fallback to appending for input types without selection ranges.
  }

  const nextValue = el.value.slice(0, start) + text + el.value.slice(end);
  const proto =
    el instanceof HTMLTextAreaElement
      ? HTMLTextAreaElement.prototype
      : HTMLInputElement.prototype;
  const nativeSetter = Object.getOwnPropertyDescriptor(proto, "value")?.set;
  if (nativeSetter) {
    nativeSetter.call(el, nextValue);
  } else {
    el.value = nextValue;
  }
  const cursorPos = start + text.length;
  el.focus();
  try {
    el.setSelectionRange(cursorPos, cursorPos);
  } catch {
    // Some input types (e.g. number) do not support setSelectionRange.
  }
  el.dispatchEvent(new Event("input", { bubbles: true }));
}

/**
 * Globally disables the WebView2 default browser context menu across the webview,
 * and turns right-click on any editable input into instant clipboard paste.
 */
export function useNativeContextMenu() {
  useEffect(() => {
    const onContextMenu = (event: MouseEvent) => {
      if (event.defaultPrevented) return;
      event.preventDefault();

      if (!isEditableElement(event.target)) {
        return;
      }

      const el = event.target;
      el.focus();
      void ipc
        .readClipboardText()
        .then((text) => {
          if (text && el.isConnected && !el.disabled && !el.readOnly) {
            insertTextAtSelection(el, text);
          }
        })
        .catch(() => {});
    };

    window.addEventListener("contextmenu", onContextMenu);
    return () => window.removeEventListener("contextmenu", onContextMenu);
  }, []);
}
