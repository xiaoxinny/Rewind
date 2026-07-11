# Rewind

A calm, privacy-first desktop wellness companion for people who spend their day at a keyboard. Rewind reminds you to look away, hydrate, stretch, and pause — and pauses itself when you have already stepped away. No activity tracking, no cloud sync, no microphone, no webcam. Everything stays on this device.

> Rest, and rewind, for you and your eyes.

## What Rewind is not

Rewind is a habit tool, not a treatment. It does not diagnose, prevent, or cure computer vision syndrome, dehydration, or musculoskeletal strain. The 20-20-20 framing it ships with is widely recommended by optometric bodies and broadly supported in the literature, but the most recent randomized work found no measurable prophylactic effect on eye strain from scheduled 20 second breaks — that finding is in the in-app About panel and in `docs/EVIDENCE_AUDIT_EYE.md`. Rewind is not a productivity coach. It does not give streaks. It does not phone home.

## Feature surface (v0.1.0)

Four independent reminder streams, each togglable in Settings, each backed by a cited source in the About panel.

- **Eye break (20-20-20)** — every 20 min, a 20 s look-away. Micro-break in the gentle default; toast-only, never an input-capturing overlay.
- **Rest break + guided eye exercise** — every 60 min, a 5 min rest with one of four CSS/SVG exercises (figure-eight, near-far focus, palming, blink cadence), recorded by `exercise_id`.
- **Hydration** — adaptive goal (default 2 L/day), reminders spread across waking hours, capped at one glass every 30 min (≈0.5 L/hr worst case) to avoid the overhydration / hyponatremia risk the Mayo Clinic and Cleveland Clinic explicitly warn about. Quick-log from tray or dashboard defers the next reminder.
- **Posture / stretch** — every 40 min, coalesced onto rest breaks where possible.

The whole thing is idle-aware. Step away for 90 s and the timers pause (no nagging an empty chair). Step away for 5 min and the cycle resets. On GNOME Wayland, idle detection degrades to a timer-only mode with an honest UI note (no `ext-idle-notify-v1`, no `org_kde_kwin_idle`).

## Install on Linux (x86_64)

The `v0.1.0` release ships two portable artifacts plus a fallback tarball. macOS and Windows binaries will appear on subsequent tags (the new CI workflow builds them on every `v*` push).

### Debian / Ubuntu (.deb)

```sh
sudo dpkg -i Rewind_0.1.0_amd64.deb
# If dependencies are missing:
sudo apt-get install -f -y
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

The tarball ships the bare `rewind` binary plus a `README.install.md` listing the runtime libraries the dynamic loader resolves at start (WebKitGTK 4.1, GTK 3, librsvg, libayatana-appindicator). It does **not** ship an autostart entry or a `.desktop` file — those are a `.deb`-only concern.

## Build from source

Prerequisites: Rust stable (1.80+), Node.js 20 LTS, and the Tauri v2 system packages for your OS (`libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`, `patchelf` on Debian/Ubuntu; WebView2 + Visual Studio Build Tools on Windows; Xcode Command Line Tools on macOS).

```sh
git clone https://github.com/xiaoxinny/Rewind.git
cd Rewind
npm install
npm run tauri build -- --bundles deb,appimage
```

The resulting artifacts land in `src-tauri/target/release/bundle/{deb,appimage}/`.

On the maintainer's headless Debian 13 host, `cargo build --workspace` and `cargo test --workspace` are configured via a small wrapper script that points `cargo` at a local sysroot (no sudo, no system-wide package changes). See `~/.local/rewind-env.sh` on that machine; the same script is not checked in, by design.

## macOS and Windows builds

The release workflow at `.github/workflows/release.yml` builds all three platforms on every `v*` tag: `ubuntu-22.04` (x86_64), `macos-latest` for both `aarch64-apple-darwin` and `x86_64-apple-darwin`, and `windows-latest`. The workflow signs nothing by default — `tauri-action` uploads whatever `cargo tauri build` produces. Code-signed `.dmg` + notarized `.app`, and EV-signed `.msi` / `.exe` are scheduled for v0.2.0.

If you only want to install from source on macOS or Windows, follow the build instructions above; the build matrix and runner prerequisites are the same.

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

See `docs/M6.md` for the rest.

## Design + audit trail

Rewind's v0.1.0 release is the first cut where every source citation in the About panel and in this README is auditable against a written document. The hard rules baked into this release:

- **No emoji as UI icons** — per `docs/DESIGN_LANGUAGE.md` §1 (anti-AI slop position). Emoji may appear in copy but never as a control glyph.
- **No exclamation marks in headings, buttons, or body copy** — per §8 (voice & copy). The single allowed use is a system-emitted error dialog.
- **No "Get started" / "Welcome to Rewind" / CTA-heavy hero** — per §1 and §10. The first-run onboarding is a 4-screen evidence-honest flow, not a launcher button.
- **Every citation uses one of four templates (T1–T4)** in §8.4. The default is T2 for organisation-published health pages, T3 for PubMed-indexed papers.

The full design + audit documentation:

- `docs/IMPLEMENTATION_PLAN.md` — the execution spec, written to be read top-to-bottom by a contributor with no prior context
- `docs/DESIGN_LANGUAGE.md` — the v0.1 design contract, with a §12 acceptance checklist you can run on any UI PR
- `docs/ADVERSARIAL_UX_REPORT.md` — three personas (Big Mick, Dr. Lin, Sasha) run through the code before the v0.1.0 cut; every R-ticket that landed is listed by SHA at the bottom of the report
- `docs/EVIDENCE_AUDIT_EYE.md` — every source citation in the eye-strain About panel, traced against PubMed and the original journal. RED findings (fabricated cite) and AMBER findings (under-stated effect) are listed with the SHA that closed each
- `docs/EVIDENCE_AUDIT_HYDRATION.md` — same audit, for the hydration column. The adaptive cap is justified against Mayo Clinic and Cleveland Clinic guidance on hyponatremia
- `docs/VERIFICATION.md` — the build matrix at HEAD, per-OS manual checklist (deferred to v0.2.0), and the seven production-readiness gaps that remain open

## References

Source links used by the in-app About panel and this README. Each line uses the T1–T4 cite-line template from `docs/DESIGN_LANGUAGE.md` §8.4.

- American Optometric Association, *Computer Vision Syndrome* (page last reviewed October 2016). aoa.org/healthy-eyes/eye-and-vision-conditions/computer-vision-syndrome
- American Academy of Ophthalmology, *Computer Use and Vision* (Eye Health — Tips & Prevention, 2024). aao.org/eye-health/tips-prevention/computer-usage
- Health and Safety Executive, *Display Screen Equipment — frequently asked questions* (HSE, 2018). hse.gov.uk/msd/dse/
- Health and Safety Executive, *VDU breaks — frequently asked questions* (HSE). hse.gov.uk/contact/faqs/vdubreaks.htm
- Mayo Clinic, *Water: How much should you drink every day?* (Healthy Lifestyle — Nutrition, reviewed). mayoclinic.org/healthy-lifestyle/nutrition-and-healthy-eating/in-depth/water/art-20044256
- Mayo Clinic, *Hyponatremia — Symptoms & causes* (reviewed July 2025). mayoclinic.org/diseases-conditions/hyponatremia/symptoms-causes/syc-20373711
- Cleveland Clinic, *Water Intoxication* (Diseases & Conditions, reviewed September 2024). my.clevelandclinic.org/health/diseases/water-intoxication
- Singh S, McGuinness MB, Anderson AJ, Downie LE. *Interventions for the Management of Computer Vision Syndrome: A Systematic Review and Meta-analysis.* Ophthalmology 2022;129(10):1192–1215. doi:10.1016/j.ophtha.2022.05.009 · PMID 35597519
- Johnson S, et al. *20-20-20 Rule: Are These Numbers Justified?* Optom Vis Sci 2023;100(1):52–56. doi:10.1097/OPX.0000000000001971 · PMID 36473088

A working note on this exact cite set: the v0.1.0 release preceded a citation-cleanup commit (`eac5c08`, "chore(docs+settings): fix fabricated Singh 2022 BMJ Open cite + cite-updates from evidence audit") that closed the R-ticket on `Settings.svelte` line 478, where the About panel had been citing a non-existent "Singh et al. 2022 (BMJ Open)" paper. The real Singh 2022 paper is in *Ophthalmology* (PMID 35597519, listed above), not *BMJ Open*. `docs/EVIDENCE_AUDIT_EYE.md` §R1 has the full diff.

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

The core is testable end-to-end with `FakeClock` + `FakeIdleSource` (see `crates/rewind-core/src/testing/`); 158 unit tests cover the FSM, the idle pause/reset policy, the reminder coordinator, hydration interval/goal logic, and storage round-trips. Adapter behavior is verified manually per-OS — the per-OS checklist at `docs/VERIFICATION.md` §"Per-OS manual verification" is the gate for v0.2.0.

## License

AGPL-3.0. See `LICENSE`. A note on the metadata: `Cargo.toml` and `package.json` still report `license = "MIT"` because those fields were never updated when the project moved to AGPL-3.0. The `LICENSE` file is the source of truth; the metadata fields are tracked under `docs/VERIFICATION.md` known issues and will be reconciled in v0.2.0.

The AGPL is the right license for a desktop app that wants to stay open and prevent a closed-source SaaS fork. If you want to embed Rewind's break-engine in a closed-source commercial product, contact the maintainer; a commercial licence is on the roadmap.
