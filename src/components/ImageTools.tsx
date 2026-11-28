// Image tools (SRS Module 14): resize pictures, convert JPG / PNG / WebP,
// shrink to a file size (for online forms), and make a plain background
// transparent. Works on picture files directly; everything stays on this PC.
import { useEffect, useRef, useState } from "react";
import { message, open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import {
  backgroundPreview,
  batchImages,
  imageColorAt,
  imageFile,
  imagePreview,
  saveBackground,
  saveImage,
  type BackgroundJob,
  type ImageFile,
  type ImageJob,
  type ImageResult,
  type OutFormat,
} from "../api";
import { fileSize, num, t, type Key } from "../i18n";
import { baseName } from "../open";
import { useApp } from "../store";
import { hexToRgb, rgbToHex } from "../textTools";

const PRESETS: { key: Key; w: number; h: number; crop: boolean }[] = [
  { key: "presetPassport", w: 300, h: 300, crop: true },
  { key: "presetSignature", w: 300, h: 80, crop: false },
  { key: "preset600", w: 600, h: 600, crop: false },
  { key: "presetHd", w: 1920, h: 1080, crop: false },
];

function sizeText(lang: "bn" | "en", bytes: number) {
  return bytes < 1024 * 1024 ? `${num(lang, Math.max(1, Math.round(bytes / 1024)))} KB` : fileSize(lang, bytes);
}

function stem(path: string) {
  return baseName(path).replace(/\.[^.]+$/, "");
}

async function fail(e: unknown) {
  const lang = useApp.getState().lang;
  const err = String(e);
  const key: Key | null = err.includes("TARGET_TOO_SMALL")
    ? "targetTooSmall"
    : err.includes("UNSUPPORTED_IMAGE")
      ? "unsupportedImage"
      : err.includes("NO_BACKGROUND_FOUND")
        ? "noBackgroundFound"
        : err.includes("NOTHING_LEFT")
          ? "nothingLeft"
          : null;
  await message(key ? t(lang, key) : err, { kind: "error", title: t(lang, "imageTools") });
}

export default function ImageTools() {
  const lang = useApp((s) => s.lang);
  const setScreen = useApp((s) => s.setScreen);
  const notify = useApp((s) => s.notify);
  const [files, setFiles] = useState<string[]>([]);
  const [current, setCurrent] = useState<string | null>(null);
  const [original, setOriginal] = useState<ImageFile | null>(null);
  const [result, setResult] = useState<ImageResult | null>(null);
  const [mode, setMode] = useState<"resize" | "background">("resize");
  const [busy, setBusy] = useState(false);
  const [job, setJob] = useState<ImageJob>({
    resize: "none",
    width: 300,
    height: 300,
    percent: 50,
    keep_aspect: true,
    crop: false,
    format: "keep",
    quality: 90,
    background: [255, 255, 255],
    target_kb: null,
  });
  const [bg, setBg] = useState<BackgroundJob>({ color: null, tolerance: 18, softness: 8, from_edges: true, trim: false });
  const timer = useRef(0);

  const add = async () => {
    const chosen = await openDialog({ multiple: true, directory: false, filters: [{ name: t(lang, "pictureFiles"), extensions: ["png", "jpg", "jpeg", "webp"] }] });
    const list = Array.isArray(chosen) ? chosen : chosen ? [chosen] : [];
    if (!list.length) return;
    setFiles((f) => [...f, ...list.filter((p) => !f.includes(p))]);
    if (!current) setCurrent(list[0]);
  };

  // The original picture.
  useEffect(() => {
    setOriginal(null);
    setResult(null);
    if (!current) return;
    imageFile(current).then(setOriginal).catch(fail);
  }, [current]);

  // The result preview, a moment after the settings stop changing.
  useEffect(() => {
    if (!current || !original) return;
    window.clearTimeout(timer.current);
    let cancelled = false;
    timer.current = window.setTimeout(() => {
      setBusy(true);
      (mode === "resize" ? imagePreview(current, job) : backgroundPreview(current, bg))
        .then((r) => !cancelled && setResult(r))
        .catch((e) => {
          if (!cancelled) {
            setResult(null);
            if (!String(e).includes("TARGET_TOO_SMALL") && !String(e).includes("NO_BACKGROUND_FOUND")) fail(e);
          }
        })
        .finally(() => !cancelled && setBusy(false));
    }, 350);
    return () => {
      cancelled = true;
      window.clearTimeout(timer.current);
    };
  }, [current, original, mode, job, bg]);

  const pickColour = async (e: React.MouseEvent<HTMLImageElement>) => {
    if (mode !== "background" || !current) return;
    const r = e.currentTarget.getBoundingClientRect();
    const colour = await imageColorAt(current, (e.clientX - r.left) / r.width, (e.clientY - r.top) / r.height);
    setBg({ ...bg, color: colour });
  };

  const saveOne = async () => {
    if (!current) return;
    const ext = mode === "background" ? "png" : result?.ext ?? "png";
    const suffix = mode === "background" ? t(lang, "transparentSuffix") : result ? `${result.width}x${result.height}` : "edited";
    const dest = await saveDialog({ defaultPath: `${stem(current)}-${suffix}.${ext}`, filters: [{ name: ext.toUpperCase(), extensions: [ext] }] });
    if (!dest) return;
    setBusy(true);
    try {
      const r = mode === "background" ? await saveBackground(current, bg, dest) : await saveImage(current, job, dest);
      notify(`${t(lang, "saved")} — ${r.width}×${r.height}, ${sizeText(lang, r.bytes)}`);
    } catch (e) {
      await fail(e);
    } finally {
      setBusy(false);
    }
  };

  const saveAll = async () => {
    const dir = await openDialog({ directory: true, multiple: false });
    if (typeof dir !== "string") return;
    setBusy(true);
    try {
      const items = await batchImages(files, job, dir);
      const bad = items.filter((i) => i.error);
      notify(t(lang, "batchDone", { n: num(lang, items.length - bad.length), folder: dir }));
      if (bad.length) await message(bad.map((b) => `${baseName(b.input)}: ${b.error}`).join("\n"), { kind: "warning" });
    } catch (e) {
      await fail(e);
    } finally {
      setBusy(false);
    }
  };

  const formats: { id: OutFormat; label: string }[] = [
    { id: "keep", label: t(lang, "formatKeep") },
    { id: "jpg", label: "JPG" },
    { id: "png", label: "PNG" },
    { id: "webp", label: "WebP" },
  ];
  const jpgOut = job.format === "jpg" || (job.format === "keep" && original?.info.format === "jpg");

  return (
    <div className="scroll-page">
      <div className="image-tools">
        <div className="card-head">
          <h1>{t(lang, "imageTools")}</h1>
          <button className="ghost" onClick={() => setScreen("home")}>← {t(lang, "back")}</button>
        </div>
        <p className="muted small">{t(lang, "imageToolsIntro")}</p>

        <div className="it-body">
          <aside className="panel it-files">
            <button className="primary" onClick={add}>+ {t(lang, "addPictures")}</button>
            <ul>
              {files.map((f) => (
                <li key={f}>
                  <button className={f === current ? "it-file on" : "it-file"} onClick={() => setCurrent(f)} title={f}>
                    {baseName(f)}
                  </button>
                  <button className="tool danger" aria-label={t(lang, "remove")} onClick={() => {
                    const rest = files.filter((x) => x !== f);
                    setFiles(rest);
                    if (current === f) setCurrent(rest[0] ?? null);
                  }}>×</button>
                </li>
              ))}
            </ul>
            {!files.length && <p className="muted small">{t(lang, "noPicturesYet")}</p>}
          </aside>

          <section className="panel it-main">
            <div className="seg" role="tablist">
              <button className={mode === "resize" ? "on" : ""} onClick={() => setMode("resize")}>{t(lang, "resizeConvert")}</button>
              <button className={mode === "background" ? "on" : ""} onClick={() => setMode("background")}>{t(lang, "removeBackground")}</button>
            </div>

            {mode === "resize" ? (
              <div className="it-controls">
                <div className="row tight">
                  {(["none", "pixels", "percent"] as const).map((m) => (
                    <label key={m} className="choice">
                      <input type="radio" checked={job.resize === m} onChange={() => setJob({ ...job, resize: m })} />
                      {t(lang, m === "none" ? "sizeSame" : m === "pixels" ? "sizePixels" : "sizePercent")}
                    </label>
                  ))}
                </div>
                {job.resize === "pixels" && (
                  <>
                    <div className="row tight">
                      <label className="choice">{t(lang, "widthPx")} <input type="number" className="small-num" min={0} value={job.width} onChange={(e) => setJob({ ...job, width: Math.max(0, Number(e.target.value) || 0) })} /></label>
                      <label className="choice">{t(lang, "heightPx")} <input type="number" className="small-num" min={0} value={job.height} onChange={(e) => setJob({ ...job, height: Math.max(0, Number(e.target.value) || 0) })} /></label>
                      <label className="choice"><input type="checkbox" checked={job.keep_aspect} onChange={(e) => setJob({ ...job, keep_aspect: e.target.checked })} /> {t(lang, "keepShape")}</label>
                      {job.keep_aspect && <label className="choice"><input type="checkbox" checked={job.crop} onChange={(e) => setJob({ ...job, crop: e.target.checked })} /> {t(lang, "cropToFill")}</label>}
                    </div>
                    <div className="row tight">
                      {PRESETS.map((p) => (
                        <button key={p.key} className="ghost small-btn" onClick={() => setJob({ ...job, width: p.w, height: p.h, keep_aspect: true, crop: p.crop })}>
                          {t(lang, p.key)}
                        </button>
                      ))}
                    </div>
                  </>
                )}
                {job.resize === "percent" && (
                  <label className="choice">
                    <input type="range" min={5} max={200} value={job.percent} onChange={(e) => setJob({ ...job, percent: Number(e.target.value) })} /> {num(lang, job.percent)}%
                  </label>
                )}
                <div className="row tight">
                  <span>{t(lang, "saveAsFormat")}</span>
                  {formats.map((f) => (
                    <label key={f.id} className="choice"><input type="radio" checked={job.format === f.id} onChange={() => setJob({ ...job, format: f.id })} /> {f.label}</label>
                  ))}
                </div>
                {jpgOut && (
                  <div className="row tight">
                    <label className="choice">{t(lang, "quality")} <input type="range" min={30} max={100} value={job.quality} onChange={(e) => setJob({ ...job, quality: Number(e.target.value) })} /> {num(lang, job.quality)}</label>
                    {original?.info.has_alpha && (
                      <label className="choice">{t(lang, "behindTransparent")} <input type="color" value={rgbToHex(job.background)} onChange={(e) => setJob({ ...job, background: hexToRgb(e.target.value) })} /></label>
                    )}
                  </div>
                )}
                <label className="choice">
                  <input type="checkbox" checked={job.target_kb !== null} onChange={(e) => setJob({ ...job, target_kb: e.target.checked ? 100 : null })} />
                  {t(lang, "targetSize")}
                  {job.target_kb !== null && (
                    <>
                      <input type="number" className="small-num" min={5} value={job.target_kb} onChange={(e) => setJob({ ...job, target_kb: Math.max(1, Number(e.target.value) || 1) })} /> KB
                    </>
                  )}
                </label>
                {job.format === "webp" && <p className="muted small">{t(lang, "webpLossless")}</p>}
              </div>
            ) : (
              <div className="it-controls">
                <p className="muted small">{t(lang, "bgIntro")}</p>
                <div className="row tight">
                  <span>{t(lang, "bgColour")}</span>
                  {bg.color ? (
                    <span className="swatch on" style={{ background: rgbToHex(bg.color) }} title={rgbToHex(bg.color)} />
                  ) : (
                    <span className="muted small">{t(lang, "bgAuto")}</span>
                  )}
                  {bg.color && <button className="link" onClick={() => setBg({ ...bg, color: null })}>{t(lang, "bgUseAuto")}</button>}
                </div>
                <label className="choice">{t(lang, "bgTolerance")} <input type="range" min={1} max={60} value={bg.tolerance} onChange={(e) => setBg({ ...bg, tolerance: Number(e.target.value) })} /> {num(lang, bg.tolerance)}</label>
                <label className="choice">{t(lang, "bgSoftness")} <input type="range" min={0} max={30} value={bg.softness} onChange={(e) => setBg({ ...bg, softness: Number(e.target.value) })} /> {num(lang, bg.softness)}</label>
                <label className="choice"><input type="checkbox" checked={bg.from_edges} onChange={(e) => setBg({ ...bg, from_edges: e.target.checked })} /> {t(lang, "bgFromEdges")}</label>
                <label className="choice"><input type="checkbox" checked={bg.trim} onChange={(e) => setBg({ ...bg, trim: e.target.checked })} /> {t(lang, "bgTrim")}</label>
              </div>
            )}

            <div className="it-previews">
              <figure>
                <figcaption>{t(lang, "originalPicture")}</figcaption>
                {original ? (
                  <img src={original.preview} alt="" className={mode === "background" ? "pickable" : undefined} onClick={pickColour} title={mode === "background" ? t(lang, "bgClickToPick") : undefined} />
                ) : (
                  <div className="it-empty muted">{current ? "…" : t(lang, "noPicturesYet")}</div>
                )}
                {original && <span className="muted small">{num(lang, original.info.width)}×{num(lang, original.info.height)} · {sizeText(lang, original.info.bytes)} · {original.info.format.toUpperCase()}</span>}
              </figure>
              <figure>
                <figcaption>{t(lang, "resultPicture")}{busy ? " …" : ""}</figcaption>
                {result ? <img src={result.preview} alt="" className="checker" /> : <div className="it-empty muted">{current && !busy ? t(lang, "noResult") : ""}</div>}
                {result && (
                  <span className={job.target_kb && result.bytes > job.target_kb * 1024 ? "error small" : "muted small"}>
                    {num(lang, result.width)}×{num(lang, result.height)} · {sizeText(lang, result.bytes)} · {result.ext.toUpperCase()}
                  </span>
                )}
              </figure>
            </div>

            <div className="row end">
              {mode === "resize" && files.length > 1 && (
                <button className="ghost" disabled={busy} onClick={saveAll}>{t(lang, "saveAllFolder", { n: num(lang, files.length) })}</button>
              )}
              <button className="primary" disabled={!result || busy} onClick={saveOne}>{t(lang, "savePicture")}</button>
            </div>
          </section>
        </div>
      </div>
    </div>
  );
}
