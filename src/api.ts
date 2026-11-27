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
  /** Goes up on every change; pages must be redrawn when it changes. */
  version: number;
  /** Changes not yet saved to `path`. */
  dirty: boolean;
  can_undo: boolean;
  can_redo: boolean;
}

/** A change to the pages of an open document. Page numbers are zero-based. */
export type PageEdit =
  | { kind: "rotate"; pages: number[]; degrees: number }
  | { kind: "delete"; pages: number[] }
  | { kind: "move"; pages: number[]; before: number }
  | { kind: "reorder"; order: number[] }
  | { kind: "insert_blank"; at: number; size: [number, number] }
  | { kind: "insert_file"; at: number; path: string; password?: string | null };

export type SplitPlan = { mode: "every"; n: number } | { mode: "ranges"; text: string };

export interface TextLine {
  text: string;
  rect: [number, number, number, number];
  size: number;
  /** Writing direction, clockwise: 0 (left to right), 90, 180 or 270. */
  angle: number;
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

// --- Page tools (Phase 3) ----------------------------------------------------

export const editPages = (id: number, edit: PageEdit) => invoke<DocInfo>("edit_pages", { id, edit });

export const undoEdit = (id: number) => invoke<DocInfo>("undo", { id });

export const redoEdit = (id: number) => invoke<DocInfo>("redo", { id });

/** Save to the document's own file, or to `dest` (Save As). */
export const savePdf = (id: number, dest?: string) => invoke<DocInfo>("save_pdf", { id, dest });

export const extractPages = (id: number, pages: number[], dest: string) =>
  invoke<void>("extract_pages", { id, pages, dest });

export const splitPdf = (id: number, pageCount: number, plan: SplitPlan, dir: string, stem: string) =>
  invoke<string[]>("split_pdf", { id, pageCount, plan, dir, stem });

export const mergePdfs = (files: { path: string; password?: string | null }[], dest: string) =>
  invoke<void>("merge_pdfs", { files, dest });

/** Check a file opens as a PDF; returns its page count. Fails with NEEDS_PASSWORD / WRONG_PASSWORD. */
export const probePdf = (path: string, password?: string | null) => invoke<number>("probe_pdf", { path, password });

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
export function renderPage(id: number, page: number, scale: number, version = 0): Promise<ImageBitmap> {
  return slot(async () => {
    // `version` is only there so callers redraw after a change.
    void version;
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
