# whisper.cpp Integration

TableScribe uses whisper.cpp as a local sidecar process.

## Sidecar Files

Configured base names:

```text
src-tauri/binaries/whisper-cli
src-tauri/binaries/whisper-stream
```

Expected platform-specific examples:

```text
src-tauri/binaries/whisper-cli-aarch64-apple-darwin
src-tauri/binaries/whisper-stream-aarch64-apple-darwin
src-tauri/binaries/whisper-cli-x86_64-pc-windows-msvc.exe
src-tauri/binaries/whisper-stream-x86_64-pc-windows-msvc.exe
```

## Final Transcription

Final transcripts should be generated from the saved full WAV recording.

## Live Draft Mode

`whisper-stream` can be used later for live draft transcripts. Live output should be labeled experimental and should not replace final transcription.
