# tauri-plugin-everything

Tauri 2 plugin for read-only, paginated queries against the default local Everything instance on Windows. The user installs and starts Everything 1.4.1 or later. Windows builds compile the bundled SDK with the target C toolchain.

Register the plugin in the host application.

```rust
tauri::Builder::default()
    .plugin(tauri_plugin_everything::init())
```

Trusted Rust callers can import `EverythingExt` and use `app.everything().status().await` or `app.everything().search(request).await`. Search accepts Everything query syntax and returns live pages with a total count and an optional next offset. Size and modification time are optional. Modification time is a Unix timestamp in milliseconds.

Tauri apps can grant `everything:allow-status` and `everything:allow-search` to specific trusted WebViews. The corresponding commands are `plugin:everything|status` and `plugin:everything|search`. The search command takes a `request` object containing `query` and optional `offset`, `limit`, and `sort`. Search access can expose indexed file paths. Triex uses its own restricted IPC bridge and leaves these capabilities ungranted.

One background worker owns the SDK state. The queue accepts at most four waiting requests and callers time out after three seconds, including time spent waiting. The synchronous SDK call can continue after a caller times out. Non-Windows callers receive `unsupported_platform` from search and `unsupportedPlatform` from status.

## SDK provenance

The SDK C implementation and headers were taken from the official SDK archive. The archive used for this import had SHA-256 `f5716d9513cce6b462b5170a0a2e7e081e191d9bcac6774f5decc061497df443`. Changes for static linking remove the SDK's DLL export annotation, normalize whitespace and match the IPC header filename to the C include directive. The SDK license is included here and in the Triex installer resources. [Everything SDK](https://www.voidtools.com/support/everything/sdk/) · [SDK license](sdk/LICENSE)
