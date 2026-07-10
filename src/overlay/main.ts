import Overlay from "./Overlay.svelte";
import { mount } from "svelte";

const target = document.getElementById("overlay-root");
if (!target) {
  throw new Error("Rewind: #overlay-root element not found in overlay.html");
}

const app = mount(Overlay, { target });

export default app;
