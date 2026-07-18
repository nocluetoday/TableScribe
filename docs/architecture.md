# Architecture

TableScribe is one cross-platform Tauri 2 app:

- React and TypeScript render the user workflow.
- Rust owns filesystem access, native commands, process management, and platform-specific paths.
- whisper.cpp runs as a bundled sidecar binary.
- Campaign data is stored as ordinary folders and readable text files.

## Layers

```text
src/
  screens/        User workflow screens
  components/     Shared UI controls
  lib/            TypeScript API and platform types

src-tauri/src/
  commands/       Tauri commands exposed to React
  platform/       macOS and Windows path and sidecar adapters
  services/       Filesystem, templates, whisper helpers
```

React should not hardcode macOS or Windows filesystem paths. Shared UI should display paths returned by Rust.

## Data Model

Campaigns are durable folders under the user's Documents directory. Transcripts, notes, and exports are plain files so users can read them without TableScribe.

## Sidecar Model

The app uses Tauri `externalBin` entries for `binaries/whisper-cli` and `binaries/whisper-stream`. Actual binaries must be named with the Rust target triple suffix.
