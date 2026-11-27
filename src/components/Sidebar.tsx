// Left sidebar: page thumbnails and the PDF's bookmarks (outline).
import { memo, useEffect, useRef, useState } from "react";
import { renderPage, type OutlineItem } from "../api";
import { num, t } from "../i18n";
import { useApp, type Tab } from "../store";

const THUMB_WIDTH = 132;

function Thumb({ docId, index, size, active, onClick }: {
  docId: number;
  index: number;
  size: [number, number];
  active: boolean;
  onClick: () => void;
}) {
  const lang = useApp((s) => s.lang);
  const ref = useRef<HTMLButtonElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [visible, setVisible] = useState(false);
  const scale = THUMB_WIDTH / size[0];
  const height = size[1] * scale;

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const io = new IntersectionObserver(([e]) => e.isIntersecting && setVisible(true), { rootMargin: "300px 0px" });
    io.observe(el);
    return () => io.disconnect();
  }, []);

  useEffect(() => {
    if (!visible) return;
    let cancelled = false;
    renderPage(docId, index, scale * (window.devicePixelRatio || 1))
      .then((bmp) => {
        const c = canvasRef.current;
        if (cancelled || !c) return bmp.close();
        c.width = bmp.width;
        c.height = bmp.height;
        c.getContext("2d")?.drawImage(bmp, 0, 0);
        bmp.close();
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [visible, docId, index, scale]);

  useEffect(() => {
    if (active) ref.current?.scrollIntoView({ block: "nearest" });
  }, [active]);

  return (
    <button ref={ref} className={active ? "thumb active" : "thumb"} onClick={onClick} aria-current={active}>
      <canvas ref={canvasRef} style={{ width: THUMB_WIDTH, height }} />
      <span>{num(lang, index + 1)}</span>
    </button>
  );
}

function OutlineTree({ items, onGoTo }: { items: OutlineItem[]; onGoTo: (p: number) => void }) {
  return (
    <ul className="outline">
      {items.map((item, i) => (
        <OutlineNode key={i} item={item} onGoTo={onGoTo} />
      ))}
    </ul>
  );
}

function OutlineNode({ item, onGoTo }: { item: OutlineItem; onGoTo: (p: number) => void }) {
  const [open, setOpen] = useState(false);
  return (
    <li>
      <div className="outline-row">
        {item.children.length > 0 ? (
          <button className="twisty" onClick={() => setOpen(!open)} aria-expanded={open}>
            {open ? "▾" : "▸"}
          </button>
        ) : (
          <span className="twisty" />
        )}
        <button className="outline-title" disabled={item.page === null} onClick={() => item.page !== null && onGoTo(item.page)}>
          {item.title}
        </button>
      </div>
      {open && item.children.length > 0 && <OutlineTree items={item.children} onGoTo={onGoTo} />}
    </li>
  );
}

function Sidebar({ tab, onGoTo }: { tab: Tab; onGoTo: (p: number) => void }) {
  const lang = useApp((s) => s.lang);
  const update = useApp((s) => s.update);
  const { doc } = tab;
  return (
    <aside className="sidebar">
      <div className="sidebar-tabs" role="tablist">
        <button role="tab" aria-selected={tab.sidebar === "pages"} onClick={() => update(doc.id, { sidebar: "pages" })}>
          {t(lang, "thumbnails")}
        </button>
        <button role="tab" aria-selected={tab.sidebar === "bookmarks"} onClick={() => update(doc.id, { sidebar: "bookmarks" })}>
          {t(lang, "bookmarks")}
        </button>
      </div>
      <div className="sidebar-body">
        {tab.sidebar === "pages" &&
          doc.pages.map((size, i) => (
            <Thumb key={i} docId={doc.id} index={i} size={size} active={i === tab.page} onClick={() => onGoTo(i)} />
          ))}
        {tab.sidebar === "bookmarks" &&
          (doc.outline.length ? (
            <OutlineTree items={doc.outline} onGoTo={onGoTo} />
          ) : (
            <p className="muted pad">{t(lang, "noBookmarks")}</p>
          ))}
      </div>
    </aside>
  );
}

export default memo(Sidebar);
