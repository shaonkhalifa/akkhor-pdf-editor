// Left sidebar: page thumbnails and the PDF's bookmarks (outline).
//
// Thumbnails are also where pages are managed (SRS Module 3): click to
// select (Ctrl/Shift for more), drag to reorder, right-click for a menu.
// Dragging uses pointer events, not HTML drag and drop, because the window
// already uses the system drag and drop for opening files.
import { memo, useEffect, useRef, useState } from "react";
import { renderPage, type OutlineItem } from "../api";
import { num, t } from "../i18n";
import {
  deletePages,
  extractToFile,
  insertBlank,
  insertFromFile,
  movePages,
  rotatePages,
  targetPages,
} from "../pageTools";
import { useApp, type Tab } from "../store";
import Icon, { type IconName } from "./Icon";

const THUMB_WIDTH = 132;
const DRAG_START_PX = 6;

function Thumb({ docId, version, index, size, active, selected, dropBefore, onPointerDown, onClick, onContextMenu }: {
  docId: number;
  version: number;
  index: number;
  size: [number, number];
  active: boolean;
  selected: boolean;
  dropBefore: boolean;
  onPointerDown: (e: React.PointerEvent) => void;
  onClick: (e: React.MouseEvent) => void;
  onContextMenu: (e: React.MouseEvent) => void;
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
    renderPage(docId, index, scale * (window.devicePixelRatio || 1), version)
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
  }, [visible, docId, index, scale, version]);

  useEffect(() => {
    if (active) ref.current?.scrollIntoView({ block: "nearest" });
  }, [active]);

  const cls = ["thumb", active && "active", selected && "selected", dropBefore && "drop-before"].filter(Boolean).join(" ");
  return (
    <button
      ref={ref}
      className={cls}
      data-index={index}
      onPointerDown={onPointerDown}
      onClick={onClick}
      onContextMenu={onContextMenu}
      aria-current={active}
      aria-pressed={selected}
      aria-label={`${t(lang, "page")} ${num(lang, index + 1)}`}
    >
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

interface Drag {
  index: number;
  startY: number;
  active: boolean;
  /** Drop before this page (page_count = at the end). */
  before: number | null;
}

function Sidebar({ tab, onGoTo }: { tab: Tab; onGoTo: (p: number) => void }) {
  const lang = useApp((s) => s.lang);
  const update = useApp((s) => s.update);
  const { doc } = tab;
  const bodyRef = useRef<HTMLDivElement>(null);
  const [drag, setDrag] = useState<Drag | null>(null);
  const dragRef = useRef<Drag | null>(null);
  const justDragged = useRef(false);
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
  const selected = new Set(tab.selected);

  const select = (pages: number[], anchor = tab.selectAnchor) =>
    update(doc.id, { selected: [...new Set(pages)].sort((a, b) => a - b), selectAnchor: anchor });

  const onThumbClick = (i: number, e: React.MouseEvent) => {
    if (justDragged.current) {
      justDragged.current = false;
      return;
    }
    if (e.shiftKey) {
      const [a, b] = [Math.min(tab.selectAnchor, i), Math.max(tab.selectAnchor, i)];
      select(Array.from({ length: b - a + 1 }, (_, k) => a + k));
    } else if (e.ctrlKey || e.metaKey) {
      select(selected.has(i) ? tab.selected.filter((p) => p !== i) : [...tab.selected, i], i);
    } else {
      select([i], i);
      onGoTo(i);
    }
  };

  // Where a drop at screen height `y` would put the pages.
  const dropIndex = (y: number): number => {
    const thumbs = bodyRef.current?.querySelectorAll<HTMLElement>(".thumb") ?? [];
    for (const el of Array.from(thumbs)) {
      const r = el.getBoundingClientRect();
      if (y < r.top + r.height / 2) return Number(el.dataset.index);
    }
    return doc.page_count;
  };

  const onPointerDown = (i: number, e: React.PointerEvent) => {
    if (e.button !== 0 || e.shiftKey || e.ctrlKey) return;
    dragRef.current = { index: i, startY: e.clientY, active: false, before: null };
  };

  useEffect(() => {
    const move = (e: PointerEvent) => {
      const d = dragRef.current;
      if (!d) return;
      if (!d.active && Math.abs(e.clientY - d.startY) < DRAG_START_PX) return;
      d.active = true;
      d.before = dropIndex(e.clientY);
      // Scroll while dragging near the top or bottom edge.
      const body = bodyRef.current;
      if (body) {
        const r = body.getBoundingClientRect();
        if (e.clientY < r.top + 40) body.scrollTop -= 14;
        else if (e.clientY > r.bottom - 40) body.scrollTop += 14;
      }
      setDrag({ ...d });
    };
    const up = () => {
      const d = dragRef.current;
      dragRef.current = null;
      setDrag(null);
      if (!d?.active || d.before === null) return;
      justDragged.current = true;
      window.setTimeout(() => (justDragged.current = false), 0);
      const pages = selected.has(d.index) ? tab.selected : [d.index];
      movePages(doc.id, pages, d.before);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    return () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
    };
  });

  const openMenu = (i: number, e: React.MouseEvent) => {
    e.preventDefault();
    if (!selected.has(i)) select([i], i);
    setMenu({ x: e.clientX, y: e.clientY });
  };

  useEffect(() => {
    if (!menu) return;
    const close = () => setMenu(null);
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && close();
    window.addEventListener("pointerdown", close);
    window.addEventListener("keydown", onKey);
    window.addEventListener("blur", close);
    return () => {
      window.removeEventListener("pointerdown", close);
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("blur", close);
    };
  }, [menu]);

  const actions: { icon: IconName; label: string; run: () => unknown; danger?: boolean }[] = [
    { icon: "rotateLeft", label: t(lang, "rotateLeft"), run: () => rotatePages(doc.id, -90) },
    { icon: "rotateRight", label: t(lang, "rotateRight"), run: () => rotatePages(doc.id, 90) },
    { icon: "blank", label: t(lang, "insertBlank"), run: () => insertBlank(doc.id) },
    { icon: "insert", label: t(lang, "insertFile"), run: () => insertFromFile(doc.id) },
    { icon: "extract", label: t(lang, "extract"), run: () => extractToFile(doc.id) },
    { icon: "trash", label: `${t(lang, "deletePages")} (Delete)`, run: () => deletePages(doc.id), danger: true },
  ];
  const count = targetPages(tab).length;

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
      {tab.sidebar === "pages" && (
        <>
          <div className="page-tools" role="toolbar" aria-label={t(lang, "thumbnails")}>
            {actions.map((a) => (
              <button key={a.icon} className={a.danger ? "tool icon danger" : "tool icon"} title={a.label} aria-label={a.label} onClick={() => a.run()}>
                <Icon name={a.icon} />
              </button>
            ))}
          </div>
          <p className="selection-note" title={t(lang, "dragHint")}>
            {tab.selected.length > 1 ? t(lang, "selectedPages", { n: num(lang, tab.selected.length) }) : t(lang, "dragHint")}
          </p>
        </>
      )}
      <div
        className={drag?.active ? "sidebar-body dragging" : "sidebar-body"}
        ref={bodyRef}
        tabIndex={-1}
        data-sidebar="pages"
        onKeyDown={(e) => {
          if (tab.sidebar === "pages" && e.ctrlKey && e.key.toLowerCase() === "a") {
            e.preventDefault();
            select(Array.from({ length: doc.page_count }, (_, i) => i));
          }
        }}
      >
        {tab.sidebar === "pages" && (
          <>
            {doc.pages.map((size, i) => (
              <Thumb
                key={i}
                docId={doc.id}
                version={doc.version}
                index={i}
                size={size}
                active={i === tab.page}
                selected={selected.has(i)}
                dropBefore={!!drag?.active && drag.before === i}
                onPointerDown={(e) => onPointerDown(i, e)}
                onClick={(e) => onThumbClick(i, e)}
                onContextMenu={(e) => openMenu(i, e)}
              />
            ))}
            {drag?.active && drag.before === doc.page_count && <div className="drop-end" />}
          </>
        )}
        {tab.sidebar === "bookmarks" &&
          (doc.outline.length ? (
            <OutlineTree items={doc.outline} onGoTo={onGoTo} />
          ) : (
            <p className="muted pad">{t(lang, "noBookmarks")}</p>
          ))}
      </div>

      {menu && (
        <div
          className="context-menu"
          role="menu"
          style={{ left: Math.min(menu.x, window.innerWidth - 280), top: Math.min(menu.y, window.innerHeight - 260) }}
          onPointerDown={(e) => e.stopPropagation()}
        >
          <div className="menu-head">{count > 1 ? t(lang, "selectedPages", { n: num(lang, count) }) : `${t(lang, "page")} ${num(lang, targetPages(tab)[0] + 1)}`}</div>
          {actions.map((a) => (
            <button
              key={a.icon}
              role="menuitem"
              className={a.danger ? "danger" : undefined}
              onClick={() => {
                setMenu(null);
                a.run();
              }}
            >
              <Icon name={a.icon} size={16} />
              {a.label}
            </button>
          ))}
        </div>
      )}
    </aside>
  );
}

export default memo(Sidebar);
