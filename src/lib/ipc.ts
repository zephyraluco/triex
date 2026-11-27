import { invoke } from "@tauri-apps/api/core";

export type ItemKind = "app" | "uwpApp" | "systemCmd" | "plugin";

export type ThemeMode = "system" | "light" | "dark";
export type VibrancyMode = "mica" | "acrylic" | "none";

export interface SearchResultItem {
  id: string;
  kind: ItemKind;
  name: string;
  subtitle: string;
  execPath: string;
  iconKey: string;
  score: number;
  matchIndices: number[];
}

export interface SearchResponse {
  requestId: number;
  items: SearchResultItem[];
  totalIndexed: number;
}

export interface IndexStatus {
  totalIndexed: number;
}

export interface AppSettings {
  hotkey: string;
  theme: ThemeMode;
  vibrancy: VibrancyMode;
  maxResults: number;
  launchOnStartup: boolean;
  hideOnBlur: boolean;
  autoCheckUpdate: boolean;
  customScanDirs: string[];
  excludedApps: string[];
  pinnedItems: string[];
}

export interface PluginFeature {
  code: string;
  explain: string;
  cmds: string[];
}

export interface PluginBackendConfig {
  kind: "process";
  entry: string;
  args: string[];
  idleTimeoutSecs: number;
}

export interface PluginManifest {
  id: string;
  name: string;
  version: string;
  description: string;
  logo?: string | null;
  entry: string;
  height: number;
  backend?: PluginBackendConfig | null;
  features: PluginFeature[];
  permissions: string[];
}

export interface InstalledPluginRecord {
  id: string;
  name: string;
  version: string;
  description: string;
  entry: string;
  enabled: boolean;
  manifest: PluginManifest;
  installedAt: number;
}

export interface ActivePluginSession {
  pluginId: string;
  pluginName: string;
  featureCode: string;
  webviewLabel: string;
  subInputPlaceholder: string;
  logo?: string | null;
  loading?: boolean;
}

export interface UpdateMetadata {
  version: string;
  currentVersion: string;
  date: string | null;
  body: string | null;
}

export interface UpdateProgress {
  downloaded: number;
  contentLength: number | null;
}

export const ipc = {
  searchItems(query: string, requestId: number, limit?: number): Promise<SearchResponse> {
    return invoke("search_items", { query, requestId, limit });
  },

  rebuildIndex(): Promise<IndexStatus> {
    return invoke("rebuild_index");
  },

  launchItem(itemId: string, asAdmin = false): Promise<void> {
    return invoke("launch_item", { itemId, asAdmin });
  },

  openItemFolder(itemId: string): Promise<void> {
    return invoke("open_item_folder", { itemId });
  },

  togglePinItem(itemId: string): Promise<string[]> {
    return invoke("toggle_pin_item", { itemId });
  },

  removeRecentItem(itemId: string): Promise<void> {
    return invoke("remove_recent_item", { itemId });
  },

  showItemContextMenu(itemId: string): Promise<void> {
    return invoke("show_item_context_menu", { itemId });
  },

  showBrandContextMenu(): Promise<void> {
    return invoke("show_brand_context_menu");
  },

  readClipboardText(): Promise<string | null> {
    return invoke("read_clipboard_text");
  },

  resizeWindow(height: number): Promise<void> {
    return invoke("resize_window", { height });
  },

  hideWindow(): Promise<void> {
    return invoke("hide_window");
  },

  completeHide(epoch: number): Promise<void> {
    return invoke("complete_hide", { epoch });
  },

  listPlugins(): Promise<InstalledPluginRecord[]> {
    return invoke("list_plugins");
  },

  installPluginDir(sourceDir: string): Promise<PluginManifest> {
    return invoke("install_plugin_dir", { sourceDir });
  },

  uninstallPlugin(pluginId: string): Promise<void> {
    return invoke("uninstall_plugin", { pluginId });
  },

  reloadPlugins(): Promise<InstalledPluginRecord[]> {
    return invoke("reload_plugins");
  },

  openPluginsDir(): Promise<void> {
    return invoke("open_plugins_dir");
  },

  setPluginEnabled(pluginId: string, enabled: boolean): Promise<void> {
    return invoke("set_plugin_enabled", { pluginId, enabled });
  },

  closePlugin(): Promise<void> {
    return invoke("close_plugin_command");
  },

  detachPlugin(): Promise<void> {
    return invoke("detach_plugin_command");
  },

  forwardSubInputValue(value: string): Promise<void> {
    return invoke("forward_sub_input_value", { value });
  },

  forwardSubInputKeydown(key: string, shiftKey = false): Promise<void> {
    return invoke("forward_sub_input_keydown", { key, shiftKey });
  },

  getSettings(): Promise<AppSettings> {
    return invoke("get_settings");
  },

  updateSettings(patch: Partial<Omit<AppSettings, "pinnedItems">>): Promise<AppSettings> {
    return invoke("update_settings", { patch });
  },

  openSettings(): Promise<void> {
    return invoke("open_settings");
  },

  closeSettings(): Promise<void> {
    return invoke("close_settings");
  },

  toggleSettings(): Promise<void> {
    return invoke("toggle_settings");
  },

  checkUpdate(): Promise<UpdateMetadata | null> {
    return invoke("check_update");
  },

  getCachedUpdate(): Promise<UpdateMetadata | null> {
    return invoke("get_cached_update");
  },

  installUpdate(): Promise<void> {
    return invoke("install_update");
  },

  openDataDir(): Promise<void> {
    return invoke("open_data_dir");
  },
};
