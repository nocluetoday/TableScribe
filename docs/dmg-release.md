# DMG Release

The Tauri bundle targets include `app` and `dmg`.

```bash
npm run tauri:build
```

Before publishing:

1. Confirm the Apple Silicon `whisper-cli-aarch64-apple-darwin` binary exists.
2. Confirm the app opens from the generated `.dmg`.
3. Confirm microphone permission appears with the TableScribe usage description.
4. Record a short test session.
5. Confirm raw WAV audio is preserved.
6. Export Markdown, TXT, and SRT.
7. Test on a clean macOS user account when possible.

Signing and notarization are planned before public distribution.
