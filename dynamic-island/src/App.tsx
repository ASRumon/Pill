import { getCurrentWindow } from "@tauri-apps/api/window";
import { EditorWindow } from "./components/EditorWindow";
import { SettingsWindow } from "./components/SettingsWindow";
import { IslandShell } from "./components/IslandShell";
import { FocusTimerProvider } from "./lib/FocusTimerContext";

type Route =
  | { kind: "island" }
  | { kind: "editor"; id: number }
  | { kind: "settings" };

function resolveRoute(): Route {
  try {
    const label = getCurrentWindow().label;

    if (label.startsWith("editor-")) {
      const id = Number.parseInt(label.slice("editor-".length), 10);
      return { kind: "editor", id: Number.isFinite(id) ? id : 0 };
    }

    if (label === "settings") {
      return { kind: "settings" };
    }

    return { kind: "island" };
  } catch {
    // Hash fallback keeps the routes testable in a plain browser.
    const hash = window.location.hash;

    if (hash.startsWith("#editor/")) {
      const id = Number.parseInt(hash.slice("#editor/".length), 10);
      return { kind: "editor", id: Number.isFinite(id) ? id : 0 };
    }

    if (hash.startsWith("#settings")) {
      return { kind: "settings" };
    }

    return { kind: "island" };
  }
}

function App() {
  const route = resolveRoute();

  if (route.kind === "editor") return <EditorWindow id={route.id} />;
  if (route.kind === "settings") return <SettingsWindow />;
  return <FocusTimerProvider><IslandShell /></FocusTimerProvider>;
}

export default App;
