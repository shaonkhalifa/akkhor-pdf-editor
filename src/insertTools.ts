// Images, stamps and signatures (SRS Modules 4 and 5): choose a picture,
// position it on the page, then place it. Placing is one change that can be
// undone like any page change.
import { message, open as openDialog } from "@tauri-apps/plugin-dialog";
import { deleteImage, loadPicture, placeImage, replaceImage } from "./api";
import { t } from "./i18n";
import { useApp, type Placing, type Tab } from "./store";

const PICTURE_FILTER = () => [{ name: t(useApp.getState().lang, "pictureFiles"), extensions: ["png", "jpg", "jpeg", "webp"] }];
const tabOf = (id: number): Tab | undefined => useApp.getState().tabs.find((tab) => tab.doc.id === id);

async function showError(e: unknown) {
  const lang = useApp.getState().lang;
  const err = String(e);
  const text = err.includes("UNSUPPORTED_IMAGE")
    ? t(lang, "unsupportedImage")
    : err.includes("NO_SIGNATURE_FOUND")
      ? t(lang, "noSignatureFound")
      : err.includes("IMAGE_NOT_FOUND")
        ? t(lang, "imageNotEditable")
        : err;
  await message(text, { kind: "error", title: t(lang, "error") });
}

/**
 * Start positioning a picture on the current page. `widthPt` is how wide it
 * starts out; it is shrunk to fit the page if needed.
 */
export function startPlacing(id: number, dataUrl: string, pixelW: number, pixelH: number, widthPt: number) {
  const tab = tabOf(id);
  if (!tab) return;
  const page = tab.page;
  const [pw, ph] = tab.doc.pages[page];
  const aspect = pixelW / Math.max(1, pixelH);
  let w = Math.min(widthPt, pw * 0.8);
  let h = w / aspect;
  if (h > ph * 0.6) {
    h = ph * 0.6;
    w = h * aspect;
  }
  const placing: Placing = { dataUrl, page, aspect, center: [pw / 2, ph / 2], size: [w, h], rotation: 0, opacity: 1 };
  useApp.getState().update(id, { placing, imageMode: false });
}

export function updatePlacing(id: number, patch: Partial<Placing>) {
  const tab = tabOf(id);
  if (!tab?.placing) return;
  useApp.getState().update(id, { placing: { ...tab.placing, ...patch } });
}

export function cancelPlacing(id: number) {
  useApp.getState().update(id, { placing: null });
}

/** Put the picture being positioned onto the page. */
export async function commitPlacing(id: number) {
  const tab = tabOf(id);
  const p = tab?.placing;
  if (!p) return;
  try {
    const doc = await placeImage(id, p.page, p.dataUrl, { center: p.center, size: p.size, rotation: p.rotation, opacity: p.opacity });
    useApp.getState().update(id, { placing: null });
    useApp.getState().setDoc(doc);
  } catch (e) {
    await showError(e);
  }
}

/** Insert a picture file (PNG, JPEG or WebP) (FR-4.1). */
export async function insertPicture(id: number) {
  const path = await openDialog({ multiple: false, directory: false, filters: PICTURE_FILTER() });
  if (typeof path !== "string") return;
  try {
    const pic = await loadPicture(path);
    // Start at the picture's size at 150 dpi, at most half the page wide.
    startPlacing(id, pic.data_url, pic.width, pic.height, Math.min(pic.width * 0.48, 300));
  } catch (e) {
    await showError(e);
  }
}

export async function deleteExistingImage(id: number, page: number, xref: number) {
  try {
    useApp.getState().setDoc(await deleteImage(id, page, xref));
  } catch (e) {
    await showError(e);
  }
}

export async function replaceExistingImage(id: number, page: number, xref: number) {
  const path = await openDialog({ multiple: false, directory: false, filters: PICTURE_FILTER() });
  if (typeof path !== "string") return;
  try {
    useApp.getState().setDoc(await replaceImage(id, page, xref, path));
  } catch (e) {
    await showError(e);
  }
}

export { showError as showInsertError };
