// One PDF page: the drawn image, an invisible text layer for selecting and
// copying, search highlights and clickable links.
import { memo, useEffect, useLayoutEffect, useRef, useState } from "react";
import { pageLinks, pageText, renderPage, type PageLink, type SearchHit, type TextLine } from "../api";
import { t } from "../i18n";
import { useApp } from "../store";

interface Props {
  docId: number;
  index: number;
  /** Page size in points. */
  size: [number, number];
  /** CSS pixels per point. */
  scale: number;
  top: number;
  hits: SearchHit[];
  currentHit: SearchHit | null;
  onGoTo: (page: number) => void;
}

function PageView({ docId, index, size, scale, top, hits, currentHit, onGoTo }: Props) {
  const lang = useApp((s) => s.lang);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const textRef = useRef<HTMLDivElement>(null);
  const [drawn, setDrawn] = useState(false);
  const [lines, setLines] = useState<TextLine[] | null>(null);
  const [links, setLinks] = useState<PageLink[]>([]);
  const width = size[0] * scale;
  const height = size[1] * scale;

  // Draw the page. On zoom the old image stays (stretched) until the new one is ready.
  useEffect(() => {
    let cancelled = false;
    const dpr = window.devicePixelRatio || 1;
    const deviceScale = Math.min(scale * dpr, 6);
    const timer = window.setTimeout(() => {
      renderPage(docId, index, deviceScale)
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
  }, [docId, index, scale]);

  // Text and links do not depend on zoom; load once.
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
  }, [docId, index]);

  // Stretch each invisible text span so it covers its line exactly.
  useLayoutEffect(() => {
    const root = textRef.current;
    if (!root) return;
    for (const el of Array.from(root.children) as HTMLElement[]) {
      el.style.transform = "";
      const target = Number(el.dataset.w);
      const natural = el.getBoundingClientRect().width;
      if (natural > 0 && target > 0) el.style.transform = `scaleX(${target / natural})`;
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
          const h = (l.rect[3] - l.rect[1]) * scale;
          return (
            <span
              key={i}
              data-w={(l.rect[2] - l.rect[0]) * scale}
              style={{ left: l.rect[0] * scale, top: l.rect[1] * scale, height: h, fontSize: h * 0.88 }}
            >
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
    </div>
  );
}

export default memo(PageView);
