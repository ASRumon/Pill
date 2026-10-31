let clipboardDragActive = false;

export function setClipboardDragActive(active: boolean): void {
  clipboardDragActive = active;
}

export function isClipboardDragActive(): boolean {
  return clipboardDragActive;
}
