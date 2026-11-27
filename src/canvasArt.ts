// Stamps and signatures are drawn here, in the app's own window, and sent to
// the Rust core as transparent PNG pictures. The browser engine shapes Bangla
// text correctly (conjuncts and vowel signs), using the bundled or Windows
// fonts, so stamps like "অনুমোদিত" look right.
import dancingUrl from "./assets/fonts/DancingScript.ttf";
import vibesUrl from "./assets/fonts/GreatVibes-Regular.ttf";

/** Handwriting-style fonts for typed signatures (FR-5.2), bundled with the app. */
export const SIGNATURE_FONTS = [
  { family: "Akkhor Dancing", label: "Dancing Script", url: dancingUrl },
  { family: "Akkhor Vibes", label: "Great Vibes", url: vibesUrl },
  { family: "Nirmala UI", label: "Nirmala UI", url: null },
] as const;

/** Bangla falls back to a Windows font that has Bangla letters. */
const BANGLA_FALLBACK = `"Nirmala UI", "Vrinda", sans-serif`;

let fontsReady: Promise<void> | null = null;
export function loadSignatureFonts(): Promise<void> {
  fontsReady ??= Promise.all(
    SIGNATURE_FONTS.filter((f) => f.url).map(async (f) => {
      const face = new FontFace(f.family, `url(${f.url})`);
      document.fonts.add(await face.load());
    }),
  ).then(() => undefined);
  return fontsReady;
}

/** Pixels per point for stamps and typed signatures: sharp when printed. */
const SHARP = 4;

function canvas(w: number, h: number) {
  const c = document.createElement("canvas");
  c.width = Math.max(1, Math.ceil(w));
  c.height = Math.max(1, Math.ceil(h));
  const ctx = c.getContext("2d")!;
  return { c, ctx };
}

/** Crop a canvas to its drawn (non-transparent) pixels, plus a margin. */
export function trim(src: HTMLCanvasElement, margin = 8): HTMLCanvasElement | null {
  const ctx = src.getContext("2d")!;
  const { width: w, height: h } = src;
  const data = ctx.getImageData(0, 0, w, h).data;
  let [x0, y0, x1, y1] = [w, h, -1, -1];
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      if (data[(y * w + x) * 4 + 3] > 8) {
        if (x < x0) x0 = x;
        if (x > x1) x1 = x;
        if (y < y0) y0 = y;
        if (y > y1) y1 = y;
      }
    }
  }
  if (x1 < 0) return null;
  x0 = Math.max(0, x0 - margin);
  y0 = Math.max(0, y0 - margin);
  x1 = Math.min(w - 1, x1 + margin);
  y1 = Math.min(h - 1, y1 + margin);
  const out = canvas(x1 - x0 + 1, y1 - y0 + 1);
  out.ctx.drawImage(src, x0, y0, x1 - x0 + 1, y1 - y0 + 1, 0, 0, x1 - x0 + 1, y1 - y0 + 1);
  return out.c;
}

/** A typed name in a handwriting font (FR-5.2). */
export async function typedSignature(text: string, family: string, color: string): Promise<HTMLCanvasElement | null> {
  await loadSignatureFonts();
  const size = 40 * SHARP;
  const font = `${size}px "${family}", ${BANGLA_FALLBACK}`;
  const probe = canvas(1, 1).ctx;
  probe.font = font;
  const width = probe.measureText(text).width;
  const { c, ctx } = canvas(width + size, size * 1.8);
  ctx.font = font;
  ctx.fillStyle = color;
  ctx.textBaseline = "middle";
  ctx.fillText(text, size / 2, c.height / 2);
  return trim(c, 2 * SHARP);
}

/** Add the signer's name and/or today's date under a signature (FR-5.8). */
export function withCaption(sig: HTMLCanvasElement, lines: string[], color: string): HTMLCanvasElement {
  const shown = lines.filter((l) => l.trim());
  if (!shown.length) return sig;
  const size = Math.max(14, Math.round(sig.height * 0.16));
  const font = `${size}px "Segoe UI", ${BANGLA_FALLBACK}`;
  const probe = canvas(1, 1).ctx;
  probe.font = font;
  const textW = Math.max(...shown.map((l) => probe.measureText(l).width));
  const lineH = size * 1.35;
  const w = Math.max(sig.width, textW + size);
  const { c, ctx } = canvas(w, sig.height + lineH * shown.length + size * 0.4);
  ctx.drawImage(sig, (w - sig.width) / 2, 0);
  ctx.strokeStyle = color;
  ctx.lineWidth = Math.max(1, size / 14);
  ctx.beginPath();
  ctx.moveTo(w * 0.1, sig.height + size * 0.1);
  ctx.lineTo(w * 0.9, sig.height + size * 0.1);
  ctx.stroke();
  ctx.font = font;
  ctx.fillStyle = color;
  ctx.textAlign = "center";
  ctx.textBaseline = "top";
  shown.forEach((l, i) => ctx.fillText(l, w / 2, sig.height + size * 0.4 + i * lineH));
  return c;
}

export interface StampSpec {
  text: string;
  /** A second, smaller line (for example a date or office name). */
  sub?: string;
  color: string;
  shape: "box" | "round";
}

/** Ready-made stamps (FR-4.6), in both languages. */
export const STAMPS: { bn: string; en: string; color: string }[] = [
  { bn: "অনুমোদিত", en: "APPROVED", color: "#1a7f37" },
  { bn: "পরিশোধিত", en: "PAID", color: "#1f6feb" },
  { bn: "গৃহীত", en: "RECEIVED", color: "#1f6feb" },
  { bn: "খসড়া", en: "DRAFT", color: "#6e6e6e" },
  { bn: "গোপনীয়", en: "CONFIDENTIAL", color: "#c62828" },
  { bn: "বাতিল", en: "REJECTED", color: "#c62828" },
];

/** Draw a stamp: a rectangle or a round office seal, slightly rough like ink. */
export function drawStamp(spec: StampSpec): HTMLCanvasElement {
  const main = 30 * SHARP;
  const sub = 13 * SHARP;
  const mainFont = `700 ${main}px "Segoe UI", ${BANGLA_FALLBACK}`;
  const subFont = `600 ${sub}px "Segoe UI", ${BANGLA_FALLBACK}`;
  const probe = canvas(1, 1).ctx;
  probe.font = mainFont;
  const mainW = probe.measureText(spec.text).width;
  probe.font = subFont;
  const subW = spec.sub ? probe.measureText(spec.sub).width : 0;
  const line = 3 * SHARP;

  if (spec.shape === "round") {
    const r = Math.max(mainW, subW) / 2 + main * 0.9;
    const { c, ctx } = canvas(r * 2 + line * 2, r * 2 + line * 2);
    const cx = c.width / 2;
    const cy = c.height / 2;
    ctx.strokeStyle = ctx.fillStyle = spec.color;
    ctx.lineWidth = line;
    for (const rr of [r, r - line * 2.2]) {
      ctx.beginPath();
      ctx.arc(cx, cy, rr, 0, Math.PI * 2);
      ctx.stroke();
    }
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    ctx.font = mainFont;
    ctx.fillText(spec.text, cx, spec.sub ? cy - sub * 0.6 : cy);
    if (spec.sub) {
      ctx.font = subFont;
      ctx.fillText(spec.sub, cx, cy + main * 0.75);
    }
    return c;
  }

  const padX = main * 0.6;
  const padY = main * 0.35;
  const w = Math.max(mainW, subW) + padX * 2;
  const h = main * 1.3 + (spec.sub ? sub * 1.5 : 0) + padY * 2;
  const { c, ctx } = canvas(w + line * 2, h + line * 2);
  ctx.strokeStyle = ctx.fillStyle = spec.color;
  ctx.lineWidth = line;
  const radius = main * 0.25;
  ctx.beginPath();
  ctx.roundRect(line, line, w, h, radius);
  ctx.stroke();
  ctx.beginPath();
  ctx.roundRect(line * 2.6, line * 2.6, w - line * 3.2, h - line * 3.2, radius * 0.7);
  ctx.lineWidth = line / 2;
  ctx.stroke();
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  ctx.font = mainFont;
  ctx.fillText(spec.text, c.width / 2, line + padY + main * 0.65);
  if (spec.sub) {
    ctx.font = subFont;
    ctx.fillText(spec.sub, c.width / 2, line + padY + main * 1.3 + sub * 0.7);
  }
  return c;
}

/** Today's date, written the way the interface language writes it. */
export function today(lang: "bn" | "en"): string {
  return new Date().toLocaleDateString(lang === "bn" ? "bn-BD" : "en-GB", { day: "numeric", month: "long", year: "numeric" });
}

/** Turn an <img> source into a canvas (for saved signatures and photos). */
export function imageToCanvas(src: string): Promise<HTMLCanvasElement> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => {
      const { c, ctx } = canvas(img.naturalWidth, img.naturalHeight);
      ctx.drawImage(img, 0, 0);
      resolve(c);
    };
    img.onerror = () => reject(new Error("picture could not be shown"));
    img.src = src;
  });
}
