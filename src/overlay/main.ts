// Design tokens — single source of truth (see docs/DESIGN_LANGUAGE.md
// Appendix A). Imported here too so the overlay window's standalone
// Vite bundle (overlay.html → src/overlay/main.ts) gets the same
// :root tokens. The dashboard's main bundle imports this from
// src/main.ts; importing it again in the overlay entry is harmless
// (Vite dedupes by URL).
import "../lib/tokens.css";

import Overlay from "./Overlay.svelte";
import { mount } from "svelte";

const target = document.getElementById("overlay-root");
if (!target) {
  throw new Error("Rewind: #overlay-root element not found in overlay.html");
}

const app = mount(Overlay, { target });

export default app;
