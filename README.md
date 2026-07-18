# TableScribe

TableScribe is an open-source desktop app for macOS and Windows that records tabletop RPG sessions, transcribes them locally with whisper.cpp, and exports editable Markdown campaign notes.

## Current Status

This repository is a v0.1 starter implementation:

- Tauri 2 desktop shell
- React and TypeScript frontend
- Rust command layer for campaigns, imports, exports, models, settings, and transcription jobs
- Platform abstraction for macOS and Windows-ready paths
- whisper.cpp sidecar configuration placeholders
- Markdown-first campaign folder templates
- macOS `.dmg` packaging target configured
- Windows NSIS target configured for v0.2

The first production release targets Apple Silicon macOS. Windows x64 support is planned for v0.2 using the same codebase.

## Development

```bash
npm install
npm run tauri:dev
```

Build the app bundle:

```bash
npm run tauri:build
```

## Local Data

Campaigns are stored in the user's Documents folder:

- macOS: `~/Documents/TableScribe/Campaigns`
- Windows: `%USERPROFILE%\Documents\TableScribe\Campaigns`

Models are stored in the app data folder:

- macOS: `~/Library/Application Support/TableScribe/models`
- Windows: `%APPDATA%\TableScribe\models`

## whisper.cpp Sidecars

The Tauri config references:

- `src-tauri/binaries/whisper-cli`
- `src-tauri/binaries/whisper-stream`

Tauri expects target-triple suffixed binaries beside those base names, such as:

- `whisper-cli-aarch64-apple-darwin`
- `whisper-stream-aarch64-apple-darwin`
- `whisper-cli-x86_64-pc-windows-msvc.exe`

Large models are intentionally not bundled in the installer.

## License

Apache-2.0
