/**
 * Lets a list widget claim the arrow keys while it has a selection.
 *
 * `WidgetTabs` listens for ←/→ on the window to switch widgets. When the shelf
 * or clipboard has items selected the same keys have to move the selection
 * instead, so the list raises this flag and the tab bar steps aside.
 */
let active = false;

export function setListNavigationActive(value: boolean): void {
  active = value;
}

export function isListNavigationActive(): boolean {
  return active;
}
