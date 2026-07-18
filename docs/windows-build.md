# Windows Build

Windows support is planned for v0.2 from the same codebase.

## Requirements

- Windows 11
- Node.js and npm
- Rust MSVC toolchain
- Tauri Windows prerequisites
- `whisper-cli-x86_64-pc-windows-msvc.exe`
- `whisper-stream-x86_64-pc-windows-msvc.exe`

## Storage

Campaigns:

```text
%USERPROFILE%\Documents\TableScribe\Campaigns\
```

Models:

```text
%APPDATA%\TableScribe\models\
```

## Build

```bash
npm install
npm run tauri:build
```
