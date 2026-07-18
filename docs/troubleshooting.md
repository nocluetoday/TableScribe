# Troubleshooting

## The App Cannot Find whisper.cpp

Confirm the sidecar binary exists in `src-tauri/binaries` with the target-triple suffix for your platform.

## Microphone Access Is Disabled

On macOS, open System Settings, then Privacy & Security, then Microphone, and enable TableScribe.

## Models Are Missing

Install a model through Model Manager or place the expected `ggml-*.bin` file in the models folder.

## Windows Shows an Unsigned App Warning

This is expected until the v0.2 Windows build is code-signed.
