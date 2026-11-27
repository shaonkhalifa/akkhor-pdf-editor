// Editing text in place (SRS 3.6): an edit box over the old text, with the
// font, size, colour, bold, italic and alignment controls (FR-6.4).
// Ctrl+Enter applies, Escape cancels. The box can be widened by dragging its
// right edge; text wraps inside it (FR-6.12).
import { useEffect, useRef, useState } from "react";
import type { FontOption } from "../api";
import { num, t } from "../i18n";
import { useApp, type Editing } from "../store";
import { applyEditing, availableFonts, cancelEditing, hexToRgb, rgbToHex, updateEditing, updateStyle } from "../textTools";

/** A font the edit box can show while typing, close to the PDF font. */
function previewFamily(english: string, bangla: string): string {
  const en = english.startsWith("Serif") ? '"Times New Roman", serif' : english.startsWith("Mono") ? '"Courier New", monospace' : english.startsWith("Sans") ? "Arial, sans-serif" : `"${english}", Arial`;
  return `${en}, "${bangla}", "Nirmala UI", sans-serif`;
}

export default function TextEditor({ docId, editing, scale, pageSize }: {
  docId: number;
  editing: Editing;
  scale: number;
  pageSize: [number, number];
}) {
  const lang = useApp((s) => s.lang);
  const [fonts, setFonts] = useState<FontOption[]>([]);
  const [busy, setBusy] = useState(false);
  const areaRef = useRef<HTMLTextAreaElement>(null);
  const resize = useRef<{ x: number; width: number } | null>(null);
  const barRef = useRef<HTMLDivElement>(null);
  const [boxH, setBoxH] = useState(0);
  const [barH, setBarH] = useState(84);
  const { style, block } = editing;
  const lh = style.line_height ?? style.size * 1.25;

  useEffect(() => {
    availableFonts().then(setFonts);
    // Put the cursor in the box (again on the next frame, in case the click
    // that opened it moved the focus elsewhere).
    const focus = () => {
      const el = areaRef.current;
      if (el && document.activeElement !== el) {
        el.focus({ preventScroll: true });
        el.setSelectionRange(el.value.length, el.value.length);
      }
    };
    focus();
    const frame = requestAnimationFrame(focus);
    return () => cancelAnimationFrame(frame);
  }, []);

  useEffect(() => {
    if (barRef.current) setBarH(barRef.current.offsetHeight);
  });

  // Grow the box with its content.
  useEffect(() => {
    const el = areaRef.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${el.scrollHeight + 2}px`;
    setBoxH(el.offsetHeight);
  }, [editing.text, style.size, lh, editing.width, scale]);

  const apply = async () => {
    setBusy(true);
    try {
      await applyEditing(docId);
    } finally {
      setBusy(false);
    }
  };

  const top = block ? block.rect[1] - (lh - style.size) / 2 : editing.top;
  // The style bar sits above the box, or below it when there is no room
  // above (near the top of the page), so it never covers the text.
  const pageW = pageSize[0] * scale;
  const barAbove = top * scale - barH - 8 >= 0;
  const barTop = barAbove ? top * scale - barH - 8 : top * scale + Math.max(boxH, lh * scale) + 8;
  const barLeft = Math.max(4, Math.min(editing.left * scale, pageW - 8 - Math.min(600, pageW - 8)));
  const bangla = fonts.filter((f) => f.script === "bangla");
  const english = fonts.filter((f) => f.script === "english");
  const withCurrent = (list: FontOption[], name: string) => (list.some((f) => f.name === name) || !name ? list : [{ name, script: "english", source: "system", serif: false } as FontOption, ...list]);

  return (
    <>
      {block && (
        // Hide the old text while it is being edited.
        <div
          className="edit-cover"
          style={{ left: block.rect[0] * scale - 2, top: block.rect[1] * scale - 2, width: (block.rect[2] - block.rect[0]) * scale + 4, height: (block.rect[3] - block.rect[1]) * scale + 4 }}
        />
      )}
      <div className="text-edit" style={{ left: editing.left * scale, top: top * scale, width: editing.width * scale }}>
        <textarea
          ref={areaRef}
          value={editing.text}
          spellCheck={false}
          aria-label={t(lang, "editTextLabel")}
          placeholder={t(lang, "typeHere")}
          style={{
            fontSize: style.size * scale,
            lineHeight: `${lh * scale}px`,
            fontFamily: previewFamily(style.english_font, style.bangla_font),
            fontWeight: style.bold ? 700 : 400,
            fontStyle: style.italic ? "italic" : "normal",
            color: rgbToHex(style.color),
            textAlign: style.align,
          }}
          onChange={(e) => updateEditing(docId, { text: e.target.value })}
          onKeyDown={(e) => {
            if (e.key === "Escape") {
              e.preventDefault();
              cancelEditing(docId);
            } else if (e.key === "Enter" && e.ctrlKey) {
              e.preventDefault();
              apply();
            }
          }}
        />
        <span
          className="width-handle"
          title={t(lang, "boxWidth")}
          onPointerDown={(e) => {
            (e.target as HTMLElement).setPointerCapture(e.pointerId);
            resize.current = { x: e.clientX, width: editing.width };
          }}
          onPointerMove={(e) => {
            const r = resize.current;
            if (!r) return;
            const w = Math.max(20, Math.min(pageSize[0] - editing.left, r.width + (e.clientX - r.x) / scale));
            updateEditing(docId, { width: w });
          }}
          onPointerUp={() => (resize.current = null)}
        />
      </div>

      <div
        ref={barRef}
        className="text-style-bar"
        style={{ left: barLeft, top: barTop, maxWidth: Math.max(260, Math.min(600, pageW - 8)) }}
        onPointerDown={(e) => e.stopPropagation()}
      >
        <div className="row tight">
          <select aria-label={t(lang, "banglaFont")} title={t(lang, "banglaFont")} value={style.bangla_font} onChange={(e) => updateStyle(docId, { bangla_font: e.target.value })}>
            {withCurrent(bangla, style.bangla_font).map((f) => (
              <option key={f.name} value={f.name}>
                {f.name}
                {f.source === "system" ? " ✓" : ""}
              </option>
            ))}
          </select>
          <select aria-label={t(lang, "englishFont")} title={t(lang, "englishFont")} value={style.english_font} onChange={(e) => updateStyle(docId, { english_font: e.target.value })}>
            {withCurrent(english, style.english_font).map((f) => (
              <option key={f.name} value={f.name}>
                {f.name}
              </option>
            ))}
          </select>
          <label className="size-field" title={t(lang, "fontSize")}>
            <input
              type="number"
              min={4}
              max={200}
              step={0.5}
              value={style.size}
              aria-label={t(lang, "fontSize")}
              onChange={(e) => {
                const size = Math.max(2, Number(e.target.value) || style.size);
                // Keep the spacing in proportion when the size changes.
                updateStyle(docId, { size, line_height: style.line_height ? (style.line_height * size) / style.size : null });
              }}
            />
            pt
          </label>
          <input type="color" title={t(lang, "textColour")} aria-label={t(lang, "textColour")} value={rgbToHex(style.color)} onChange={(e) => updateStyle(docId, { color: hexToRgb(e.target.value) })} />
          <button className={style.bold ? "tool on" : "tool"} aria-pressed={style.bold} title={t(lang, "bold")} onClick={() => updateStyle(docId, { bold: !style.bold })}>
            <b>B</b>
          </button>
          <button className={style.italic ? "tool on" : "tool"} aria-pressed={style.italic} title={t(lang, "italic")} onClick={() => updateStyle(docId, { italic: !style.italic })}>
            <i>I</i>
          </button>
          {(["left", "center", "right"] as const).map((a) => (
            <button key={a} className={style.align === a ? "tool on" : "tool"} aria-pressed={style.align === a} title={t(lang, a === "left" ? "alignLeft" : a === "center" ? "alignCenter" : "alignRight")} onClick={() => updateStyle(docId, { align: a })}>
              {a === "left" ? "⇤" : a === "center" ? "↔" : "⇥"}
            </button>
          ))}
        </div>
        <div className="row tight between">
          <span className="muted small">
            {block && !block.readable ? (
              <span className="warn">⚠ {t(lang, "unreadableText")}</span>
            ) : block ? (
              `${t(lang, "originalFont")}: ${block.font || "?"} · ${num(lang, Math.round(block.size * 10) / 10)} pt`
            ) : (
              t(lang, "newBoxHint")
            )}
          </span>
          <span className="row tight">
            <button className="ghost" onClick={() => cancelEditing(docId)}>
              {t(lang, "cancel")}
            </button>
            <button className="primary" disabled={busy} onClick={apply} title="Ctrl+Enter">
              {busy ? t(lang, "working") : t(lang, "apply")}
            </button>
          </span>
        </div>
      </div>
    </>
  );
}
