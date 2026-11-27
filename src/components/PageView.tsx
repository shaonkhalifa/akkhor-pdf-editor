// One PDF page: the drawn image, an invisible text layer for selecting and
// copying, search highlights and clickable links.
import { memo, useEffect, useLayoutEffect, useRef, useState } from "react";
import { pageImages, pageLinks, pageText, renderPage, textBlocks, type PageImage, type PageLink, type SearchHit, type TextBlock, type TextLine } from "../api";
import { t } from "../i18n";
import { deleteExistingImage, replaceExistingImage } from "../insertTools";
import { useApp, type Editing, type Placing, type Tool } from "../store";
import { editBlock, markArea, newTextBox, redact } from "../textTools";
import PlacementOverlay from "./PlacementOverlay";
import TextEditor from "./TextEditor";

interface Props {
  docId: number;
  /** Document version: pages are redrawn when it changes. */
  version: number;
  index: number;
  /** Page size in points. */
  size: [number, number];
  /** CSS pixels per point. */
  scale: number;
  top: number;
  hits: SearchHit[];
  currentHit: SearchHit | null;
  onGoTo: (page: number) => void;
  /** The picture being positioned, when it is on this page. */
  placing: Placing | null;
  /** What clicking or dragging on the page does. */
  tool: Tool;
  /** Text being edited on this page, if any. */
  editing: Editing | null;
}

const DRAG_TOOLS: Tool[] = ["highlight", "underline", "strike", "redact"];

function PageView({ docId, version, index, size, scale, top, hits, currentHit, onGoTo, placing, tool, editing }: Props) {
  const imageMode = tool === "images";
  const lang = useApp((s) => s.lang);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const textRef = useRef<HTMLDivElement>(null);
  const [drawn, setDrawn] = useState(false);
  const [lines, setLines] = useState<TextLine[] | null>(null);
  const [links, setLinks] = useState<PageLink[]>([]);
  const [images, setImages] = useState<PageImage[]>([]);
  const [picked, setPicked] = useState<number | null>(null);
  const [blocks, setBlocks] = useState<TextBlock[]>([]);
  const [drag, setDrag] = useState<{ x0: number; y0: number; x1: number; y1: number } | null>(null);
  const pageRef = useRef<HTMLDivElement>(null);

  // Text blocks, only while the text tool is on.
  useEffect(() => {
    if (tool !== "text") {
      setBlocks([]);
      return;
    }
    let cancelled = false;
    textBlocks(docId, index)
      .then((list) => !cancelled && setBlocks(list))
      .catch(() => !cancelled && setBlocks([]));
    return () => {
      cancelled = true;
    };
  }, [docId, index, version, tool]);

  const toPage = (e: { clientX: number; clientY: number }) => {
    const r = pageRef.current!.getBoundingClientRect();
    return [(e.clientX - r.left) / scale, (e.clientY - r.top) / scale] as const;
  };

  // Images already on the page, only while the image tool is on.
  useEffect(() => {
    setPicked(null);
    if (!imageMode) {
      setImages([]);
      return;
    }
    let cancelled = false;
    pageImages(docId, index)
      .then((list) => !cancelled && setImages(list))
      .catch(() => !cancelled && setImages([]));
    return () => {
      cancelled = true;
    };
  }, [docId, index, version, imageMode]);
  const width = size[0] * scale;
  const height = size[1] * scale;

  // Draw the page. On zoom the old image stays (stretched) until the new one is ready.
  useEffect(() => {
    let cancelled = false;
    const dpr = window.devicePixelRatio || 1;
    const deviceScale = Math.min(scale * dpr, 6);
    const timer = window.setTimeout(() => {
      renderPage(docId, index, deviceScale, version)
        .then((bmp) => {
          const c = canvasRef.current;
          if (cancelled || !c) {
            bmp.close();
            return;
          }
          c.width = bmp.width;
          c.height = bmp.height;
          c.getContext("2d")?.drawImage(bmp, 0, 0);
          bmp.close();
          setDrawn(true);
        })
        .catch(() => {});
    }, 30);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [docId, index, scale, version]);

  // Text and links do not depend on zoom; load once per version.
  useEffect(() => {
    let cancelled = false;
    pageText(docId, index)
      .then((l) => !cancelled && setLines(l))
      .catch(() => !cancelled && setLines([]));
    pageLinks(docId, index)
      .then((l) => !cancelled && setLinks(l))
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [docId, index, version]);

  // Stretch each invisible text span so it covers its line exactly, turned to
  // the line's writing direction (text on rotated pages runs up or down).
  useLayoutEffect(() => {
    const root = textRef.current;
    if (!root) return;
    for (const el of Array.from(root.children) as HTMLElement[]) {
      const turn = el.dataset.angle && el.dataset.angle !== "0" ? `rotate(${el.dataset.angle}deg) ` : "";
      el.style.transform = "";
      const target = Number(el.dataset.w);
      const natural = el.offsetWidth;
      el.style.transform = natural > 0 && target > 0 ? `${turn}scaleX(${target / natural})` : turn;
    }
  }, [lines, scale]);

  const box = (r: [number, number, number, number]) => ({
    left: r[0] * scale,
    top: r[1] * scale,
    width: (r[2] - r[0]) * scale,
    height: (r[3] - r[1]) * scale,
  });

  return (
    <div className="pdf-page" ref={pageRef} style={{ top, width, height }} data-page={index}>
      <canvas ref={canvasRef} className="pdf-canvas" style={{ width, height }} />
      {!drawn && <div className="page-placeholder">{index + 1}</div>}

      <div className="text-layer" ref={textRef}>
        {lines?.map((l, i) => {
          const [x0, y0, x1, y1] = l.rect.map((v) => v * scale);
          const vertical = l.angle === 90 || l.angle === 270;
          // Line height across the text, length along it.
          const h = vertical ? x1 - x0 : y1 - y0;
          const len = vertical ? y1 - y0 : x1 - x0;
          // The span's top-left corner, before turning, sits at the start of the line.
          const [left, top] = l.angle === 90 ? [x1, y0] : l.angle === 180 ? [x1, y1] : l.angle === 270 ? [x0, y1] : [x0, y0];
          return (
            <span key={i} data-w={len} data-angle={l.angle} style={{ left, top, height: h, fontSize: h * 0.88 }}>
              {l.text}
            </span>
          );
        })}
      </div>

      {hits.map((hit, i) => (
        <div key={i} className={hit === currentHit ? "hit current" : "hit"} style={box(hit.rect)} />
      ))}

      {links.map((link, i) =>
        link.page !== null ? (
          <button
            key={i}
            className="pdf-link"
            style={box(link.rect)}
            aria-label={`${t(lang, "page")} ${link.page + 1}`}
            onClick={() => onGoTo(link.page!)}
          />
        ) : link.uri ? (
          <span key={i} className="pdf-link external" style={box(link.rect)} title={`${t(lang, "externalLink")} ${link.uri}`} />
        ) : null,
      )}

      {imageMode && images.length === 0 && <div className="no-images">{t(lang, "noImagesHere")}</div>}
      {images.map((img, i) => (
        <button
          key={i}
          className={picked === i ? "page-image picked" : "page-image"}
          style={box(img.rect)}
          aria-label={`${t(lang, "imageWord")} ${i + 1}`}
          onClick={() => setPicked(picked === i ? null : i)}
        />
      ))}
      {picked !== null && images[picked] && (
        <div
          className="image-menu"
          style={{ left: images[picked].rect[0] * scale, top: Math.min(images[picked].rect[3] * scale + 6, height - 40) }}
        >
          {images[picked].xref !== null ? (
            <>
              <button onClick={() => replaceExistingImage(docId, index, images[picked].xref!)}>{t(lang, "replaceImage")}</button>
              <button className="danger" onClick={() => deleteExistingImage(docId, index, images[picked].xref!)}>
                {t(lang, "deleteImage")}
              </button>
            </>
          ) : (
            <span className="muted small">{t(lang, "imageNotEditable")}</span>
          )}
        </div>
      )}

      {placing && <PlacementOverlay docId={docId} placing={placing} scale={scale} pageSize={size} />}

      {tool === "text" &&
        !editing &&
        blocks.map((b, i) => (
          <button
            key={i}
            className={b.readable ? "text-block" : "text-block unreadable"}
            style={box([b.rect[0] - 2, b.rect[1] - 2, b.rect[2] + 2, b.rect[3] + 2])}
            title={b.readable ? t(lang, "clickToEdit") : t(lang, "unreadableText")}
            aria-label={`${t(lang, "clickToEdit")}: ${b.text.slice(0, 60)}`}
            onClick={() => editBlock(docId, index, b)}
          />
        ))}

      {(tool === "addtext" || DRAG_TOOLS.includes(tool)) && !editing && (
        <div
          className={`tool-layer ${tool}`}
          onPointerDown={(e) => {
            if (e.button !== 0) return;
            // Stop the browser from moving focus to the page after this
            // click: it would take the focus away from the new text box.
            e.preventDefault();
            const [x, y] = toPage(e);
            if (tool === "addtext") {
              newTextBox(docId, index, x, y);
              return;
            }
            (e.target as HTMLElement).setPointerCapture(e.pointerId);
            setDrag({ x0: x, y0: y, x1: x, y1: y });
          }}
          onPointerMove={(e) => {
            if (!drag) return;
            const [x, y] = toPage(e);
            setDrag({ ...drag, x1: x, y1: y });
          }}
          onPointerUp={() => {
            const d = drag;
            setDrag(null);
            if (!d || Math.abs(d.x1 - d.x0) < 3 || Math.abs(d.y1 - d.y0) < 3) return;
            const rect = [Math.min(d.x0, d.x1), Math.min(d.y0, d.y1), Math.max(d.x0, d.x1), Math.max(d.y0, d.y1)];
            if (tool === "redact") redact(docId, index, rect);
            else markArea(docId, index, tool as "highlight" | "underline" | "strike", rect);
          }}
        >
          {drag && (
            <div
              className={`drag-rect ${tool}`}
              style={box([Math.min(drag.x0, drag.x1), Math.min(drag.y0, drag.y1), Math.max(drag.x0, drag.x1), Math.max(drag.y0, drag.y1)])}
            />
          )}
        </div>
      )}

      {editing && <TextEditor docId={docId} editing={editing} scale={scale} pageSize={size} />}
    </div>
  );
}

export default memo(PageView);
