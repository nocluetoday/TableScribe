# Windows Installer

The `nsis` target is configured for v0.2.

Before releasing a Windows installer:

1. Build on a native Windows CI runner or Windows machine.
2. Include Windows x64 whisper.cpp sidecars.
3. Test microphone recording on Windows 11.
4. Confirm campaign files are written under Documents.
5. Confirm model files are written under AppData.
6. Document unsigned app warnings until code signing is available.

Do not create a separate Windows project.
