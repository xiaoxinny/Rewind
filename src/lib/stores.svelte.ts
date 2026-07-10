// Svelte 5 runes-based reactive mirror of the engine's state. Lands in
// M1 once `core-event` events are wired in. Display-only — no
// business logic in the frontend (per implementation plan §10).

import type { Timestamp } from "./types";

export const state = $state({
  lastHeartbeat: null as Timestamp | null,
});
