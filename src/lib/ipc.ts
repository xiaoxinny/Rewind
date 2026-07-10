// Typed `invoke()` wrappers for each Rust command + an event listener
// for `core-event`. Both surfaces land in M1; this stub is here so
// other modules can import the path without TS errors.

import type { Timestamp } from "./types";

// Stub — populated in M1 once the engine emits `core-event` over the
// Tauri event bus.
export async function pingEngine(): Promise<Timestamp> {
  return { unixMs: Date.now() };
}
