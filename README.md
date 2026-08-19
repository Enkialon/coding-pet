# CodingPet

English | [简体中文](README.zh-CN.md)

A lightweight native desktop pet for Windows and macOS, powered by Rust.

CodingPet is an extremely lightweight programmer's desktop pet. It uses the Codex v2 custom-pet asset contract and a provider-neutral local event protocol, displaying coding activity directly on your desktop with near-zero idle resource usage.

## Features (V0.1)

- Transparent borderless animated pet on desktop
- Always On Top
- Mouse drag to reposition with directional pet animation
- Codex v2 pointer look directions (16 clockwise poses)
- Multi-monitor support (including negative coordinates)
- Per-monitor DPI aware
- Loads Codex v2 pet spritesheets (1536×2288, 8×11 grid, PNG/WebP)
- Reads legacy Codex v1 8×9 pets during migration
- Auto-discovers pets from `~/.codex/pets/`
- Windows system tray or macOS menu-bar icon with show/hide/exit
- Right-click context menu (play animations, select pet)
- Random ambient animations (wave/jump)
- Configuration persistence
- Event-driven (near-zero idle CPU)
- Local integration API for Codex and other coding tools

## Build

Requires the stable Rust toolchain on Windows or macOS.

```bash
cargo build --release
```

Output:

- Windows: `target/release/coding-pet.exe`
- macOS: `target/release/coding-pet` (GitHub Releases provide a universal `.app` bundle)

## Run

Double-click the downloaded application, or run the locally built executable:

```powershell
.\target\release\coding-pet.exe
```

```bash
./target/release/coding-pet
```

For debug logging:

```powershell
$env:RUST_LOG = "debug"
.\target\release\coding-pet.exe
```

## Coding-tool integrations

CodingPet exposes a token-protected HTTP API on a random loopback port and includes a generic
event emitter for hooks:

```powershell
coding-pet.exe emit agent.turn.started --source codex --session turn-123
coding-pet.exe emit agent.turn.completed --source codex --session turn-123
```

Codex, Claude Code, Cursor, IDE, Git, and CI adapters all translate their native lifecycle events
into the same versioned event envelope. See [Integration Protocol v1](docs/integrations.md) for
the event catalog, HTTP contract, security model, and adapter rules.

## Codex Pet Import

CodingPet automatically discovers pets from the Codex pets directory in the current user's home:

```
~/.codex/pets/
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

- Primary contract: Codex v2 (`spriteVersionNumber: 2`)
- Dimensions: 1536 × 2288 pixels
- Format: PNG or WebP with transparency
- Grid: 8 columns × 11 rows (192×208 per cell)
- Legacy v1 sheets without `spriteVersionNumber` remain readable at 1536 × 1872

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
| 9 | Look directions 000°–157.5° | 8 |
| 10 | Look directions 180°–337.5° | 8 |

Rows 9–10 contain one continuous clockwise set of 16 look poses. `000°` is up,
`090°` is screen-right, `180°` is down, and `270°` is screen-left.

### pet.json

```json
{
  "id": "my-pet",
  "displayName": "My Pet",
  "description": "A custom desktop pet",
  "spriteVersionNumber": 2,
  "spritesheetPath": "spritesheet.webp"
}
```

Unknown fields are ignored for forward compatibility. Omitting
`spriteVersionNumber` selects the legacy v1 contract.

## Interaction priority

Animation states are resolved in this order:

```text
drag animation > pointer look > coding-tool status > ambient animation
```

Hook events received during pointer interaction are retained. When interaction ends, CodingPet
continues with the latest task state or deferred one-shot notification.

## Configuration

Config file location: `%APPDATA%\CodingPet\config\config.json`

```json
{
  "pet": "cat",
  "position": { "x": 1200, "y": 700 },
  "scale": 0.75,
  "always_on_top": true,
  "idle_animation": false,
  "language": "auto"
}
```

| Field | Meaning |
|---|---|
| `pet` | Active pet ID |
| `position` | Window position in physical desktop pixels |
| `scale` | Pet scale factor |
| `always_on_top` | Keep the pet above other windows |
| `idle_animation` | Enable random ambient animations |
| `language` | `auto`, `zh`, or `en` |

Missing or corrupt configuration is automatically reset to defaults.

## Performance Targets

| Metric | Target |
|--------|--------|
| Idle CPU | ≈ 0% (event-driven, no busy loop) |
| Animating CPU | < 2% |
| RSS Memory | < 30 MB on the Windows GDI backend (typical ~14 MB) |
| Startup | Instant (no splash, no network) |

## Architecture

- Pure Rust, single-threaded
- Platform-neutral animation, pet asset, hook, and configuration core
- Windows backend: Win32 `UpdateLayeredWindow` per-pixel transparency
- macOS backend: transparent AppKit window with an event-driven Metal surface
- `winit` for event loop and input handling
- Event-driven animation (only redraws on frame change)
- No WebView or external runtime; Windows remains CPU/GDI-only
- Codex v2 asset contract in the pet resource layer
- Independent provider-neutral protocol in the coding-tool integration layer

## Roadmap

- **V0.2**: Provider adapters and configurable event-to-animation mappings
- **V0.3**: AI task status display (running/needs input/completed/failed), Toast notifications
- **V0.4**: Break reminders (sitting, water, eyes, wrist)
- **V0.5**: Custom event mapping, settings UI, plugin/adapter system

## Releasing

Pushing a semantic version tag automatically tests Windows and macOS, builds a Windows x86_64 executable and a universal macOS app, generates SHA-256 checksums, and publishes them to a GitHub Release.

```powershell
# First update the version in Cargo.toml, then:
git commit -am "chore: release v0.0.2"
git tag v0.0.2
git push origin main v0.0.2
```

The tag (without its leading `v`) must exactly match the package version in `Cargo.toml`.

## License

[TBD]
