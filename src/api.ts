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

// --- Images, stamps and signatures (Phase 4) --------------------------------

/** A picture ready to place (PNG or JPEG as a data URL). */
export interface Picture {
  data_url: string;
  width: number;
  height: number;
}

/** A box on the page in points (as shown), turned clockwise around its centre. */
export interface Placement {
  center: [number, number];
  size: [number, number];
  rotation: number;
  opacity: number;
}

export interface PageImage {
  rect: [number, number, number, number];
  /** Set when the image can be deleted or replaced. */
  xref: number | null;
  width: number;
  height: number;
}

export interface SavedSignature {
  id: string;
  label: string;
  data_url: string;
  created: number;
}

export const loadPicture = (path: string) => invoke<Picture>("load_picture", { path });

export const photoSignature = (path: string, ink: [number, number, number]) =>
  invoke<Picture>("photo_signature", { path, ink });

export const placeImage = (id: number, page: number, dataUrl: string, placement: Placement) =>
  invoke<DocInfo>("place_image", { id, page, dataUrl, placement });

export const pageImages = (id: number, page: number) => invoke<PageImage[]>("page_images", { id, page });

export const deleteImage = (id: number, page: number, xref: number) => invoke<DocInfo>("delete_image", { id, page, xref });

export const replaceImage = (id: number, page: number, xref: number, path: string) =>
  invoke<DocInfo>("replace_image", { id, page, xref, path });

export const listSignatures = () => invoke<SavedSignature[]>("list_signatures");

export const saveSignature = (label: string, dataUrl: string) => invoke<SavedSignature>("save_signature", { label, dataUrl });

export const deleteSignature = (id: string) => invoke<void>("delete_signature", { id });

// --- Text editing (Phase 5) ---------------------------------------------------

export interface FontOption {
  name: string;
  script: "bangla" | "english";
  source: "bundled" | "builtin" | "system";
  serif: boolean;
}

/** A block of text on a page, ready to edit. */
export interface TextBlock {
  text: string;
  rect: [number, number, number, number];
  lines: [number, number, number, number][];
  /** Baseline of the first line: [left, y]. */
  origin: [number, number];
  size: number;
  line_height: number;
  max_width: number;
  font: string;
  bold: boolean;
  italic: boolean;
  color: string;
  bangla_font: string;
  english_font: string;
  has_bangla: boolean;
  readable: boolean;
}

export type Align = "left" | "center" | "right";

export interface StyleChoice {
  bangla_font: string;
  english_font: string;
  size: number;
  line_height: number | null;
  bold: boolean;
  italic: boolean;
  color: [number, number, number];
  align: Align;
}

export type Fit = "check" | "shrink" | "overflow";

export interface TextPlacement {
  left: number;
  width: number;
  baseline: number | null;
  top: number | null;
  max_height: number | null;
  remove: [number, number, number, number][];
  fit: Fit;
}

export type MarkupKind = "highlight" | "underline" | "strike";

export const fontList = () => invoke<FontOption[]>("font_list");

export const textBlocks = (id: number, page: number) => invoke<TextBlock[]>("text_blocks", { id, page });

export const placeText = (id: number, page: number, text: string, style: StyleChoice, placement: TextPlacement) =>
  invoke<DocInfo>("place_text", { id, page, text, style, placement });

export const markupText = (id: number, page: number, markup: MarkupKind, rect: number[], color: [number, number, number]) =>
  invoke<DocInfo>("markup_text", { id, page, markup, rect, color });

export const redactArea = (id: number, page: number, rect: number[]) => invoke<DocInfo>("redact_area", { id, page, rect });
