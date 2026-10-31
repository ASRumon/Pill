/**
 * Cross-widget request to collapse the island.
 *
 * Starting a native drag (`dragFilesOut` → `DoDragDrop`) takes the mouse loop
 * away from the webview: Windows stops delivering `mousemove`/`mouseleave` to
 * the island while the drag is running. The panel therefore believed the
 * pointer was still inside and only collapsed once the cursor happened to leave
 * it again — the laggy close people noticed after dragging a clip out.
 *
 * Widgets call this once the OS drag has finished and the island closes with
 * its normal (guarded) collapse path.
 */
export function requestIslandCollapse(): void {
  window.dispatchEvent(new CustomEvent("island:collapse"));
}
