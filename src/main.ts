import App from "./App.svelte";
import { mount } from "svelte";

const target = document.getElementById("app");
if (!target) {
  throw new Error("Rewind: #app root element not found in index.html");
}

const app = mount(App, { target });

export default app;
