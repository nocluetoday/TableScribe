# whisper.cpp Sidecars

Place compiled whisper.cpp binaries in this directory using Tauri's target-triple suffix convention.

Examples:

- `whisper-cli-aarch64-apple-darwin`
- `whisper-stream-aarch64-apple-darwin`
- `whisper-cli-x86_64-pc-windows-msvc.exe`
- `whisper-stream-x86_64-pc-windows-msvc.exe`

The base names configured in `tauri.conf.json` are:

- `binaries/whisper-cli`
- `binaries/whisper-stream`

Do not commit large model files here.
