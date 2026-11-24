/**
 * Builds the custom Tauri v2 protocol URL for cached application icons.
 * On Windows WebView2, custom schemes registered with `register_uri_scheme_protocol("icon", ...)`
 * are served under `http://icon.localhost/<path>`.
 */
export function getIconUrl(iconKey: string): string | null {
  if (!iconKey) {
    return null;
  }
  return `http://icon.localhost/${iconKey}.png`;
}
