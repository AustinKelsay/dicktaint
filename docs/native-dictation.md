# Legacy Redirect: Native Dictation

Canonical locations:

- [`../llm/implementation/NATIVE_DESKTOP_DICTATION.md`](../llm/implementation/NATIVE_DESKTOP_DICTATION.md)
- [`../llm/implementation/MODEL_MANAGEMENT.md`](../llm/implementation/MODEL_MANAGEMENT.md)
- [`../llm/implementation/CONFIG_AND_PATH_RESOLUTION.md`](../llm/implementation/CONFIG_AND_PATH_RESOLUTION.md)

Operational runbooks:

- [`../llm/workflow/LOCAL_DEVELOPMENT.md`](../llm/workflow/LOCAL_DEVELOPMENT.md)
- [`../llm/workflow/SMOKE_TESTS.md`](../llm/workflow/SMOKE_TESTS.md)
- [`../llm/workflow/TROUBLESHOOTING.md`](../llm/workflow/TROUBLESHOOTING.md)

`Model download completed but file is still missing ...`
- Destination path was not persisted after download.
- Check local filesystem permissions and available disk space.

`whisper-cli transcription failed: ...`
- Verify model file is valid, command path is correct, and CLI can run manually.

## Manual CLI Verification

If you need to verify CLI independently:

```bash
whisper-cli --help
```

And with a known sample WAV:

```bash
whisper-cli \
  -m "$HOME/.dicktaint/whisper-models/ggml-base.en.bin" \
  -f /opt/homebrew/opt/whisper-cpp/share/whisper-cpp/jfk.wav \
  -l en \
  -otxt \
  -nt
```

## Notes on Language

Current implementation forces English transcription (`-l en` in Rust command invocation).

- Use `.en` models for best results with this setup.
- Multilingual transcription support would require code changes.
