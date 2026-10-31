import type { SearchEngine } from "../types";

/**
 * Engines that always exist. `{query}` is the only placeholder; custom engines
 * added in settings use the same syntax.
 */
export const BUILTIN_ENGINES: SearchEngine[] = [
  {
    id: "google",
    name: "Google",
    url: "https://www.google.com/search?q={query}",
  },
  {
    id: "youtube",
    name: "YouTube",
    url: "https://www.youtube.com/results?search_query={query}",
  },
  {
    id: "duckduckgo",
    name: "DuckDuckGo",
    url: "https://duckduckgo.com/?q={query}",
  },
];

export function allEngines(custom: SearchEngine[] = []): SearchEngine[] {
  return [...BUILTIN_ENGINES, ...custom];
}

/** Bakes the term into an engine template; spaces become `%20`-safe. */
export function engineUrl(engine: SearchEngine, query: string): string {
  return engine.url.split("{query}").join(encodeURIComponent(query.trim()));
}
