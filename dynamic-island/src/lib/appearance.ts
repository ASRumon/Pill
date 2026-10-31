import type { AppSettings } from "../types";

export type BackgroundMode = "glass" | "amoled" | "custom";

export const BACKGROUND_OPTIONS: { value: BackgroundMode; label: string }[] = [
  { value: "amoled", label: "AMOLED" },
  { value: "glass", label: "Glass" },
  { value: "custom", label: "Custom" },
];

/** Surface tokens owned by the appearance setting, in the order they reset. */
const SURFACE_VARS = [
  "--mica-tint",
  "--bg-main",
  "--bg-card",
  "--glass-blur",
  "--glass-saturation",
];

const MIN_OPACITY = 10;
const MAX_OPACITY = 100;
const MAX_BLUR = 40;

function clampInt(value: number, min: number, max: number, fallback: number) {
  return Number.isFinite(value) ? Math.min(max, Math.max(min, Math.round(value))) : fallback;
}

export function normalizeBackgroundMode(mode: string): BackgroundMode {
  // AMOLED is the default surface; only the explicit values opt out of it.
  return mode === "glass" || mode === "custom" ? mode : "amoled";
}

/**
 * Paints the island surface for the given appearance settings.
 *
 * `glass` leaves the stylesheet defaults alone so the OS light/dark tokens keep
 * working. The two dark presets write inline custom properties on the document
 * element (which win over both `:root` and the light-mode media query) and set
 * `data-surface="dark"` so the light-mode colour remaps in `styles.css` step
 * aside.
 */
export function applyIslandAppearance(
  settings: Pick<AppSettings, "island_background" | "island_opacity" | "island_blur">,
) {
  if (typeof document === "undefined") return;
  const root = document.documentElement;
  const mode = normalizeBackgroundMode(settings.island_background);

  if (mode === "glass") {
    SURFACE_VARS.forEach((name) => root.style.removeProperty(name));
    root.removeAttribute("data-surface");
    return;
  }

  root.setAttribute("data-surface", "dark");

  if (mode === "amoled") {
    root.style.setProperty("--mica-tint", "#000000");
    root.style.setProperty("--bg-main", "#000000");
    root.style.setProperty("--bg-card", "#000000");
    root.style.setProperty("--glass-blur", "0px");
    root.style.setProperty("--glass-saturation", "100%");
    return;
  }

  const alpha = clampInt(settings.island_opacity, MIN_OPACITY, MAX_OPACITY, 92) / 100;
  const blur = clampInt(settings.island_blur, 0, MAX_BLUR, 24);
  // Cards sit one step denser than the shell so they stay readable when the
  // shell is dialled down to a very low opacity.
  const cardAlpha = Math.min(1, alpha + 0.08);

  root.style.setProperty("--mica-tint", `rgba(18, 19, 26, ${alpha.toFixed(3)})`);
  root.style.setProperty("--bg-main", `rgba(13, 14, 20, ${alpha.toFixed(3)})`);
  root.style.setProperty("--bg-card", `rgba(28, 30, 38, ${cardAlpha.toFixed(3)})`);
  root.style.setProperty("--glass-blur", `${blur}px`);
  root.style.setProperty("--glass-saturation", "132%");
}
