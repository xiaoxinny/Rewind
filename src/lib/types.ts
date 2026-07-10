// TypeScript mirror of `rewind-core` types. Keep in sync with the Rust
// definitions — see implementation plan §10 and §7c. The real
// `CoreEvent` / `CoreCommand` / `AppConfig` shapes land in M1.

export type Millis = number;

export interface Timestamp {
  /** Unix milliseconds since the epoch. */
  readonly unixMs: number;
}

export const REWIND_VERSION = "0.1.0-m0";
