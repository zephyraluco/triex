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

  window.triex = Object.freeze({
    onPluginEnter(cb) {
      enterHandler = cb;
    },
    setSubInput(cb, placeholder = "") {
      subInputHandler = cb;
      return invoke("set_plugin_sub_input", { placeholder });
    },
    hideMainWindow() {
      return invoke("hide_window");
    },
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
