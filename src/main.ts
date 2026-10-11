import { mount } from "svelte";
import App from "./App.svelte";
import "./app.css";

async function start() {
  // Outside the desktop app (plain `npm run dev` in a browser) fake the backend.
  if (import.meta.env.DEV && !("__TAURI_INTERNALS__" in window)) {
    const { installMock } = await import("./lib/dev/mock");
    installMock();
  }
  mount(App, { target: document.getElementById("app")! });
}

start();
