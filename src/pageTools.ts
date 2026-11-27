// Page tools (SRS Module 3): the actions behind the toolbar buttons, the
// thumbnail menu and the keyboard shortcuts. All PDF work happens in Rust;
// these functions ask for it and put the updated document into the store.
import { message, open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import {
  editPages,
  extractPages,
  probePdf,
  redoEdit,
  savePdf,
  undoEdit,
  type DocInfo,
  type PageEdit,
} from "./api";
import { choose } from "./components/ChoiceDialog";
import { t } from "./i18n";
import { askPassword, baseName } from "./open";
import { useApp, type Tab } from "./store";

const tabOf = (id: number): Tab | undefined => useApp.getState().tabs.find((tab) => tab.doc.id === id);
const lang = () => useApp.getState().lang;

/** Pages an action works on: the selected thumbnails, or else the current page. */
export function targetPages(tab: Tab): number[] {
  return tab.selected.length ? tab.selected : [tab.page];
}

function stem(name: string) {
  return name.replace(/\.pdf$/i, "");
}

async function showError(e: unknown, title?: string) {
  const err = String(e);
  if (err.startsWith("NOTHING_TO_")) return;
  const text = err.includes("CANNOT_DELETE_ALL") ? t(lang(), "cannotDeleteAll") : err;
  await message(text, { kind: "error", title: title ?? t(lang(), "error") });
}

/** Run a page change and show the result. Returns the new document, or null. */
async function change(id: number, edit: PageEdit, selected?: number[], goTo?: number): Promise<DocInfo | null> {
  try {
    const doc = await editPages(id, edit);
    useApp.getState().setDoc(doc, selected);
    if (goTo !== undefined) useApp.getState().update(id, { goTo });
    return doc;
  } catch (e) {
    await showError(e);
    return null;
  }
}

export function rotatePages(id: number, degrees: number) {
  const tab = tabOf(id);
  if (!tab) return;
  return change(id, { kind: "rotate", pages: targetPages(tab), degrees });
}

export function deletePages(id: number) {
  const tab = tabOf(id);
  if (!tab) return;
  const pages = targetPages(tab);
  if (pages.length >= tab.doc.page_count) return showError("CANNOT_DELETE_ALL");
  const first = Math.min(...pages);
  return change(id, { kind: "delete", pages }, [], Math.min(first, tab.doc.page_count - pages.length - 1));
}

/** Move pages (kept in order) to just before the page now at `before`. */
export function movePages(id: number, pages: number[], before: number) {
  const tab = tabOf(id);
  if (!tab) return;
  const moving = [...new Set(pages)].sort((a, b) => a - b);
  // Where the moved pages land: after the unmoved pages that come before the drop point.
  let at = 0;
  for (let p = 0; p < before; p++) if (!moving.includes(p)) at++;
  const landed = moving.map((_, i) => at + i);
  const unchanged = landed.every((p, i) => p === moving[i]);
  if (unchanged) return;
  return change(id, { kind: "move", pages: moving, before }, landed);
}

export function insertBlank(id: number) {
  const tab = tabOf(id);
  if (!tab) return;
  const after = Math.max(...targetPages(tab));
  const size = tab.doc.pages[after] ?? [595, 842];
  return change(id, { kind: "insert_blank", at: after + 1, size }, [after + 1], after + 1);
}

/** Ask for a password until it is right; null if the user gives up. */
async function unlock(path: string): Promise<{ pages: number; password: string | null } | null> {
  let password: string | null = null;
  for (;;) {
    try {
      return { pages: await probePdf(path, password), password };
    } catch (e) {
      const err = String(e);
      if (err.includes("NEEDS_PASSWORD") || err.includes("WRONG_PASSWORD")) {
        password = await askPassword(baseName(path), err.includes("WRONG_PASSWORD"));
        if (password === null) return null;
        continue;
      }
      await message(`${t(lang(), "cannotOpen")}\n\n${err}`, { kind: "error", title: baseName(path) });
      return null;
    }
  }
}

export async function insertFromFile(id: number) {
  const tab = tabOf(id);
  if (!tab) return;
  const path = await openDialog({ multiple: false, directory: false, filters: [{ name: t(lang(), "pdfFiles"), extensions: ["pdf"] }] });
  if (typeof path !== "string") return;
  const other = await unlock(path);
  if (!other) return;
  const at = Math.max(...targetPages(tab)) + 1;
  const added = Array.from({ length: other.pages }, (_, i) => at + i);
  return change(id, { kind: "insert_file", at, path, password: other.password }, added, at);
}

export async function extractToFile(id: number) {
  const tab = tabOf(id);
  if (!tab) return;
  const pages = targetPages(tab);
  const dest = await saveDialog({
    defaultPath: `${stem(tab.doc.name)} ${t(lang(), "pagesSuffix")}.pdf`,
    filters: [{ name: t(lang(), "pdfFiles"), extensions: ["pdf"] }],
  });
  if (!dest) return;
  try {
    await extractPages(id, pages, dest);
    useApp.getState().notify(t(lang(), "extracted"));
  } catch (e) {
    await showError(e, t(lang(), "cannotSave"));
  }
}

export async function undo(id: number) {
  try {
    useApp.getState().setDoc(await undoEdit(id), []);
  } catch (e) {
    await showError(e);
  }
}

export async function redo(id: number) {
  try {
    useApp.getState().setDoc(await redoEdit(id), []);
  } catch (e) {
    await showError(e);
  }
}

/** Save to the document's own file. Returns false if it failed. */
export async function save(id: number): Promise<boolean> {
  try {
    useApp.getState().setDoc(await savePdf(id));
    useApp.getState().notify(t(lang(), "saved"));
    return true;
  } catch (e) {
    await showError(e, t(lang(), "cannotSave"));
    return false;
  }
}

export async function saveAs(id: number): Promise<boolean> {
  const tab = tabOf(id);
  if (!tab) return false;
  const dest = await saveDialog({ defaultPath: tab.doc.path, filters: [{ name: t(lang(), "pdfFiles"), extensions: ["pdf"] }] });
  if (!dest) return false;
  try {
    useApp.getState().setDoc(await savePdf(id, dest));
    useApp.getState().notify(t(lang(), "saved"));
    return true;
  } catch (e) {
    await showError(e, t(lang(), "cannotSave"));
    return false;
  }
}

/**
 * Before closing documents: if any have unsaved changes, ask whether to save
 * them. Returns true when it is fine to close.
 */
export async function confirmClose(ids: number[]): Promise<boolean> {
  const dirty = ids.map(tabOf).filter((tab): tab is Tab => !!tab?.doc.dirty);
  if (!dirty.length) return true;
  const l = lang();
  const text = dirty.length === 1 ? t(l, "unsavedOne", { name: dirty[0].doc.name }) : t(l, "unsavedMany", { n: dirty.length });
  const answer = await choose({ title: t(l, "unsavedTitle"), message: text, buttons: [t(l, "saveBtn"), t(l, "dontSave"), t(l, "cancel")] });
  if (answer === 2) return false;
  if (answer === 1) return true;
  for (const tab of dirty) if (!(await save(tab.doc.id))) return false;
  return true;
}
