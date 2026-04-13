# Tauri + React + Typescript

This template should help get you started developing with Tauri, React and Typescript in Vite.

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

## Auto Updater Setup

This app is configured to use the Tauri updater plugin with a GitHub Releases endpoint.

1. Update `src-tauri/tauri.conf.json` with your real repository endpoint:
	- `https://github.com/<owner>/<repo>/releases/latest/download/latest.json`
2. Replace `pubkey` in `src-tauri/tauri.conf.json` with your updater public key.
3. Build and publish signed updater artifacts (including `latest.json`) to your GitHub Releases.

After this is configured, the app checks for updates on startup and downloads the latest release automatically.
