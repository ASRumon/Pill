import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { applyIslandAppearance } from "./lib/appearance";
import "./styles.css";

// AMOLED is the default surface. Paint it before React mounts so the island
// never flashes the glass stylesheet first; IslandShell re-applies the saved
// choice (glass / custom) as soon as settings load.
applyIslandAppearance({ island_background: "amoled", island_opacity: 92, island_blur: 24 });

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
