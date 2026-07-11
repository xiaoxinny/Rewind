// Design tokens — single source of truth (see docs/DESIGN_LANGUAGE.md
// Appendix A). Imported first so :root tokens are available to every
// component's <style> block on first paint.
import "./lib/tokens.css";

import App from "./App.svelte";
import { mount } from "svelte";

const target = document.getElementById("app");
if (!target) {
  throw new Error("Rewind: #app root element not found in index.html");
}

const app = mount(App, { target });

export default app;
