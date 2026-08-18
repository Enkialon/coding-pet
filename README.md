# CodingPet

A lightweight Windows-native desktop pet powered by pure Rust.

CodingPet is an extremely lightweight programmer's desktop pet. It is compatible with the Codex custom pet resource format, displaying animated sprites directly on your desktop with near-zero resource usage.

## Features (V0.1)

- Transparent borderless animated pet on desktop
- Always On Top
- Mouse drag to reposition
- Multi-monitor support (including negative coordinates)
- Per-monitor DPI aware
- Loads Codex pet spritesheets (1536×1872, 8×9 grid, PNG/WebP)
- Auto-discovers pets from `%USERPROFILE%\.codex\pets\`
- System tray with show/hide/exit
- Right-click context menu (play animations, select pet)
- Random ambient animations (wave/jump)
- Configuration persistence
- Event-driven (near-zero idle CPU)

## Build

Requires Rust (stable toolchain, Windows target).

```bash
cargo build --release
```

Output: `target/release/coding-pet.exe` (single executable, no runtime dependencies)

## Run

Double-click `coding-pet.exe` or run from terminal:

```bash
./target/release/coding-pet.exe
```

For debug logging:
```bash
set RUST_LOG=debug
coding-pet.exe
```

## Codex Pet Import

CodingPet automatically discovers pets from the Codex pets directory:

```
%USERPROFILE%\.codex\pets\
```

### Directory Structure

```
.codex/
└── pets/
    ├── cat/
    │   ├── pet.json        (optional metadata)
    │   └── spritesheet.webp
    └── dog/
        ├── pet.json
        └── spritesheet.png
```

### Spritesheet Format

- Dimensions: 1536 × 1872 pixels
- Format: PNG or WebP with transparency
- Grid: 8 columns × 9 rows (192×208 per cell)
- Maximum file size: 20 MiB

### Animation Layout

| Row | Animation | Frames |
|-----|-----------|--------|
| 0 | Idle | 6 |
| 1 | Running Right | 8 |
| 2 | Running Left | 8 |
| 3 | Waving | 4 |
| 4 | Jumping | 5 |
| 5 | Failed | 8 |
| 6 | Waiting | 6 |
| 7 | Running | 6 |
| 8 | Review | 6 |

### pet.json (optional)

```json
{
  "name": "My Pet",
  "description": "A custom desktop pet"
}
```

Unknown fields are ignored for forward compatibility.

## Configuration

Config file location: `%APPDATA%\CodingPet\config.json`

```json
{
  "pet": "cat",
  "position": { "x": 1200, "y": 700 },
  "scale": 0.75,
  "always_on_top": true,
  "idle_animation": true
}
```

Missing or corrupt configuration is automatically reset to defaults.

## Performance Targets

| Metric | Target |
|--------|--------|
| Idle CPU | ≈ 0% (event-driven, no busy loop) |
| Animating CPU | < 2% |
| RSS Memory | < 30 MB (typical ~14 MB) |
| Startup | Instant (no splash, no network) |

## Architecture

- Pure Rust, single-threaded
- Win32 `UpdateLayeredWindow` for per-pixel alpha transparency
- `winit` for event loop and input handling
- `windows-sys` for zero-overhead Win32 FFI
- Event-driven animation (only redraws on frame change)
- No GPU, no web tech, no external runtime

## Roadmap

- **V0.2**: Windows Named Pipe IPC, unified Agent Event system, Codex/Cursor/Claude Code completion notifications
- **V0.3**: AI task status display (running/needs input/completed/failed), Toast notifications
- **V0.4**: Break reminders (sitting, water, eyes, wrist)
- **V0.5**: Custom event mapping, settings UI, plugin/adapter system

## License

[TBD]
