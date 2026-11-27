// Create and convert (SRS Module 7): the dialogs for new PDFs, pictures,
// compression, passwords, and page numbers / header / footer / watermark.
import { useEffect, useRef, useState, type ReactNode } from "react";
import { message, open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import {
  compressPdf,
  decoratePages,
  exportPictures,
  newBlankPdf,
  picturesToPdf,
  protectPdf,
  unprotectPdf,
  ocrPage,
  ocrStatus,
  pageText,
  type OcrStatus,
  type Compression,
  type DocInfo,
  type PictureFormat,
  type PictureLayout,
} from "../api";
import { fileSize, num, t, type Key } from "../i18n";
import { baseName } from "../open";
import { targetPages } from "../pageTools";
import { useApp } from "../store";
import { hexToRgb, rgbToHex, SANS } from "../textTools";

function Dialog({ title, onClose, children, actions }: { title: string; onClose: () => void; children: ReactNode; actions: ReactNode }) {
  return (
    <div className="overlay" role="dialog" aria-modal="true" aria-label={title} onKeyDown={(e) => e.key === "Escape" && onClose()}>
      <div className="dialog wide">
        <h2>{title}</h2>
        {children}
        <div className="row end">{actions}</div>
      </div>
    </div>
  );
}

const stemOf = (name: string) => name.replace(/\.pdf$/i, "");
const pdfFilter = () => [{ name: t(useApp.getState().lang, "pdfFiles"), extensions: ["pdf"] }];

async function fail(e: unknown) {
  const lang = useApp.getState().lang;
  const err = String(e);
  const text = err.includes("NO_OFFICE") ? t(lang, "noOffice") : err.includes("EMPTY_TEXT") ? t(lang, "emptyText") : err;
  await message(text, { kind: "error", title: t(lang, "error") });
}

// ---------------------------------------------------------------------------

const SIZES: { key: Key; size: [number, number] }[] = [
  { key: "sizeA4", size: [595, 842] },
  { key: "sizeLetter", size: [612, 792] },
  { key: "sizeLegal", size: [612, 1008] },
  { key: "sizeA5", size: [420, 595] },
];

/** A new blank PDF (FR-7.1). */
export function NewPdfDialog({ onClose }: { onClose: () => void }) {
  const lang = useApp((s) => s.lang);
  const addTab = useApp((s) => s.addTab);
  const [preset, setPreset] = useState(0);
  const [custom, setCustom] = useState<[number, number]>([210, 297]); // mm
  const [landscape, setLandscape] = useState(false);
  const [pages, setPages] = useState(1);
  const [busy, setBusy] = useState(false);

  const create = async () => {
    const mm = (v: number) => (v * 72) / 25.4;
    let size: [number, number] = preset < SIZES.length ? SIZES[preset].size : [mm(custom[0]), mm(custom[1])];
    if (landscape) size = [size[1], size[0]];
    setBusy(true);
    try {
      addTab(await newBlankPdf(size, pages, `${t(lang, "untitled")}.pdf`));
      onClose();
    } catch (e) {
      await fail(e);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog
      title={t(lang, "newPdf")}
      onClose={onClose}
      actions={
        <>
          <button className="ghost" onClick={onClose}>{t(lang, "cancel")}</button>
          <button className="primary" disabled={busy} onClick={create}>{t(lang, "create")}</button>
        </>
      }
    >
      <div className="choice-grid">
        {SIZES.map((s, i) => (
          <label key={s.key} className="choice">
            <input type="radio" checked={preset === i} onChange={() => setPreset(i)} /> {t(lang, s.key)}
          </label>
        ))}
        <label className="choice">
          <input type="radio" checked={preset === SIZES.length} onChange={() => setPreset(SIZES.length)} /> {t(lang, "sizeCustom")}
          <input type="number" className="small-num" min={20} max={5000} value={custom[0]} onFocus={() => setPreset(SIZES.length)} onChange={(e) => setCustom([Number(e.target.value) || 1, custom[1]])} />
          ×
          <input type="number" className="small-num" min={20} max={5000} value={custom[1]} onFocus={() => setPreset(SIZES.length)} onChange={(e) => setCustom([custom[0], Number(e.target.value) || 1])} />
          mm
        </label>
      </div>
      <label className="choice">
        <input type="checkbox" checked={landscape} onChange={(e) => setLandscape(e.target.checked)} /> {t(lang, "landscape")}
      </label>
      <label className="choice">
        {t(lang, "pageCount")}
        <input type="number" className="small-num" min={1} max={500} value={pages} onChange={(e) => setPages(Math.max(1, Math.min(500, Number(e.target.value) || 1)))} />
      </label>
    </Dialog>
  );
}

/** Pictures to one PDF (FR-7.2). */
export function PicturesDialog({ onClose }: { onClose: () => void }) {
  const lang = useApp((s) => s.lang);
  const addTab = useApp((s) => s.addTab);
  const [files, setFiles] = useState<string[]>([]);
  const [layout, setLayout] = useState<PictureLayout>("a4");
  const [busy, setBusy] = useState(false);

  const add = async () => {
    const chosen = await openDialog({ multiple: true, directory: false, filters: [{ name: t(lang, "pictureFiles"), extensions: ["png", "jpg", "jpeg", "webp"] }] });
    const list = Array.isArray(chosen) ? chosen : chosen ? [chosen] : [];
    setFiles((f) => [...f, ...list]);
  };
  const move = (i: number, by: number) =>
    setFiles((list) => {
      const next = [...list];
      const [x] = next.splice(i, 1);
      next.splice(Math.max(0, Math.min(next.length, i + by)), 0, x);
      return next;
    });

  const make = async () => {
    setBusy(true);
    try {
      addTab(await picturesToPdf(files, layout, `${t(lang, "picturesPdfName")}.pdf`));
      onClose();
    } catch (e) {
      await fail(e);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog
      title={t(lang, "picturesToPdf")}
      onClose={onClose}
      actions={
        <>
          <button className="ghost" onClick={onClose}>{t(lang, "cancel")}</button>
          <button className="primary" disabled={!files.length || busy} onClick={make}>
            {busy ? t(lang, "working") : t(lang, "create")}
          </button>
        </>
      }
    >
      <p className="muted small">{t(lang, "picturesIntro")}</p>
      <ol className="merge-list">
        {files.map((f, i) => (
          <li key={`${f}-${i}`}>
            <span className="file-badge">IMG</span>
            <span className="merge-name" title={f}>{baseName(f)}</span>
            <button className="tool" aria-label={t(lang, "moveUp")} disabled={i === 0} onClick={() => move(i, -1)}>↑</button>
            <button className="tool" aria-label={t(lang, "moveDown")} disabled={i === files.length - 1} onClick={() => move(i, 1)}>↓</button>
            <button className="tool danger" aria-label={t(lang, "remove")} onClick={() => setFiles(files.filter((_, k) => k !== i))}>×</button>
          </li>
        ))}
      </ol>
      <div>
        <button className="ghost" onClick={add}>+ {t(lang, "addPictures")}</button>
      </div>
      <label className="choice">
        <input type="radio" checked={layout === "a4"} onChange={() => setLayout("a4")} /> {t(lang, "layoutA4")}
      </label>
      <label className="choice">
        <input type="radio" checked={layout === "fit_picture"} onChange={() => setLayout("fit_picture")} /> {t(lang, "layoutFit")}
      </label>
    </Dialog>
  );
}

/** Which pages: all, the selected ones, or typed ranges. */
function usePages(doc: DocInfo) {
  const tab = useApp((s) => s.tabs.find((x) => x.doc.id === doc.id));
  const [which, setWhich] = useState<"all" | "selected">("all");
  const selected = tab ? targetPages(tab) : [0];
  const pages = which === "all" ? Array.from({ length: doc.page_count }, (_, i) => i) : selected;
  return { which, setWhich, pages, selectedCount: selected.length };
}

function PagesChoice({ p, lang }: { p: ReturnType<typeof usePages>; lang: "bn" | "en" }) {
  return (
    <div className="row tight">
      <label className="choice">
        <input type="radio" checked={p.which === "all"} onChange={() => p.setWhich("all")} /> {t(lang, "allPages")}
      </label>
      <label className="choice">
        <input type="radio" checked={p.which === "selected"} onChange={() => p.setWhich("selected")} /> {t(lang, "selectedPagesChoice", { n: num(lang, p.selectedCount) })}
      </label>
    </div>
  );
}

/** Pages to PNG or JPG pictures (FR-7.3). */
export function ExportDialog({ doc, onClose }: { doc: DocInfo; onClose: () => void }) {
  const lang = useApp((s) => s.lang);
  const notify = useApp((s) => s.notify);
  const p = usePages(doc);
  const [dpi, setDpi] = useState(150);
  const [format, setFormat] = useState<PictureFormat>("png");
  const [busy, setBusy] = useState(false);

  const run = async () => {
    const dir = await openDialog({ directory: true, multiple: false });
    if (typeof dir !== "string") return;
    setBusy(true);
    try {
      const files = await exportPictures(doc.id, p.pages, dpi, format, dir, stemOf(doc.name));
      onClose();
      notify(t(lang, "picturesSaved", { n: num(lang, files.length), folder: dir }));
    } catch (e) {
      await fail(e);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog
      title={t(lang, "exportPictures")}
      onClose={onClose}
      actions={
        <>
          <button className="ghost" onClick={onClose}>{t(lang, "cancel")}</button>
          <button className="primary" disabled={busy || !p.pages.length} onClick={run}>{busy ? t(lang, "working") : t(lang, "chooseFolderSave")}</button>
        </>
      }
    >
      <PagesChoice p={p} lang={lang} />
      <div className="row tight">
        <label className="choice"><input type="radio" checked={format === "png"} onChange={() => setFormat("png")} /> PNG</label>
        <label className="choice"><input type="radio" checked={format === "jpg"} onChange={() => setFormat("jpg")} /> JPG</label>
      </div>
      <label className="choice">
        {t(lang, "quality")}
        <select value={dpi} onChange={(e) => setDpi(Number(e.target.value))}>
          {[72, 96, 150, 200, 300, 600].map((d) => (
            <option key={d} value={d}>{num(lang, d)} dpi{d === 150 ? ` (${t(lang, "recommended")})` : d === 300 ? ` (${t(lang, "forPrint")})` : ""}</option>
          ))}
        </select>
      </label>
    </Dialog>
  );
}

/** A smaller copy (FR-7.4). */
export function CompressDialog({ doc, onClose }: { doc: DocInfo; onClose: () => void }) {
  const lang = useApp((s) => s.lang);
  const notify = useApp((s) => s.notify);
  const [level, setLevel] = useState<Compression>("medium");
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<{ before: number; after: number } | null>(null);

  const run = async () => {
    const dest = await saveDialog({ defaultPath: `${stemOf(doc.name)} (${t(lang, "compressedSuffix")}).pdf`, filters: pdfFilter() });
    if (!dest) return;
    setBusy(true);
    try {
      const r = await compressPdf(doc.id, level, dest);
      setResult(r);
      notify(t(lang, "saved"));
    } catch (e) {
      await fail(e);
    } finally {
      setBusy(false);
    }
  };

  const levels: { id: Compression; key: Key; hint: Key }[] = [
    { id: "low", key: "compressLow", hint: "compressLowHint" },
    { id: "medium", key: "compressMedium", hint: "compressMediumHint" },
    { id: "high", key: "compressHigh", hint: "compressHighHint" },
  ];
  return (
    <Dialog
      title={t(lang, "compress")}
      onClose={onClose}
      actions={
        result ? (
          <button className="primary" onClick={onClose}>{t(lang, "done")}</button>
        ) : (
          <>
            <button className="ghost" onClick={onClose}>{t(lang, "cancel")}</button>
            <button className="primary" disabled={busy} onClick={run}>{busy ? t(lang, "working") : t(lang, "compressSave")}</button>
          </>
        )
      }
    >
      {levels.map((l) => (
        <label key={l.id} className="choice">
          <input type="radio" checked={level === l.id} onChange={() => setLevel(l.id)} disabled={!!result} />
          <b>{t(lang, l.key)}</b> <span className="muted small">{t(lang, l.hint)}</span>
        </label>
      ))}
      <p className="muted small">{t(lang, "compressNote")}</p>
      {result && (
        <p className="status ok">
          {t(lang, "compressResult", {
            before: fileSize(lang, result.before),
            after: fileSize(lang, result.after),
            pct: num(lang, Math.max(0, Math.round((1 - result.after / Math.max(1, result.before)) * 100))),
          })}
        </p>
      )}
    </Dialog>
  );
}

/** Add or remove a password (FR-7.5). */
export function PasswordToolDialog({ doc, onClose }: { doc: DocInfo; onClose: () => void }) {
  const lang = useApp((s) => s.lang);
  const notify = useApp((s) => s.notify);
  const [mode, setMode] = useState<"add" | "remove">("add");
  const [pw, setPw] = useState("");
  const [again, setAgain] = useState("");
  const [busy, setBusy] = useState(false);
  const ok = mode === "remove" || (pw.length >= 4 && pw === again);

  const run = async () => {
    const suffix = mode === "add" ? t(lang, "protectedSuffix") : t(lang, "unprotectedSuffix");
    const dest = await saveDialog({ defaultPath: `${stemOf(doc.name)} (${suffix}).pdf`, filters: pdfFilter() });
    if (!dest) return;
    setBusy(true);
    try {
      if (mode === "add") await protectPdf(doc.id, pw, null, dest);
      else await unprotectPdf(doc.id, dest);
      onClose();
      notify(t(lang, "saved"));
    } catch (e) {
      await fail(e);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog
      title={t(lang, "passwordTool")}
      onClose={onClose}
      actions={
        <>
          <button className="ghost" onClick={onClose}>{t(lang, "cancel")}</button>
          <button className="primary" disabled={!ok || busy} onClick={run}>{busy ? t(lang, "working") : t(lang, "saveCopy")}</button>
        </>
      }
    >
      <div className="seg" role="tablist">
        <button className={mode === "add" ? "on" : ""} onClick={() => setMode("add")}>{t(lang, "addPassword")}</button>
        <button className={mode === "remove" ? "on" : ""} onClick={() => setMode("remove")}>{t(lang, "removePassword")}</button>
      </div>
      {mode === "add" ? (
        <>
          <label className="field">
            {t(lang, "newPassword")}
            <input type="password" value={pw} autoComplete="new-password" onChange={(e) => setPw(e.target.value)} autoFocus />
          </label>
          <label className="field">
            {t(lang, "repeatPassword")}
            <input type="password" value={again} autoComplete="new-password" onChange={(e) => setAgain(e.target.value)} />
          </label>
          {pw.length > 0 && pw.length < 4 && <p className="error small">{t(lang, "passwordShort")}</p>}
          {again.length > 0 && pw !== again && <p className="error small">{t(lang, "passwordMismatch")}</p>}
          <p className="muted small">🔒 {t(lang, "passwordAddNote")}</p>
        </>
      ) : (
        <p className="muted">{t(lang, "passwordRemoveNote")}</p>
      )}
    </Dialog>
  );
}

type DecoKind = "numbers" | "header" | "footer" | "watermark";

/** Page numbers, header, footer or watermark (FR-7.6). */
export function DecorateDialog({ doc, kind, onClose }: { doc: DocInfo; kind: DecoKind; onClose: () => void }) {
  const lang = useApp((s) => s.lang);
  const setDoc = useApp((s) => s.setDoc);
  const p = usePages(doc);
  const defaults: Record<DecoKind, string> = {
    numbers: lang === "bn" ? "পৃষ্ঠা {n} / {total}" : "Page {n} of {total}",
    header: "",
    footer: "",
    watermark: lang === "bn" ? "গোপনীয়" : "CONFIDENTIAL",
  };
  const [text, setText] = useState(defaults[kind]);
  const [align, setAlign] = useState<"left" | "center" | "right">(kind === "numbers" ? "center" : kind === "header" ? "left" : "center");
  const [size, setSize] = useState(kind === "watermark" ? 64 : 10);
  const [color, setColor] = useState(kind === "watermark" ? "#c62828" : "#333333");
  const [bnDigits, setBnDigits] = useState(lang === "bn");
  const [first, setFirst] = useState(1);
  const [opacity, setOpacity] = useState(25);
  const [rotation, setRotation] = useState(-45);
  const [busy, setBusy] = useState(false);

  const run = async () => {
    setBusy(true);
    try {
      const updated = await decoratePages(
        doc.id,
        {
          text,
          spot: kind === "header" ? "top" : kind === "watermark" ? "middle" : "bottom",
          margin: 28,
          rotation: kind === "watermark" ? rotation : 0,
          opacity: kind === "watermark" ? opacity / 100 : 1,
          bangla_digits: bnDigits,
          pages: p.which === "all" ? [] : p.pages,
          first_number: first,
        },
        {
          bangla_font: "Noto Sans Bengali",
          english_font: SANS,
          size,
          line_height: null,
          bold: kind === "watermark",
          italic: false,
          color: hexToRgb(color),
          align: kind === "watermark" ? "center" : align,
        },
      );
      setDoc(updated);
      onClose();
    } catch (e) {
      await fail(e);
    } finally {
      setBusy(false);
    }
  };

  const titles: Record<DecoKind, Key> = { numbers: "pageNumbers", header: "header", footer: "footer", watermark: "watermark" };
  return (
    <Dialog
      title={t(lang, titles[kind])}
      onClose={onClose}
      actions={
        <>
          <button className="ghost" onClick={onClose}>{t(lang, "cancel")}</button>
          <button className="primary" disabled={busy || !text.trim()} onClick={run}>{busy ? t(lang, "working") : t(lang, "addToPages")}</button>
        </>
      }
    >
      <label className="field">
        {t(lang, "decoText")}
        <input type="text" value={text} onChange={(e) => setText(e.target.value)} placeholder={t(lang, kind === "header" ? "headerHint" : "footerHint")} autoFocus />
      </label>
      {kind !== "watermark" && <p className="muted small">{t(lang, "placeholdersHint")}</p>}
      <div className="row tight">
        {kind !== "watermark" &&
          (["left", "center", "right"] as const).map((a) => (
            <label key={a} className="choice">
              <input type="radio" checked={align === a} onChange={() => setAlign(a)} /> {t(lang, a === "left" ? "alignLeft" : a === "center" ? "alignCenter" : "alignRight")}
            </label>
          ))}
      </div>
      <div className="row tight">
        <label className="choice">
          {t(lang, "fontSize")} <input type="number" className="small-num" min={4} max={200} value={size} onChange={(e) => setSize(Math.max(4, Number(e.target.value) || size))} /> pt
        </label>
        <label className="choice">
          {t(lang, "textColour")} <input type="color" value={color} onChange={(e) => setColor(rgbToHex(hexToRgb(e.target.value)))} />
        </label>
        {kind !== "watermark" && (
          <label className="choice">
            <input type="checkbox" checked={bnDigits} onChange={(e) => setBnDigits(e.target.checked)} /> {t(lang, "banglaDigits")}
          </label>
        )}
        {kind === "numbers" && (
          <label className="choice">
            {t(lang, "startAt")} <input type="number" className="small-num" min={0} value={first} onChange={(e) => setFirst(Number(e.target.value) || 0)} />
          </label>
        )}
      </div>
      {kind === "watermark" && (
        <div className="row tight">
          <label className="choice">
            {t(lang, "opacity")} <input type="range" min={5} max={100} value={opacity} onChange={(e) => setOpacity(Number(e.target.value))} /> {num(lang, opacity)}%
          </label>
          <label className="choice">
            {t(lang, "angle")}
            <select value={rotation} onChange={(e) => setRotation(Number(e.target.value))}>
              {[-45, 0, 45, -90].map((r) => (
                <option key={r} value={r}>{num(lang, r)}°</option>
              ))}
            </select>
          </label>
        </div>
      )}
      <PagesChoice p={p} lang={lang} />
      <p className="muted small">{t(lang, "undoHint")}</p>
    </Dialog>
  );
}


/** Make scanned pages searchable with OCR (SRS Module 10). */
export function OcrDialog({ doc, onClose }: { doc: DocInfo; onClose: () => void }) {
  const lang = useApp((s) => s.lang);
  const setDoc = useApp((s) => s.setDoc);
  const notify = useApp((s) => s.notify);
  const p = usePages(doc);
  const [status, setStatus] = useState<OcrStatus | null>(null);
  const [dpi, setDpi] = useState(300);
  const [skipText, setSkipText] = useState(true);
  const [progress, setProgress] = useState<{ done: number; total: number } | null>(null);
  const stop = useRef(false);

  useEffect(() => {
    ocrStatus().then(setStatus).catch(() => setStatus({ datadir: null, languages: [] }));
  }, []);

  const run = async () => {
    stop.current = false;
    let todo = p.pages;
    if (skipText) {
      const kept: number[] = [];
      for (const page of todo) {
        const lines = await pageText(doc.id, page).catch(() => []);
        if (lines.length === 0) kept.push(page);
      }
      todo = kept;
    }
    if (!todo.length) {
      await message(t(lang, "ocrNothingToDo"), { kind: "info" });
      return;
    }
    setProgress({ done: 0, total: todo.length });
    try {
      for (let i = 0; i < todo.length; i++) {
        if (stop.current) break;
        setDoc(await ocrPage(doc.id, todo[i], dpi));
        setProgress({ done: i + 1, total: todo.length });
      }
      notify(t(lang, "ocrDone"));
      onClose();
    } catch (e) {
      await fail(e);
      setProgress(null);
    }
  };

  const ready = !!status?.datadir;
  return (
    <Dialog
      title={t(lang, "ocrTitle")}
      onClose={() => !progress && onClose()}
      actions={
        progress ? (
          <button className="ghost" onClick={() => (stop.current = true)}>{t(lang, "stop")}</button>
        ) : (
          <>
            <button className="ghost" onClick={onClose}>{t(lang, "cancel")}</button>
            <button className="primary" disabled={!ready} onClick={run}>{t(lang, "ocrStart")}</button>
          </>
        )
      }
    >
      {status === null ? (
        <p className="muted">…</p>
      ) : !ready ? (
        <>
          <p className="error">{t(lang, "ocrMissing")}</p>
          <p className="muted small">{t(lang, "ocrMissingHow")}</p>
          <code className="copyable">https://github.com/shaonkhalifa/akkhor-pdf-editor/releases</code>
        </>
      ) : progress ? (
        <>
          <p>{t(lang, "ocrWorking", { done: num(lang, progress.done), total: num(lang, progress.total) })}</p>
          <progress max={progress.total} value={progress.done} />
        </>
      ) : (
        <>
          <p className="muted small">{t(lang, "ocrIntro")}</p>
          <p className="muted small">✓ {t(lang, "ocrLanguages")}: {status.languages.join(", ")}</p>
          <PagesChoice p={p} lang={lang} />
          <label className="choice">
            <input type="checkbox" checked={skipText} onChange={(e) => setSkipText(e.target.checked)} /> {t(lang, "ocrSkipText")}
          </label>
          <label className="choice">
            {t(lang, "quality")}
            <select value={dpi} onChange={(e) => setDpi(Number(e.target.value))}>
              {[200, 300, 400].map((d) => (
                <option key={d} value={d}>{num(lang, d)} dpi{d === 300 ? ` (${t(lang, "recommended")})` : ""}</option>
              ))}
            </select>
          </label>
          <p className="muted small">{t(lang, "ocrNote")}</p>
        </>
      )}
    </Dialog>
  );
}
