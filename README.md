# Rewind

A calm, privacy-first desktop wellness companion for people who spend their day at a keyboard. Rewind reminds you to look away, hydrate, stretch, and pause — and pauses itself when you have already stepped away. No activity tracking, no cloud sync, no microphone, no webcam. Everything stays on this device.

> Rest, and rewind, for you and your eyes.

## What Rewind is not

Rewind is a habit tool, not a treatment. It does not diagnose, prevent, or cure computer vision syndrome, dehydration, or musculoskeletal strain. The 20-20-20 framing it ships with is a common optometric recommendation, but the most recent randomized work found no measurable prophylactic effect on eye strain from scheduled 20-second breaks — see the in-app About panel and the project references for the source. Rewind is not a productivity coach. It does not give streaks. It does not phone home.

## Feature surface (v0.1.0)

Four independent reminder streams, each togglable in Settings.

- **Eye break (20-20-20)** — every 20 min, a 20 s look-away. Micro-break in the gentle default; toast-only, never an input-capturing overlay.
- **Rest break + guided eye exercise** — every 60 min, a 5 min rest with one of four CSS/SVG exercises (figure-eight, near-far focus, palming, blink cadence), recorded by `exercise_id`.
- **Hydration** — adaptive goal (default 2 L/day), reminders spread across waking hours, capped at one glass every 30 min so a worst-case prompt-following intake stays well under the renal-clearance ceiling. Quick-log from tray or dashboard defers the next reminder.
- **Posture / stretch** — every 40 min, coalesced onto rest breaks where possible.

The whole thing is idle-aware. Step away for 90 s and the timers pause. Step away for 5 min and the cycle resets. On GNOME Wayland, idle detection degrades to a timer-only mode with an honest UI note.

## Install on Linux (x86_64)

The `v0.1.0` release ships three portable artifacts. macOS and Windows binaries are not attached to this release; the CI workflow at `.github/workflows/release.yml` builds them automatically on every `v*` tag and uploads them to the matching release page — they will appear on the next tag. Until then, build from source below.

### Debian / Ubuntu (.deb)

```sh
sudo dpkg -i Rewind_0.1.0_amd64.deb
sudo apt-get install -f -y   # if dependency resolution complains
```

After install, `Rewind` launches from the applications menu and registers an autostart entry that you can toggle in Settings.

### AppImage

```sh
chmod +x Rewind_0.1.0_amd64.AppImage
./Rewind_0.1.0_amd64.AppImage
```

No install, no root, no autostart registration. The first run writes `~/.local/share/com.rewind.app/` for the SQLite history and settings.

### Fallback tarball (no bundles)

If your host cannot run the .deb installer or the AppImage (missing `libfuse2`, no FUSE at all, sandboxed CI runner, etc.), use the static binary tarball:

```sh
tar -xzf rewind-v0.1.0-x86_64-unknown-linux-gnu.tar.gz
./rewind
```

The tarball ships the bare `rewind` binary plus a `README.install.md` listing the runtime libraries the dynamic loader resolves at start (WebKitGTK 4.1, GTK 3, librsvg, libayatana-appindicator). It does not ship an autostart entry or a `.desktop` file.

## Build from source

Prerequisites: Rust stable (1.80+), Node.js 20 LTS, and the Tauri v2 system packages for your OS (`libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`, `patchelf` on Debian/Ubuntu; WebView2 + Visual Studio Build Tools on Windows; Xcode Command Line Tools on macOS).

```sh
git clone https://github.com/xiaoxinny/Rewind.git
cd Rewind
npm install
npm run tauri build -- --bundles deb,appimage
```

The resulting artifacts land in `src-tauri/target/release/bundle/{deb,appimage}/`.

## Configuration

All defaults can be changed in Settings. The shape is mirrored in `crates/rewind-core/src/config.rs` (Rust) and `src/lib/types.ts` (TypeScript). Highlights:

| Field | Default | Notes |
|---|---|---|
| micro interval | 20 min | The 20-20-20 cadence |
| micro duration | 20 s | Look-away time |
| rest interval | 60 min | The long break window |
| rest duration | 5 min | Inside rest, one guided exercise |
| strictness | gentle | gentle / normal / strict |
| idle pause threshold | 90 s | Pause after this much idle |
| idle reset threshold | 5 min | Reset the cycle after this much idle |
| hydration goal | 2000 ml | Adaptive; conservative default |
| hydration glass | 250 ml | One glass logged per nudge |
| posture interval | 40 min | Coalesces onto rest breaks |
| autostart | off | Toggle to register a login item |

## Architecture at a glance

A Cargo workspace enforces the engine-vs-adapter boundary at the type system:

```
crates/
  rewind-core/      # pure, deterministic state transformer; zero OS deps
  rewind-adapters/  # OS integrations: idle, notifier, overlay, autostart
  rewind-storage/   # SQLite history (sqlx, 0.8), migrations, daily rollups
src-tauri/          # composition root; 1 Hz tick loop, IPC commands
src/                # Svelte 5 + TypeScript UI; no business logic
```

The core is testable end-to-end with `FakeClock` + `FakeIdleSource` (see `crates/rewind-core/src/testing/`); unit tests cover the FSM, the idle pause/reset policy, the reminder coordinator, hydration interval/goal logic, and storage round-trips. Adapter behavior is verified manually per-OS.

## License

AGPL-3.0. See `LICENSE`. A note on the metadata: `Cargo.toml` and `package.json` still report `license = "MIT"` because those fields were never updated when the project moved to AGPL-3.0. The `LICENSE` file is the source of truth; the metadata fields will be reconciled in a later release.

The AGPL is the right license for a desktop app that wants to stay open and prevent a closed-source SaaS fork. If you want to embed Rewind's break-engine in a closed-source commercial product, contact the maintainer; a commercial licence is on the roadmap.

## Bug reports

https://github.com/xiaoxinny/Rewind/issues
