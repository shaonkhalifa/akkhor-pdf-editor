// Typed calls into the Rust core. The UI never reads files itself.
import { invoke } from "@tauri-apps/api/core";

export interface OutlineItem {
  title: string;
  page: number | null;
  children: OutlineItem[];
}

export interface DocInfo {
  id: number;
  path: string;
  name: string;
  page_count: number;
  /** [width, height] in points for every page. */
  pages: [number, number][];
  outline: OutlineItem[];
  file_size: number;
}

export interface TextLine {
  text: string;
  rect: [number, number, number, number];
  size: number;
}

export interface PageLink {
  rect: [number, number, number, number];
  page: number | null;
  uri: string | null;
}

export interface SearchHit {
  page: number;
  rect: [number, number, number, number];
}

export const openPdf = (path: string, password?: string, force?: boolean) =>
  invoke<DocInfo>("open_pdf", { path, password, force });

export const closePdf = (id: number) => invoke<void>("close_pdf", { id });

export const pageText = (id: number, page: number) => invoke<TextLine[]>("page_text", { id, page });

export const pageLinks = (id: number, page: number) => invoke<PageLink[]>("page_links", { id, page });

export const searchPdf = (id: number, pageCount: number, needle: string) =>
  invoke<SearchHit[]>("search_pdf", { id, pageCount, needle });

export const saveCopy = (id: number, dest: string) => invoke<void>("save_copy", { id, dest });

export const recentFiles = () => invoke<string[]>("recent_files");

export const clearRecentFiles = () => invoke<void>("clear_recent_files");

export const renderPagePng = async (id: number, page: number, scale: number): Promise<Blob> => {
  const bytes = await invoke<ArrayBuffer>("render_page_png", { id, page, scale });
  return new Blob([bytes], { type: "image/png" });
};

// --- Page rendering -------------------------------------------------------
// The core sends raw RGBA pixels (8-byte width/height header). At most a few
// pages are drawn at once so the visible page is never stuck behind others.

const MAX_PARALLEL = 3;
let running = 0;
const waiting: (() => void)[] = [];

async function slot<T>(work: () => Promise<T>): Promise<T> {
  if (running >= MAX_PARALLEL) await new Promise<void>((r) => waiting.push(r));
  running++;
  try {
    return await work();
  } finally {
    running--;
    waiting.shift()?.();
  }
}

/** Draw a page; `scale` is device pixels per PDF point. */
export function renderPage(id: number, page: number, scale: number): Promise<ImageBitmap> {
  return slot(async () => {
    const buf = await invoke<ArrayBuffer>("render_page", { id, page, scale });
    const view = new DataView(buf);
    const w = view.getUint32(0, true);
    const h = view.getUint32(4, true);
    const pixels = new Uint8ClampedArray(buf, 8, w * h * 4);
    return createImageBitmap(new ImageData(pixels, w, h));
  });
}

/** CSS pixels per PDF point at 100% zoom (a point is 1/72 inch, a CSS pixel 1/96). */
export const PX_PER_PT = 96 / 72;
