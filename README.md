# CodingPet

English | [简体中文](README.zh-CN.md)

A tiny native desktop companion that reacts to your AI coding workflow.

**~14 MB typical Windows memory · ~0% idle CPU · Native Rust · No WebView · No extra runtime**

Windows · macOS

[**Download for Windows (x86_64)**](https://github.com/Enkialon/coding-pet/releases/download/v0.2.2/coding-pet-v0.2.2-windows-x86_64.exe) · [**Download for macOS (Universal)**](https://github.com/Enkialon/coding-pet/releases/download/v0.2.2/coding-pet-v0.2.2-macos-universal.zip) · [All releases](https://github.com/Enkialon/coding-pet/releases)

CodingPet turns coding-agent events into visible pet animations, so you can tell at a glance whether your agent is working, waiting for you, finished, or failed.

| Coding-agent state | Pet response |
|---|---|
| Working | Running |
| Reviewing or using a tool | Review |
| Waiting for input or permission | Waiting |
| Completed | Waving |
| Failed | Failed |

## Why CodingPet?

- **Tiny by design:** typical memory use is about 14 MB on the Windows GDI backend.
- **Quiet when idle:** the event-driven loop does not continuously redraw or poll.
- **Truly native:** pure Rust, with Win32/GDI on Windows and AppKit/Metal on macOS.
- **No WebView:** no browser process and no separate runtime to install.
- **Open pet format:** loads Codex v2 PNG/WebP pets and legacy Codex v1 pets.
- **Tool-neutral events:** Codex and other tools can drive the same local integration protocol.

## Get started

1. Download the build for your platform above.
2. Start CodingPet.
3. Put a Codex-compatible pet in `~/.codex/pets/`, or use a pet already installed there.
4. Connect your coding tool using the built-in emitter or adapter described below.

CodingPet automatically discovers pet folders containing `spritesheet.webp` or `spritesheet.png`. If no pet is installed yet, it opens with a generated fallback sheet so the application remains usable; a bundled starter pet is not included in v0.2.2.

## Coding-tool integration

CodingPet v0.2.2 includes a provider-neutral local event API, a generic command-line emitter, and a built-in Codex hook adapter. Integrations for other tools can translate their lifecycle events into the same protocol.

```powershell
coding-pet.exe emit agent.turn.started --source codex --session turn-123
coding-pet.exe emit agent.input.required --source claude-code --message "Approval needed"
coding-pet.exe emit agent.turn.completed --source cursor --session turn-123
```

Hook setup is currently manual; one-click installation is planned. See [Integration Protocol v1](docs/integrations.md) for the Codex adapter, event catalog, HTTP contract, security model, and adapter rules.

## Pet assets

CodingPet discovers pets in:

```text
~/.codex/pets/
```

```text
.codex/
└── pets/
    └── cat/
        ├── pet.json
        └── spritesheet.webp
```

The primary format is Codex v2: a transparent 1536×2288 PNG or WebP spritesheet arranged as an 8×11 grid. Legacy 1536×1872 Codex v1 sheets remain supported.

```json
{
  "id": "my-pet",
  "displayName": "My Pet",
  "description": "A custom desktop pet",
  "spriteVersionNumber": 2,
  "spritesheetPath": "spritesheet.webp"
}
```

The animation rows cover idle, dragging, waving, jumping, failure, waiting, running, review, and 16 clockwise pointer-look directions. Unknown `pet.json` fields are ignored for forward compatibility.

## Everyday features

- Transparent, borderless, always-on-top pet window
- Drag to reposition, with directional animation
- Pointer tracking with 16 look directions
- Multi-monitor and per-monitor DPI support
- Windows system tray and macOS menu-bar controls
- Right-click animation and pet selection
- Optional ambient animations and persistent settings

Animation states are resolved in this order:

```text
drag animation > pointer look > coding-tool status > ambient animation
```

Events received during pointer interaction are retained and shown after the interaction ends.

## Performance

CodingPet is designed for an event-driven, always-running desktop workload. The current Windows GDI backend typically uses about 14 MB of memory in normal use.

| Metric | Current target |
|---|---|
| Idle CPU | ≈ 0% |
| Animating CPU | < 2% |
| Windows RSS memory | < 30 MB; typically ~14 MB |
| Startup | No splash screen or network request |

Figures depend on platform, pet asset, display scale, and measurement method. These are current engineering targets rather than a cross-platform benchmark suite.

## Development

### Build

Install the stable Rust toolchain on Windows or macOS, then run:

```bash
cargo build --release
```

The output is `target/release/coding-pet.exe` on Windows or `target/release/coding-pet` on macOS. GitHub Releases provide a universal macOS `.app` bundle.

### Configuration

Settings are stored in `%APPDATA%\CodingPet\config\config.json` on Windows. Missing or corrupt configuration is reset to defaults.

### Architecture

- Single-threaded, event-driven Rust core
- Platform-neutral animation, pet asset, configuration, and integration layers
- Win32 `UpdateLayeredWindow` per-pixel transparency on Windows
- Transparent AppKit window with an event-driven Metal surface on macOS
- Redraws only when an animation frame changes

```text
src/
├── app/          # lifecycle, event dispatch, state, and pointer interaction
├── animation/    # animation playback and ambient timing
├── integration/  # CLI, HTTP server, runtime, and coding-tool events
├── pet/          # Codex assets, shared models, and provider contract
├── platform/     # native Windows and macOS window/rendering backends
├── ui/           # tray and settings UI
└── main.rs       # process entry point
```

## Project status

Current release: **v0.2.2**

Available now: the provider-neutral event protocol, generic emitter, Codex hook adapter, event-to-animation states, and settings UI. Planned work includes easier hook installation, a starter-pet experience, notifications, break reminders, and customizable mappings/adapters.

## License

Licensed under the [Apache License 2.0](LICENSE).
