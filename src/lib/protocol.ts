/**
 * Builds the custom Tauri v2 protocol URL for cached application icons
 * or dynamically loaded plugin logos (`plugin:<pluginId>:<relativePath>`).
 */
export function getIconUrl(iconKey: string): string | null {
  if (!iconKey) {
    return null;
  }
  if (iconKey.startsWith("plugin:")) {
    const rest = iconKey.slice("plugin:".length);
    const sep = rest.indexOf(":");
    if (sep <= 0) return null;
    const pluginId = rest.slice(0, sep);
    const relPath = rest.slice(sep + 1).replace(/^\/+/, "");
    return getPluginAssetUrl(pluginId, relPath);
  }
  return `http://icon.localhost/${iconKey}.png`;
}

/**
 * Builds the custom Tauri v2 protocol URL for assets inside an installed plugin package.
 */
export function getPluginAssetUrl(pluginId: string, relPath?: string | null): string | null {
  if (!pluginId || !relPath) {
    return null;
  }
  const clean = relPath.trim().replace(/^\/+/, "");
  if (!clean) {
    return null;
  }
  return `http://plugin.localhost/${pluginId}/${clean}`;
}
