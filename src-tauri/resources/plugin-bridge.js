// Triex Plugin Bridge SDK injected via Tauri WebviewBuilder::initialization_script
(function () {
  if (window.triex) return;

  const invoke = (...args) => {
    const internals = window.__TAURI_INTERNALS__;
    if (!internals || typeof internals.invoke !== "function") {
      return Promise.reject(new Error("Triex IPC bridge unavailable"));
    }
    return internals.invoke(...args);
  };

  let enterHandler = null;
  let subInputHandler = null;
  let subInputKeyDownHandler = null;
  const backendListeners = new Map();

  window.addEventListener("triex:plugin-enter", (event) => {
    if (typeof enterHandler === "function") {
      enterHandler(event.detail);
    }
  });

  window.addEventListener("triex:sub-input-change", (event) => {
    if (typeof subInputHandler === "function") {
      subInputHandler(event.detail);
    }
  });

  window.addEventListener("triex:sub-input-keydown", (event) => {
    if (typeof subInputKeyDownHandler === "function") {
      subInputKeyDownHandler(event.detail);
    }
  });

  window.addEventListener("triex:backend-event", (event) => {
    const detail = event.detail;
    if (!detail || typeof detail.event !== "string") return;
    const handlers = backendListeners.get(detail.event);
    if (!handlers) return;
    for (const cb of handlers) {
      try {
        cb(detail.payload);
      } catch (err) {
        console.error("[triex] backend event handler failed:", err);
      }
    }
  });

  window.triex = Object.freeze({
    onPluginEnter(cb) {
      enterHandler = cb;
    },
    setSubInput(cb, placeholder = "") {
      subInputHandler = cb;
      return invoke("set_plugin_sub_input", { placeholder });
    },
    onSubInputKeyDown(cb) {
      subInputKeyDownHandler = cb;
    },
    setHeight(height) {
      return invoke("plugin_set_height", { height });
    },
    hideMainWindow() {
      return invoke("hide_window");
    },
    backend: Object.freeze({
      call(method, params = null, options = {}) {
        return invoke("plugin_backend_call", {
          method,
          params,
          timeoutMs: options?.timeoutMs,
        });
      },
      onEvent(eventName, cb) {
        if (typeof cb !== "function") return () => {};
        let set = backendListeners.get(eventName);
        if (!set) {
          set = new Set();
          backendListeners.set(eventName, set);
        }
        set.add(cb);
        return () => {
          set.delete(cb);
          if (set.size === 0) backendListeners.delete(eventName);
        };
      },
    }),
    process: Object.freeze({
      exec(command, args = [], options = {}) {
        return invoke("plugin_exec_command", {
          command,
          args,
          timeoutMs: options?.timeoutMs,
        });
      },
    }),
    db: Object.freeze({
      get(key) {
        return invoke("plugin_db_get", { key });
      },
      put(key, value) {
        return invoke("plugin_db_put", { key, value });
      },
      remove(key) {
        return invoke("plugin_db_delete", { key });
      },
    }),
    clipboard: Object.freeze({
      getSnapshot() {
        return invoke("get_clipboard_snapshot");
      },
    }),
  });
})();
