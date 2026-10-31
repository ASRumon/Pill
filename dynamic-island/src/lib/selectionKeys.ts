import { useEffect, useRef } from "react";
import { setListNavigationActive } from "./listNavigation";

/**
 * Explorer-style keyboard selection for a grid/list of tiles.
 *
 * Supported keys (only while a non-text control has focus):
 *  - Ctrl/⌘ + A        select everything in view
 *  - Ctrl/⌘ + Shift + A clear the selection
 *  - Arrow keys        move the selection (←/→ one item, ↑/↓ one row)
 *  - Shift + Arrow     extend the selection from the anchor
 *  - Home / End        first / last item (Shift extends)
 *  - Ctrl/⌘ + Space    toggle the focused item without clearing the rest
 *  - Delete/Backspace  remove the selected items
 *  - Escape            clear the selection (then `onEscape`, e.g. leave a folder)
 *  - Enter             activate the focused item (`onActivate`)
 */
export interface SelectionKeyOptions {
  /** Ids of the items in view, in visual order. */
  ids: string[];
  /** Currently selected ids. */
  selected: string[];
  setSelected: (next: string[]) => void;
  getAnchor: () => string | null;
  setAnchor: (id: string | null) => void;
  /** Number of columns the grid is laid out with, for ↑/↓. */
  columns: number;
  /** Attribute used to scroll the focused tile into view. */
  itemAttribute?: string;
  onActivate?: (id: string) => void;
  onDelete?: (ids: string[]) => void;
  onEscape?: () => void;
}

function isTextTarget(target: EventTarget | null): boolean {
  const element = target as HTMLElement | null;
  if (!element) return false;
  if (element.isContentEditable) return true;
  return ["INPUT", "TEXTAREA", "SELECT"].includes(element.tagName);
}

export function useSelectionKeys(options: SelectionKeyOptions): void {
  const latest = useRef(options);
  latest.current = options;

  // While something is selected the arrow keys belong to this list, not to the
  // widget tab bar.
  useEffect(() => {
    setListNavigationActive(options.selected.length > 0);
  }, [options.selected.length]);

  useEffect(() => () => setListNavigationActive(false), []);

  useEffect(() => {
    const scrollTo = (id: string) => {
      const selector = `[${latest.current.itemAttribute ?? "data-select-id"}="${CSS.escape(id)}"]`;
      document.querySelector(selector)?.scrollIntoView({ block: "nearest", inline: "nearest" });
    };

    const onKeyDown = (event: KeyboardEvent) => {
      if (isTextTarget(event.target)) return;

      const { ids, selected, setSelected, getAnchor, setAnchor, columns } = latest.current;
      const mod = event.ctrlKey || event.metaKey;
      const key = event.key;

      if (mod && (key === "a" || key === "A")) {
        event.preventDefault();
        if (event.shiftKey) {
          setSelected([]);
          setAnchor(null);
        } else {
          setSelected([...ids]);
          setAnchor(ids[ids.length - 1] ?? null);
        }
        return;
      }

      if (!ids.length) return;

      const anchorId = getAnchor();
      const anchorIndex = anchorId ? ids.indexOf(anchorId) : -1;
      const firstSelected = selected.reduce(
        (found, id) => (found >= 0 ? found : ids.indexOf(id)),
        -1,
      );
      const cursor = anchorIndex >= 0 ? anchorIndex : firstSelected;

      if (key === "Escape") {
        if (selected.length) {
          event.preventDefault();
          setSelected([]);
          setAnchor(null);
        } else {
          latest.current.onEscape?.();
        }
        return;
      }

      if (key === "Delete" || key === "Backspace") {
        const targets = selected.length
          ? selected
          : anchorId && ids.includes(anchorId)
            ? [anchorId]
            : [];
        if (!targets.length) return;
        event.preventDefault();
        setAnchor(null);
        latest.current.onDelete?.([...targets]);
        return;
      }

      if (key === "Enter") {
        // Buttons keep their own Enter behaviour.
        const tag = (event.target as HTMLElement | null)?.tagName;
        if (tag === "BUTTON" || tag === "A") return;
        const target = selected[0] ?? (anchorId && ids.includes(anchorId) ? anchorId : ids[0]);
        if (!target) return;
        event.preventDefault();
        latest.current.onActivate?.(target);
        return;
      }

      if (mod && key === " ") {
        const id = cursor >= 0 ? ids[cursor] : ids[0];
        if (!id) return;
        event.preventDefault();
        setAnchor(id);
        setSelected(
          selected.includes(id) ? selected.filter((value) => value !== id) : [...selected, id],
        );
        scrollTo(id);
        return;
      }

      let target = cursor;
      switch (key) {
        case "ArrowRight":
          target = cursor < 0 ? 0 : cursor + 1;
          break;
        case "ArrowLeft":
          target = cursor < 0 ? 0 : cursor - 1;
          break;
        case "ArrowDown":
          target = cursor < 0 ? 0 : cursor + Math.max(1, columns);
          break;
        case "ArrowUp":
          target = cursor < 0 ? 0 : cursor - Math.max(1, columns);
          break;
        case "Home":
          target = 0;
          break;
        case "End":
          target = ids.length - 1;
          break;
        default:
          return;
      }

      event.preventDefault();
      target = Math.max(0, Math.min(ids.length - 1, target));
      const targetId = ids[target];

      if (!event.shiftKey) {
        setAnchor(targetId);
        setSelected([targetId]);
      } else {
        const startId = anchorId && anchorIndex >= 0 ? anchorId : ids[cursor >= 0 ? cursor : target];
        setAnchor(startId);
        const start = ids.indexOf(startId);
        const low = Math.min(start, target);
        const high = Math.max(start, target);
        setSelected(ids.slice(low, high + 1));
      }
      scrollTo(targetId);
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);
}
