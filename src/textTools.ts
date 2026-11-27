// Text editing (SRS Module 6): open a block or a new text box for editing,
// apply the change (asking what to do when the text no longer fits), and
// mark or black out text. All PDF work happens in the Rust core; every
// change can be undone.
import { message } from "@tauri-apps/plugin-dialog";
import {
  fontList,
  markupText,
  placeText,
  redactArea,
  type FontOption,
  type Fit,
  type MarkupKind,
  type StyleChoice,
  type TextBlock,
} from "./api";
import { choose } from "./components/ChoiceDialog";
import { num, t } from "./i18n";
import { useApp, type Editing, type Tab, type Tool } from "./store";

export const SANS = "Sans (Helvetica, Arial widths)";
const tabOf = (id: number): Tab | undefined => useApp.getState().tabs.find((tab) => tab.doc.id === id);
const lang = () => useApp.getState().lang;

let fonts: Promise<FontOption[]> | null = null;
/** Fonts the user can choose (scanned once). */
export function availableFonts(): Promise<FontOption[]> {
  fonts ??= fontList().catch(() => []);
  return fonts;
}

export function hexToRgb(hex: string): [number, number, number] {
  const n = parseInt(hex.replace("#", "").padEnd(6, "0").slice(0, 6), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

export function rgbToHex([r, g, b]: [number, number, number]): string {
  return `#${[r, g, b].map((v) => v.toString(16).padStart(2, "0")).join("")}`;
}

export function setTool(id: number, tool: Tool) {
  const tab = tabOf(id);
  if (!tab) return;
  const next = tab.tool === tool ? "none" : tool;
  useApp.getState().update(id, { tool: next, editing: null, placing: null });
}

/** Start editing an existing block (FR-6.1). */
export function editBlock(id: number, page: number, block: TextBlock) {
  const style: StyleChoice = {
    bangla_font: block.bangla_font,
    english_font: block.english_font,
    size: Math.round(block.size * 2) / 2,
    line_height: block.lines.length > 1 ? block.line_height : null,
    bold: block.bold,
    italic: block.italic,
    color: hexToRgb(block.color),
    align: "left",
  };
  const editing: Editing = { page, block, left: block.origin[0], top: block.rect[1], width: block.max_width, text: block.text, style };
  useApp.getState().update(id, { editing });
}

/** Start a new text box where the user clicked (FR-6.3). */
export function newTextBox(id: number, page: number, x: number, y: number) {
  const tab = tabOf(id);
  if (!tab) return;
  const [pw] = tab.doc.pages[page];
  const width = Math.max(60, Math.min(320, pw - x - 24));
  const style: StyleChoice = {
    bangla_font: "Noto Sans Bengali",
    english_font: SANS,
    size: 12,
    line_height: null,
    bold: false,
    italic: false,
    color: [0, 0, 0],
    align: "left",
  };
  useApp.getState().update(id, { editing: { page, block: null, left: x, top: y, width, text: "", style } });
}

export function updateEditing(id: number, patch: Partial<Editing>) {
  const tab = tabOf(id);
  if (!tab?.editing) return;
  useApp.getState().update(id, { editing: { ...tab.editing, ...patch } });
}

export function updateStyle(id: number, patch: Partial<StyleChoice>) {
  const tab = tabOf(id);
  if (!tab?.editing) return;
  updateEditing(id, { style: { ...tab.editing.style, ...patch } });
}

export function cancelEditing(id: number) {
  useApp.getState().update(id, { editing: null });
}

async function showError(e: unknown) {
  const err = String(e);
  const l = lang();
  const text = err.includes("NO_TEXT_THERE") ? t(l, "noTextThere") : err.includes("EMPTY_TEXT") ? t(l, "emptyText") : err;
  await message(text, { kind: "error", title: t(l, "error") });
}

/** Put the edited text into the PDF. Returns true when done. */
export async function applyEditing(id: number, fit: Fit = "check"): Promise<boolean> {
  const tab = tabOf(id);
  const e = tab?.editing;
  if (!e) return false;
  const b = e.block;
  if (b && e.text === b.text && fit === "check" && sameStyle(e, b)) {
    cancelEditing(id); // Nothing changed.
    return true;
  }
  const placement = b
    ? {
        left: e.left,
        width: e.width,
        baseline: b.origin[1],
        top: null,
        max_height: b.rect[3] - b.rect[1],
        remove: b.lines,
        fit,
      }
    : { left: e.left, width: e.width, baseline: null, top: e.top, max_height: null, remove: [], fit: "overflow" as Fit };
  try {
    const doc = await placeText(id, e.page, e.text, e.style, placement);
    useApp.getState().update(id, { editing: null });
    useApp.getState().setDoc(doc);
    return true;
  } catch (err) {
    const s = String(err);
    if (s.includes("TEXT_TOO_LONG")) {
      // FR-6.12: the text needs more lines than the box has.
      const [, need, fits] = s.slice(s.indexOf("TEXT_TOO_LONG")).split(":");
      const l = lang();
      const answer = await choose({
        title: t(l, "tooLongTitle"),
        message: t(l, "tooLongText", { need: num(l, Number(need)), fits: num(l, Number(fits)) }),
        buttons: [t(l, "shrinkToFit"), t(l, "letItOverflow"), t(l, "keepEditing")],
      });
      if (answer === 0) return applyEditing(id, "shrink");
      if (answer === 1) return applyEditing(id, "overflow");
      return false;
    }
    await showError(err);
    return false;
  }
}

function sameStyle(e: Editing, b: TextBlock) {
  const s = e.style;
  return (
    s.bangla_font === b.bangla_font &&
    s.english_font === b.english_font &&
    Math.abs(s.size - Math.round(b.size * 2) / 2) < 0.01 &&
    s.bold === b.bold &&
    s.italic === b.italic &&
    rgbToHex(s.color) === b.color &&
    s.align === "left"
  );
}

const MARK_COLORS: Record<MarkupKind, [number, number, number]> = {
  highlight: [255, 214, 0],
  underline: [30, 90, 220],
  strike: [210, 30, 30],
};

/** Highlight, underline or strike the text inside a dragged area (FR-6.7). */
export async function markArea(id: number, page: number, kind: MarkupKind, rect: number[]) {
  try {
    useApp.getState().setDoc(await markupText(id, page, kind, rect, MARK_COLORS[kind]));
  } catch (e) {
    await showError(e);
  }
}

/** Black out an area for good (FR-6.8), after asking. */
export async function redact(id: number, page: number, rect: number[]) {
  const l = lang();
  const answer = await choose({
    title: t(l, "redactTitle"),
    message: t(l, "redactAsk"),
    buttons: [t(l, "redactConfirm"), t(l, "cancel")],
  });
  if (answer !== 0) return;
  try {
    useApp.getState().setDoc(await redactArea(id, page, rect));
  } catch (e) {
    await showError(e);
  }
}
