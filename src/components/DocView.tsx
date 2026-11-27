// The viewer for one open PDF: toolbar, sidebar and the scrolling pages.
//
// Only pages near the visible area exist in the page (SRS 4.1: render visible
// pages plus one ahead and behind), so a 2,000-page file scrolls as smoothly
// as a 2-page one.
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { PX_PER_PT, searchPdf } from "../api";
import { num, t } from "../i18n";
import { deletePages, redo, rotatePages, save, saveAs, undo } from "../pageTools";
import { printDocument } from "../print";
import { useApp, type Tab } from "../store";
import Icon from "./Icon";
import PageView from "./PageView";
import Sidebar from "./Sidebar";
import SignatureDialog from "./SignatureDialog";
import SplitDialog from "./SplitDialog";
import StampDialog from "./StampDialog";
import { insertPicture } from "../insertTools";
import { setTool } from "../textTools";
import type { Tool } from "../store";
import type { Key } from "../i18n";

const TOOL_BANNERS: Record<Exclude<Tool, "none">, Key> = {
  text: "editTextBanner",
  addtext: "addTextBanner",
  highlight: "markBanner",
  underline: "markBanner",
  strike: "markBanner",
  redact: "redactBanner",
  images: "editImagesBanner",
};

const TEXT_TOOLS: { tool: Tool; icon: "editText" | "addText" | "highlight" | "underline" | "strike" | "redact"; key: Key }[] = [
  { tool: "text", icon: "editText", key: "editText" },
  { tool: "addtext", icon: "addText", key: "addText" },
  { tool: "highlight", icon: "highlight", key: "highlight" },
  { tool: "underline", icon: "underline", key: "underline" },
  { tool: "strike", icon: "strike", key: "strike" },
  { tool: "redact", icon: "redact", key: "redact" },
];

const GAP = 16; // px between pages
const PAD = 24; // px around the page column
const ZOOM_STEPS = [0.25, 0.33, 0.5, 0.67, 0.75, 0.9, 1, 1.1, 1.25, 1.5, 1.75, 2, 2.5, 3, 4];
const OVERSCAN = 1; // pages drawn above and below the visible ones

export default function DocView({ tab }: { tab: Tab }) {
  const lang = useApp((s) => s.lang);
  const update = useApp((s) => s.update);
  const { doc } = tab;
  const scrollRef = useRef<HTMLDivElement>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const [viewport, setViewport] = useState({ w: 800, h: 600, top: 0 });
  const [pageInput, setPageInput] = useState("1");
  const [query, setQuery] = useState(tab.query);
  const [searching, setSearching] = useState(false);
  const [printing, setPrinting] = useState<number | null>(null);
  const [splitting, setSplitting] = useState(false);
  const [dialog, setDialog] = useState<"signature" | "stamp" | null>(null);
  const setMerge = useApp((s) => s.setMerge);

  const maxW = useMemo(() => Math.max(...doc.pages.map((p) => p[0])), [doc.pages]);

  // Resolve fit modes to a number whenever the window or page changes.
  const zoom = useMemo(() => {
    const fitW = (viewport.w - PAD * 2) / (maxW * PX_PER_PT);
    if (tab.zoomMode === "fit-width") return Math.max(0.25, Math.min(4, fitW));
    if (tab.zoomMode === "fit-page") {
      const [, h] = doc.pages[tab.page] ?? doc.pages[0];
      const fitH = (viewport.h - PAD * 2) / (h * PX_PER_PT);
      return Math.max(0.25, Math.min(4, fitW, fitH));
    }
    return tab.zoom;
  }, [tab.zoomMode, tab.zoom, tab.page, viewport.w, viewport.h, maxW, doc.pages]);
  const scale = zoom * PX_PER_PT;

  // Page positions in the scroll area.
  const { tops, total } = useMemo(() => {
    const tops: number[] = [];
    let y = PAD;
    for (const [, h] of doc.pages) {
      tops.push(y);
      y += h * scale + GAP;
    }
    return { tops, total: y - GAP + PAD };
  }, [doc.pages, scale]);

  // Which pages are on screen.
  const pageAt = useCallback(
    (y: number) => {
      let lo = 0;
      let hi = tops.length - 1;
      while (lo < hi) {
        const mid = (lo + hi + 1) >> 1;
        if (tops[mid] <= y) lo = mid;
        else hi = mid - 1;
      }
      return lo;
    },
    [tops],
  );
  const first = Math.max(0, pageAt(viewport.top) - OVERSCAN);
  const last = Math.min(doc.page_count - 1, pageAt(viewport.top + viewport.h) + OVERSCAN);

  // Track the scroll area's size.
  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setViewport((v) => ({ ...v, w: el.clientWidth, h: el.clientHeight })));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  // Keep the reader's place when zoom changes.
  const anchor = useRef<{ page: number; frac: number } | null>(null);
  const lastScale = useRef(scale);
  useLayoutEffect(() => {
    const el = scrollRef.current;
    if (!el || lastScale.current === scale) return;
    const a = anchor.current;
    if (a) el.scrollTop = tops[a.page] + a.frac * doc.pages[a.page][1] * scale - PAD;
    lastScale.current = scale;
  }, [scale, tops, doc.pages]);

  const onScroll = () => {
    const el = scrollRef.current;
    if (!el) return;
    const top = el.scrollTop;
    setViewport((v) => ({ ...v, top }));
    const p = pageAt(top + el.clientHeight / 3);
    anchor.current = { page: p, frac: (top + PAD - tops[p]) / (doc.pages[p][1] * scale) };
    if (p !== tab.page) update(doc.id, { page: p });
  };

  const goTo = useCallback(
    (page: number, y?: number) => {
      const el = scrollRef.current;
      if (!el || page < 0 || page >= doc.page_count) return;
      const offset = y !== undefined ? Math.max(0, y * scale - el.clientHeight / 3) : -GAP / 2;
      el.scrollTo({ top: tops[page] + offset - PAD / 2 });
      update(doc.id, { page });
    },
    [doc.id, doc.page_count, scale, tops, update],
  );

  useEffect(() => {
    if (tab.goTo !== null) {
      goTo(tab.goTo);
      update(doc.id, { goTo: null });
    }
  }, [tab.goTo, goTo, doc.id, update]);

  useEffect(() => setPageInput(num(lang, tab.page + 1)), [tab.page, lang]);

  const setZoom = (z: number) => {
    const el = scrollRef.current;
    if (el) {
      const p = pageAt(el.scrollTop + el.clientHeight / 3);
      anchor.current = { page: p, frac: (el.scrollTop + PAD - tops[p]) / (doc.pages[p][1] * scale) };
    }
    update(doc.id, { zoom: Math.max(0.25, Math.min(4, z)), zoomMode: "custom" });
  };
  const stepZoom = (dir: 1 | -1) => {
    const next = dir > 0 ? ZOOM_STEPS.find((s) => s > zoom + 0.001) : [...ZOOM_STEPS].reverse().find((s) => s < zoom - 0.001);
    setZoom(next ?? zoom);
  };

  // Ctrl + mouse wheel zooms.
  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      if (!e.ctrlKey) return;
      e.preventDefault();
      stepZoom(e.deltaY < 0 ? 1 : -1);
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
  });

  // --- Search ---------------------------------------------------------------
  const showHit = (index: number, hits = tab.hits) => {
    if (!hits.length) return;
    const i = (index + hits.length) % hits.length;
    update(doc.id, { hitIndex: i });
    goTo(hits[i].page, hits[i].rect[1]);
  };

  const runSearch = async (backwards = false) => {
    if (query.trim() === tab.query && tab.hits.length) {
      showHit(tab.hitIndex + (backwards ? -1 : 1));
      return;
    }
    setSearching(true);
    try {
      const hits = await searchPdf(doc.id, doc.page_count, query);
      // Start from the first hit on or after the current page.
      const start = Math.max(0, hits.findIndex((h) => h.page >= tab.page));
      update(doc.id, { query: query.trim(), hits, hitIndex: start });
      if (hits.length) showHit(start, hits);
    } finally {
      setSearching(false);
    }
  };

  const clearSearch = () => {
    setQuery("");
    update(doc.id, { query: "", hits: [], hitIndex: 0 });
  };

  // --- Print and save -------------------------------------------------------
  const print = async () => {
    setPrinting(0);
    try {
      await printDocument(doc, setPrinting);
    } finally {
      setPrinting(null);
    }
  };

  // Keyboard shortcuts for this document.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement;
      const typing = target?.tagName === "INPUT" || target?.tagName === "TEXTAREA";
      // Another window (a dialog) is open: leave its keys alone.
      if (document.querySelector(".overlay")) return;
      const key = e.key.toLowerCase();
      if (e.ctrlKey && !typing && key === "z" && !e.shiftKey) {
        e.preventDefault();
        undo(doc.id);
      } else if (e.ctrlKey && !typing && (key === "y" || (key === "z" && e.shiftKey))) {
        e.preventDefault();
        redo(doc.id);
      } else if (e.ctrlKey && key === "s") {
        e.preventDefault();
        if (e.shiftKey) saveAs(doc.id);
        else save(doc.id);
      } else if (!typing && e.key === "Delete" && target?.closest(".sidebar")) {
        e.preventDefault();
        deletePages(doc.id);
      } else if (e.ctrlKey && !typing && (key === "]" || key === "[") && target?.closest(".sidebar")) {
        e.preventDefault();
        rotatePages(doc.id, key === "]" ? 90 : -90);
      } else if (e.ctrlKey && e.key.toLowerCase() === "f") {
        e.preventDefault();
        searchRef.current?.focus();
        searchRef.current?.select();
      } else if (e.ctrlKey && e.key.toLowerCase() === "p") {
        e.preventDefault();
        print();
      } else if (e.ctrlKey && (e.key === "=" || e.key === "+")) {
        e.preventDefault();
        stepZoom(1);
      } else if (e.ctrlKey && e.key === "-") {
        e.preventDefault();
        stepZoom(-1);
      } else if (e.ctrlKey && e.key === "0") {
        e.preventDefault();
        update(doc.id, { zoomMode: "fit-width" });
      } else if (!typing && e.key === "Home") {
        goTo(0);
      } else if (!typing && e.key === "End") {
        goTo(doc.page_count - 1);
      } else if (e.key === "F3") {
        e.preventDefault();
        showHit(tab.hitIndex + (e.shiftKey ? -1 : 1));
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  const hitsByPage = useMemo(() => {
    const map = new Map<number, typeof tab.hits>();
    for (const h of tab.hits) map.set(h.page, [...(map.get(h.page) ?? []), h]);
    return map;
  }, [tab.hits]);
  const currentHit = tab.hits[tab.hitIndex] ?? null;

  const visible: number[] = [];
  for (let i = first; i <= last; i++) visible.push(i);

  return (
    <div className="docview">
      <div className="toolbar" role="toolbar">
        <button
          className={tab.sidebar ? "tool on" : "tool"}
          title={t(lang, "sidebar")}
          aria-pressed={!!tab.sidebar}
          onClick={() => update(doc.id, { sidebar: tab.sidebar ? null : "pages" })}
        >
          ☰
        </button>
        <span className="sep" />
        <button className="tool icon" title={`${t(lang, "undo")} (Ctrl+Z)`} aria-label={t(lang, "undo")} disabled={!doc.can_undo} onClick={() => undo(doc.id)}>
          <Icon name="undo" />
        </button>
        <button className="tool icon" title={`${t(lang, "redo")} (Ctrl+Y)`} aria-label={t(lang, "redo")} disabled={!doc.can_redo} onClick={() => redo(doc.id)}>
          <Icon name="redo" />
        </button>
        <button
          className={doc.dirty ? "tool text save dirty" : "tool text save"}
          title={`${t(lang, "save")} (Ctrl+S)`}
          onClick={() => save(doc.id)}
        >
          <Icon name="save" size={16} /> {t(lang, "save")}
        </button>
        <span className="sep" />
        <form
          className="page-nav"
          onSubmit={(e) => {
            e.preventDefault();
            const n = Number(pageInput.replace(/[০-৯]/g, (d) => String("০১২৩৪৫৬৭৮৯".indexOf(d))));
            if (n >= 1 && n <= doc.page_count) goTo(n - 1);
            else setPageInput(num(lang, tab.page + 1));
          }}
        >
          <label>
            {t(lang, "page")}{" "}
            <input value={pageInput} onChange={(e) => setPageInput(e.target.value)} onFocus={(e) => e.target.select()} />
          </label>
          <span className="muted">/ {num(lang, doc.page_count)}</span>
        </form>
        <span className="sep" />
        <button className="tool" title={`${t(lang, "zoomOut")} (Ctrl −)`} onClick={() => stepZoom(-1)}>
          −
        </button>
        <span className="zoom-label">{num(lang, Math.round(zoom * 100))}%</span>
        <button className="tool" title={`${t(lang, "zoomIn")} (Ctrl +)`} onClick={() => stepZoom(1)}>
          +
        </button>
        <button className={tab.zoomMode === "fit-width" ? "tool text on" : "tool text"} onClick={() => update(doc.id, { zoomMode: "fit-width" })}>
          {t(lang, "fitWidth")}
        </button>
        <button className={tab.zoomMode === "fit-page" ? "tool text on" : "tool text"} onClick={() => update(doc.id, { zoomMode: "fit-page" })}>
          {t(lang, "fitPage")}
        </button>

        <span className="spacer" />

        <form
          className="search"
          onSubmit={(e) => {
            e.preventDefault();
            runSearch();
          }}
        >
          <input
            ref={searchRef}
            type="search"
            placeholder={t(lang, "search")}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Escape") clearSearch();
              if (e.key === "Enter" && e.shiftKey) {
                e.preventDefault();
                runSearch(true);
              }
            }}
          />
          {searching ? (
            <span className="muted small">…</span>
          ) : tab.query ? (
            <span className="muted small">
              {tab.hits.length ? `${num(lang, tab.hitIndex + 1)}/${num(lang, tab.hits.length)}` : t(lang, "noMatches")}
            </span>
          ) : null}
          <button type="button" className="tool" title={t(lang, "prev")} disabled={!tab.hits.length} onClick={() => showHit(tab.hitIndex - 1)}>
            ↑
          </button>
          <button type="button" className="tool" title={t(lang, "next")} disabled={!tab.hits.length} onClick={() => showHit(tab.hitIndex + 1)}>
            ↓
          </button>
        </form>

        <span className="sep" />
        {TEXT_TOOLS.map((x) => (
          <button
            key={x.tool}
            className={tab.tool === x.tool ? (x.tool === "text" || x.tool === "addtext" ? "tool text on" : "tool icon on") : x.tool === "text" || x.tool === "addtext" ? "tool text" : "tool icon"}
            aria-pressed={tab.tool === x.tool}
            title={t(lang, x.key)}
            aria-label={t(lang, x.key)}
            onClick={() => setTool(doc.id, x.tool)}
          >
            <Icon name={x.icon} size={16} />
            {(x.tool === "text" || x.tool === "addtext") && ` ${t(lang, x.key)}`}
          </button>
        ))}
        <span className="sep" />
        <button className="tool text" onClick={() => insertPicture(doc.id)} title={t(lang, "insertPictureHint")}>
          <Icon name="picture" size={16} /> {t(lang, "picture")}
        </button>
        <button className="tool text" onClick={() => setDialog("signature")} title={t(lang, "signature")}>
          <Icon name="signature" size={16} /> {t(lang, "signature")}
        </button>
        <button className="tool text" onClick={() => setDialog("stamp")} title={t(lang, "stamp")}>
          <Icon name="stamp" size={16} /> {t(lang, "stamp")}
        </button>
        <button
          className={tab.tool === "images" ? "tool text on" : "tool text"}
          aria-pressed={tab.tool === "images"}
          onClick={() => setTool(doc.id, "images")}
          title={t(lang, "editImagesHint")}
        >
          <Icon name="images" size={16} /> {t(lang, "editImages")}
        </button>
        <span className="sep" />
        <button className="tool text" onClick={() => setSplitting(true)} title={t(lang, "splitTitle")}>
          <Icon name="split" size={16} /> {t(lang, "split")}
        </button>
        <button className="tool text" onClick={() => setMerge([doc.path])} title={t(lang, "mergeTitle")}>
          <Icon name="merge" size={16} /> {t(lang, "merge")}
        </button>
        <span className="sep" />
        <button className="tool text" onClick={print} disabled={printing !== null} title="Ctrl+P">
          {t(lang, "print")}
        </button>
        <button className="tool text" onClick={() => saveAs(doc.id)} title="Ctrl+Shift+S">
          {t(lang, "saveAs")}
        </button>
      </div>

      <div className="doc-body">
        {tab.sidebar && <Sidebar tab={tab} onGoTo={goTo} />}
        <div className="pages-scroll" ref={scrollRef} onScroll={onScroll} tabIndex={0}>
          <div className="pages" style={{ height: total, minWidth: maxW * scale + PAD * 2 }}>
            {visible.map((i) => (
              <PageView
                key={i}
                docId={doc.id}
                version={doc.version}
                index={i}
                size={doc.pages[i]}
                scale={scale}
                top={tops[i]}
                hits={hitsByPage.get(i) ?? []}
                currentHit={currentHit && currentHit.page === i ? currentHit : null}
                onGoTo={goTo}
                placing={tab.placing && tab.placing.page === i ? tab.placing : null}
                tool={tab.tool}
                editing={tab.editing && tab.editing.page === i ? tab.editing : null}
              />
            ))}
          </div>
        </div>
      </div>

      {printing !== null && (
        <div className="overlay">
          <div className="dialog">
            <p>{t(lang, "preparingPrint")}</p>
            <progress max={doc.page_count} value={printing} />
            <p className="muted small">
              {num(lang, printing)} / {num(lang, doc.page_count)}
            </p>
          </div>
        </div>
      )}
      {splitting && <SplitDialog doc={doc} onClose={() => setSplitting(false)} />}
      {dialog === "signature" && <SignatureDialog docId={doc.id} onClose={() => setDialog(null)} />}
      {dialog === "stamp" && <StampDialog docId={doc.id} onClose={() => setDialog(null)} />}
      {tab.tool !== "none" && !tab.editing && (
        <div className="mode-banner">
          {t(lang, TOOL_BANNERS[tab.tool])}
          <button className="banner-done" onClick={() => setTool(doc.id, tab.tool)}>
            {t(lang, "done")}
          </button>
        </div>
      )}
    </div>
  );
}
