// Make or choose a signature (SRS Module 5): draw it with the mouse, pen or
// touchpad, type a name in a handwriting font, or use a photo of a signature
// on paper. Signatures can be saved, encrypted, in "My Signatures".
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { message, open as openDialog } from "@tauri-apps/plugin-dialog";
import { deleteSignature, listSignatures, photoSignature, saveSignature, type SavedSignature } from "../api";
import { imageToCanvas, loadSignatureFonts, SIGNATURE_FONTS, today, trim, typedSignature, withCaption } from "../canvasArt";
import { t } from "../i18n";
import { showInsertError, startPlacing } from "../insertTools";
import { useApp } from "../store";
import { choose } from "./ChoiceDialog";

type Tab = "draw" | "type" | "photo" | "saved";
type Point = [number, number, number]; // x, y, pressure

const INKS = [
  { color: "#111111", key: "inkBlack" },
  { color: "#1a3fb0", key: "inkBlue" },
] as const;

const PAD_W = 560;
const PAD_H = 200;

function hexToRgb(hex: string): [number, number, number] {
  const n = parseInt(hex.slice(1), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

/** The drawing pad. Strokes are kept as points so the ink colour can change. */
function DrawPad({ strokes, setStrokes, color }: { strokes: Point[][]; setStrokes: (s: Point[][]) => void; color: string }) {
  const ref = useRef<HTMLCanvasElement>(null);
  const current = useRef<Point[] | null>(null);
  const dpr = window.devicePixelRatio || 1;

  const draw = (all: Point[][]) => {
    const c = ref.current;
    if (!c) return;
    const ctx = c.getContext("2d")!;
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.clearRect(0, 0, c.width, c.height);
    ctx.setTransform(dpr * 2, 0, 0, dpr * 2, 0, 0);
    ctx.strokeStyle = ctx.fillStyle = color;
    ctx.lineCap = ctx.lineJoin = "round";
    for (const s of all) {
      if (s.length === 1) {
        ctx.beginPath();
        ctx.arc(s[0][0], s[0][1], 1.4, 0, Math.PI * 2);
        ctx.fill();
        continue;
      }
      // Smooth curves through the midpoints; pen pressure changes the width.
      for (let i = 1; i < s.length; i++) {
        const [x0, y0] = i > 1 ? [(s[i - 2][0] + s[i - 1][0]) / 2, (s[i - 2][1] + s[i - 1][1]) / 2] : [s[0][0], s[0][1]];
        const [x1, y1] = [(s[i - 1][0] + s[i][0]) / 2, (s[i - 1][1] + s[i][1]) / 2];
        ctx.lineWidth = 1.6 + 2.2 * s[i][2];
        ctx.beginPath();
        ctx.moveTo(x0, y0);
        ctx.quadraticCurveTo(s[i - 1][0], s[i - 1][1], x1, y1);
        ctx.stroke();
      }
    }
  };

  useLayoutEffect(() => draw(strokes));

  const point = (e: React.PointerEvent): Point => {
    const r = ref.current!.getBoundingClientRect();
    const pressure = e.pointerType === "pen" && e.pressure > 0 ? e.pressure : 0.5;
    return [e.clientX - r.left, e.clientY - r.top, pressure];
  };

  return (
    <canvas
      ref={ref}
      className="sign-pad"
      width={PAD_W * dpr * 2}
      height={PAD_H * dpr * 2}
      style={{ width: PAD_W, height: PAD_H }}
      onPointerDown={(e) => {
        (e.target as HTMLElement).setPointerCapture(e.pointerId);
        current.current = [point(e)];
        draw([...strokes, current.current]);
      }}
      onPointerMove={(e) => {
        if (!current.current) return;
        current.current.push(point(e));
        draw([...strokes, current.current]);
      }}
      onPointerUp={() => {
        if (current.current) setStrokes([...strokes, current.current]);
        current.current = null;
      }}
    />
  );
}

export default function SignatureDialog({ docId, onClose }: { docId: number; onClose: () => void }) {
  const lang = useApp((s) => s.lang);
  const notify = useApp((s) => s.notify);
  const [tab, setTab] = useState<Tab>("draw");
  const [color, setColor] = useState<string>(INKS[0].color);
  const [strokes, setStrokes] = useState<Point[][]>([]);
  const [typed, setTyped] = useState("");
  const [font, setFont] = useState<string>(SIGNATURE_FONTS[0].family);
  const [photoPath, setPhotoPath] = useState<string | null>(null);
  const [photo, setPhoto] = useState<string | null>(null);
  const [saved, setSaved] = useState<SavedSignature[]>([]);
  const [addName, setAddName] = useState(false);
  const [name, setName] = useState("");
  const [addDate, setAddDate] = useState(false);
  const [keep, setKeep] = useState(false);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    loadSignatureFonts().catch(() => {});
    listSignatures()
      .then((list) => {
        setSaved(list);
        if (list.length) setTab("saved");
      })
      .catch(() => {});
  }, []);

  // A photo is turned into ink again when the colour changes.
  useEffect(() => {
    if (!photoPath) return;
    let cancelled = false;
    setBusy(true);
    photoSignature(photoPath, hexToRgb(color))
      .then((pic) => !cancelled && setPhoto(pic.data_url))
      .catch((e) => {
        if (!cancelled) {
          setPhoto(null);
          showInsertError(e);
        }
      })
      .finally(() => !cancelled && setBusy(false));
    return () => {
      cancelled = true;
    };
  }, [photoPath, color]);

  const pickPhoto = async () => {
    const path = await openDialog({ multiple: false, directory: false, filters: [{ name: t(lang, "pictureFiles"), extensions: ["png", "jpg", "jpeg", "webp"] }] });
    if (typeof path === "string") setPhotoPath(path);
  };

  /** The signature as drawn, typed or photographed, before the caption. */
  const signatureCanvas = async (): Promise<HTMLCanvasElement | null> => {
    if (tab === "draw") {
      if (!strokes.length) return null;
      const pad = document.querySelector<HTMLCanvasElement>(".sign-pad");
      return pad ? trim(pad, 12) : null;
    }
    if (tab === "type") return typed.trim() ? typedSignature(typed.trim(), font, color) : null;
    if (tab === "photo") return photo ? imageToCanvas(photo) : null;
    return null;
  };

  const use = async (fromSaved?: SavedSignature) => {
    setBusy(true);
    try {
      const sig = fromSaved ? await imageToCanvas(fromSaved.data_url) : await signatureCanvas();
      if (!sig) {
        await message(t(lang, "signatureEmpty"), { kind: "info" });
        return;
      }
      if (keep && !fromSaved) {
        const label = (tab === "type" && typed.trim()) || name.trim() || `${t(lang, "signature")} ${saved.length + 1}`;
        await saveSignature(label, sig.toDataURL("image/png"));
        notify(t(lang, "signatureSaved"));
      }
      const caption = [addName ? name : "", addDate ? today(lang) : ""];
      const final = withCaption(sig, caption, fromSaved ? "#111111" : color);
      startPlacing(docId, final.toDataURL("image/png"), final.width, final.height, 160);
      onClose();
    } catch (e) {
      await showInsertError(e);
    } finally {
      setBusy(false);
    }
  };

  const remove = async (s: SavedSignature) => {
    const answer = await choose({
      title: t(lang, "deleteSignature"),
      message: t(lang, "deleteSignatureAsk", { name: s.label }),
      buttons: [t(lang, "deleteSignature"), t(lang, "cancel")],
    });
    if (answer !== 0) return;
    await deleteSignature(s.id);
    setSaved((list) => list.filter((x) => x.id !== s.id));
  };

  const tabs: { id: Tab; key: Parameters<typeof t>[1] }[] = [
    { id: "draw", key: "sigDraw" },
    { id: "type", key: "sigType" },
    { id: "photo", key: "sigPhoto" },
    { id: "saved", key: "sigSaved" },
  ];

  return (
    <div className="overlay" role="dialog" aria-modal="true" aria-labelledby="sig-title" onKeyDown={(e) => e.key === "Escape" && onClose()}>
      <div className="dialog sign-dialog">
        <h2 id="sig-title">{t(lang, "signature")}</h2>
        <div className="seg" role="tablist">
          {tabs.map((x) => (
            <button key={x.id} role="tab" aria-selected={tab === x.id} className={tab === x.id ? "on" : ""} onClick={() => setTab(x.id)}>
              {t(lang, x.key)}
              {x.id === "saved" && saved.length ? ` (${saved.length})` : ""}
            </button>
          ))}
        </div>

        {tab !== "saved" && (
          <div className="ink-row">
            <span className="muted small">{t(lang, "inkColour")}</span>
            {INKS.map((ink) => (
              <button
                key={ink.color}
                className={color === ink.color ? "swatch on" : "swatch"}
                style={{ background: ink.color }}
                title={t(lang, ink.key)}
                aria-label={t(lang, ink.key)}
                aria-pressed={color === ink.color}
                onClick={() => setColor(ink.color)}
              />
            ))}
            <label className="swatch custom" title={t(lang, "inkCustom")}>
              <input type="color" value={color} onChange={(e) => setColor(e.target.value)} aria-label={t(lang, "inkCustom")} />
            </label>
          </div>
        )}

        {tab === "draw" && (
          <div className="sign-area">
            <DrawPad strokes={strokes} setStrokes={setStrokes} color={color} />
            <div className="sign-line" aria-hidden="true" />
            <div className="row between">
              <span className="muted small">{t(lang, "drawHint")}</span>
              <span>
                <button className="ghost" disabled={!strokes.length} onClick={() => setStrokes(strokes.slice(0, -1))}>
                  {t(lang, "undo")}
                </button>{" "}
                <button className="ghost" disabled={!strokes.length} onClick={() => setStrokes([])}>
                  {t(lang, "clear")}
                </button>
              </span>
            </div>
          </div>
        )}

        {tab === "type" && (
          <div className="sign-area">
            <input type="text" className="wide-input" placeholder={t(lang, "typeYourName")} value={typed} onChange={(e) => setTyped(e.target.value)} autoFocus />
            <div className="font-choices">
              {SIGNATURE_FONTS.map((f) => (
                <button key={f.family} className={font === f.family ? "font-choice on" : "font-choice"} onClick={() => setFont(f.family)}>
                  <span style={{ fontFamily: `"${f.family}", "Nirmala UI"`, color }}>{typed.trim() || t(lang, "typeYourName")}</span>
                  <small>{f.label}</small>
                </button>
              ))}
            </div>
          </div>
        )}

        {tab === "photo" && (
          <div className="sign-area">
            <p className="muted small">{t(lang, "photoHint")}</p>
            <div className="photo-box">{photo ? <img src={photo} alt={t(lang, "signature")} /> : <span className="muted">{busy ? t(lang, "working") : ""}</span>}</div>
            <button className="ghost" onClick={pickPhoto}>
              {photoPath ? t(lang, "otherPhoto") : t(lang, "choosePhoto")}
            </button>
          </div>
        )}

        {tab === "saved" && (
          <div className="sign-area">
            {saved.length === 0 ? (
              <p className="muted">{t(lang, "noSavedSignatures")}</p>
            ) : (
              <ul className="saved-sigs">
                {saved.map((s) => (
                  <li key={s.id}>
                    <button className="saved-sig" onClick={() => use(s)} disabled={busy} title={t(lang, "useSignature")}>
                      <img src={s.data_url} alt={s.label} />
                      <span>{s.label}</span>
                    </button>
                    <button className="tool danger" title={t(lang, "deleteSignature")} aria-label={`${t(lang, "deleteSignature")} ${s.label}`} onClick={() => remove(s)}>
                      ×
                    </button>
                  </li>
                ))}
              </ul>
            )}
            <p className="muted small">🔒 {t(lang, "savedPrivacy")}</p>
          </div>
        )}

        <div className="caption-opts">
          <label className="choice">
            <input type="checkbox" checked={addName} onChange={(e) => setAddName(e.target.checked)} />
            {t(lang, "addName")}
            {addName && <input type="text" value={name} placeholder={t(lang, "yourName")} onChange={(e) => setName(e.target.value)} />}
          </label>
          <label className="choice">
            <input type="checkbox" checked={addDate} onChange={(e) => setAddDate(e.target.checked)} />
            {t(lang, "addDate")} <span className="muted small">({today(lang)})</span>
          </label>
          {tab !== "saved" && (
            <label className="choice">
              <input type="checkbox" checked={keep} onChange={(e) => setKeep(e.target.checked)} />
              {t(lang, "keepSignature")}
            </label>
          )}
        </div>

        <div className="row end">
          <button className="ghost" onClick={onClose}>
            {t(lang, "cancel")}
          </button>
          {tab !== "saved" && (
            <button className="primary" disabled={busy} onClick={() => use()}>
              {t(lang, "placeOnPage")}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
