// Positioning a picture on a page before placing it (FR-4.2, FR-5.6):
// drag to move, drag the corner to resize (keeping its shape), drag the round
// handle to turn it. Enter places it, Escape cancels, arrow keys nudge it.
import { useEffect, useRef } from "react";
import { num, t } from "../i18n";
import { cancelPlacing, commitPlacing, updatePlacing } from "../insertTools";
import { useApp, type Placing } from "../store";

type Mode = "move" | "resize" | "rotate";

export default function PlacementOverlay({ docId, placing, scale, pageSize }: {
  docId: number;
  placing: Placing;
  scale: number;
  pageSize: [number, number];
}) {
  const lang = useApp((s) => s.lang);
  const boxRef = useRef<HTMLDivElement>(null);
  const drag = useRef<{ mode: Mode; start: [number, number]; center: [number, number] } | null>(null);
  const { center, size, rotation, opacity } = placing;

  // Pointer position in page points.
  const toPage = (e: { clientX: number; clientY: number }): [number, number] => {
    const page = boxRef.current?.parentElement?.getBoundingClientRect();
    if (!page) return [0, 0];
    return [(e.clientX - page.left) / scale, (e.clientY - page.top) / scale];
  };

  const begin = (mode: Mode) => (e: React.PointerEvent) => {
    if (e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    (e.target as HTMLElement).setPointerCapture(e.pointerId);
    drag.current = { mode, start: toPage(e), center };
  };

  const move = (e: React.PointerEvent) => {
    const d = drag.current;
    if (!d) return;
    const [x, y] = toPage(e);
    if (d.mode === "move") {
      const cx = Math.min(pageSize[0], Math.max(0, d.center[0] + x - d.start[0]));
      const cy = Math.min(pageSize[1], Math.max(0, d.center[1] + y - d.start[1]));
      updatePlacing(docId, { center: [cx, cy] });
    } else if (d.mode === "resize") {
      // Distance from the centre, measured along the picture's own axes.
      const a = (-rotation * Math.PI) / 180;
      const dx = x - center[0];
      const dy = y - center[1];
      const lx = Math.abs(dx * Math.cos(a) - dy * Math.sin(a));
      const ly = Math.abs(dx * Math.sin(a) + dy * Math.cos(a));
      const w = Math.max(12, Math.min(pageSize[0] * 2, Math.max(lx * 2, ly * 2 * placing.aspect)));
      updatePlacing(docId, { size: [w, w / placing.aspect] });
    } else {
      let angle = (Math.atan2(y - center[1], x - center[0]) * 180) / Math.PI + 90;
      angle = ((angle % 360) + 360) % 360;
      // Snap to straight angles when close, or to 15° steps with Shift.
      const snapped = Math.round(angle / 90) * 90;
      if (Math.abs(snapped - angle) < 4) angle = snapped % 360;
      else if (e.shiftKey) angle = Math.round(angle / 15) * 15;
      updatePlacing(docId, { rotation: Math.round(angle * 10) / 10 });
    }
  };

  const end = () => {
    drag.current = null;
  };

  useEffect(() => {
    boxRef.current?.focus({ preventScroll: true });
  }, []);

  const onKey = (e: React.KeyboardEvent) => {
    const step = e.shiftKey ? 10 : 1;
    const nudge: Record<string, [number, number]> = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, -step], ArrowDown: [0, step] };
    if (e.key === "Enter") {
      e.preventDefault();
      commitPlacing(docId);
    } else if (e.key === "Escape") {
      e.preventDefault();
      cancelPlacing(docId);
    } else if (nudge[e.key]) {
      e.preventDefault();
      updatePlacing(docId, { center: [center[0] + nudge[e.key][0], center[1] + nudge[e.key][1]] });
    }
  };

  const w = size[0] * scale;
  const h = size[1] * scale;
  return (
    <>
      <div
        ref={boxRef}
        className="placing"
        tabIndex={0}
        role="group"
        aria-label={t(lang, "placeHint")}
        style={{ left: center[0] * scale - w / 2, top: center[1] * scale - h / 2, width: w, height: h, transform: `rotate(${rotation}deg)` }}
        onPointerDown={begin("move")}
        onPointerMove={move}
        onPointerUp={end}
        onKeyDown={onKey}
      >
        <img src={placing.dataUrl} alt="" draggable={false} style={{ opacity }} />
        <span className="rotate-stem" />
        <span className="handle rotate" title={t(lang, "rotateHandle")} onPointerDown={begin("rotate")} onPointerMove={move} onPointerUp={end} />
        <span className="handle resize" title={t(lang, "resizeHandle")} onPointerDown={begin("resize")} onPointerMove={move} onPointerUp={end} />
      </div>
      <div
        className="placing-bar"
        style={{ left: Math.max(4, center[0] * scale - 160), top: Math.max(4, center[1] * scale + Math.max(w, h) / 2 + 14) }}
        onPointerDown={(e) => e.stopPropagation()}
        onKeyDown={onKey}
      >
        <label title={t(lang, "opacity")}>
          {t(lang, "opacity")}
          <input
            type="range"
            min={10}
            max={100}
            value={Math.round(opacity * 100)}
            onChange={(e) => updatePlacing(docId, { opacity: Number(e.target.value) / 100 })}
          />
          <span className="num">{num(lang, Math.round(opacity * 100))}%</span>
        </label>
        <button className="tool" title={t(lang, "rotateLeft")} onClick={() => updatePlacing(docId, { rotation: (rotation + 270) % 360 })}>
          ↺
        </button>
        <button className="tool" title={t(lang, "rotateRight")} onClick={() => updatePlacing(docId, { rotation: (rotation + 90) % 360 })}>
          ↻
        </button>
        <button className="ghost" onClick={() => cancelPlacing(docId)}>
          {t(lang, "cancel")}
        </button>
        <button className="primary" onClick={() => commitPlacing(docId)}>
          {t(lang, "place")} ↵
        </button>
      </div>
    </>
  );
}
