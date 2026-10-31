import { ipc } from "./ipc";
import { shelfStore } from "./shelfStore";
import type { DropPayload, ShelfFile } from "../types";

/**
 * Turns an HTML5 drag or paste payload into an inbox payload.
 *
 * Real file paths arrive through the native drop target in Rust; this path
 * handles what the browser only exposes through `DataTransfer` (text, links,
 * HTML fragments and inline image data).
 */

function fileToDataUrl(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result ?? ""));
    reader.onerror = () => reject(reader.error ?? new Error("cannot read image"));
    reader.readAsDataURL(file);
  });
}

export async function captureContent(data: DataTransfer): Promise<ShelfFile[]> {
  const payload: DropPayload = {};

  const image = Array.from(data.files ?? []).find((file) => file.type.startsWith("image/"));
  if (image) {
    payload.image_base64 = await fileToDataUrl(image);
    payload.image_mime = image.type;
  }

  const html = data.getData("text/html");
  const plain = data.getData("text/plain");
  const uri = data.getData("text/uri-list").trim();

  if (html) payload.html = html;
  if (plain) payload.text = plain;
  if (uri) payload.url = uri;

  if (!payload.html && !payload.text && !payload.url && !payload.image_base64) {
    return [];
  }

  const saved = await ipc.ingestDrop([payload]);
  if (saved.length) shelfStore.upsert(saved);
  return saved;
}

