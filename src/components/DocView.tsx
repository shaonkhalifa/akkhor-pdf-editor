// The viewer for one open PDF: toolbar, sidebar and the scrolling pages.
//
// Only pages near the visible area exist in the page (SRS 4.1: render visible
// pages plus one ahead and behind), so a 2,000-page file scrolls as smoothly
// as a 2-page one.
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { save as saveDialog, message } from "@tauri-apps/plugin-dialog";
import { PX_PER_PT, saveCopy, searchPdf } from "../api";
import { num, t } from "../i18n";
import { printDocument } from "../print";
import { useApp, type Tab } from "../store";
import PageView from "./PageView";
import Sidebar from "./Sidebar";

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
  const [toast, setToast] = useState<string | null>(null);

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

  const saveAs = async () => {
    const dest = await saveDialog({
      defaultPath: doc.name,
      filters: [{ name: t(lang, "pdfFiles"), extensions: ["pdf"] }],
    });
    if (!dest) return;
    try {
      await saveCopy(doc.id, dest);
      setToast(t(lang, "saved"));
      window.setTimeout(() => setToast(null), 2000);
    } catch (e) {
      await message(String(e), { kind: "error" });
    }
  };

  // Keyboard shortcuts for this document.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const typing = (e.target as HTMLElement)?.tagName === "INPUT";
      if (e.ctrlKey && e.key.toLowerCase() === "f") {
        e.preventDefault();
        searchRef.current?.focus();
        searchRef.current?.select();
      } else if (e.ctrlKey && e.key.toLowerCase() === "p") {
        e.preventDefault();
        print();
      } else if (e.ctrlKey && e.key.toLowerCase() === "s") {
        e.preventDefault();
        saveAs();
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
        <button className="tool text" onClick={print} disabled={printing !== null} title="Ctrl+P">
          {t(lang, "print")}
        </button>
        <button className="tool text" onClick={saveAs} title="Ctrl+S">
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
                index={i}
                size={doc.pages[i]}
                scale={scale}
                top={tops[i]}
                hits={hitsByPage.get(i) ?? []}
                currentHit={currentHit && currentHit.page === i ? currentHit : null}
                onGoTo={goTo}
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
      {toast && <div className="toast">{toast}</div>}
    </div>
  );
}
