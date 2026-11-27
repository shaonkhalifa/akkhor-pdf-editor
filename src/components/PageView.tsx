// One PDF page: the drawn image, an invisible text layer for selecting and
// copying, search highlights and clickable links.
import { memo, useEffect, useLayoutEffect, useRef, useState } from "react";
import { pageImages, pageLinks, pageText, renderPage, type PageImage, type PageLink, type SearchHit, type TextLine } from "../api";
import { t } from "../i18n";
import { deleteExistingImage, replaceExistingImage } from "../insertTools";
import { useApp, type Placing } from "../store";
import PlacementOverlay from "./PlacementOverlay";

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
  /** Outline the images on this page so they can be deleted or replaced. */
  imageMode: boolean;
}

function PageView({ docId, version, index, size, scale, top, hits, currentHit, onGoTo, placing, imageMode }: Props) {
  const lang = useApp((s) => s.lang);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const textRef = useRef<HTMLDivElement>(null);
  const [drawn, setDrawn] = useState(false);
  const [lines, setLines] = useState<TextLine[] | null>(null);
  const [links, setLinks] = useState<PageLink[]>([]);
  const [images, setImages] = useState<PageImage[]>([]);
  const [picked, setPicked] = useState<number | null>(null);

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
    <div className="pdf-page" style={{ top, width, height }} data-page={index}>
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
    </div>
  );
}

export default memo(PageView);
