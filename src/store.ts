// App state: open tabs, the active tab and interface language.
import { create } from "zustand";
import type { DocInfo, Placement, SearchHit, StyleChoice, TextBlock } from "./api";
import type { Lang } from "./i18n";

export type ZoomMode = "custom" | "fit-width" | "fit-page";

export interface Tab {
  doc: DocInfo;
  /** 1 = 100% (actual size). */
  zoom: number;
  zoomMode: ZoomMode;
  /** Zero-based page the reader is on. */
  page: number;
  /** Page to scroll to; cleared once done. */
  goTo: number | null;
  query: string;
  hits: SearchHit[];
  hitIndex: number;
  sidebar: "pages" | "bookmarks" | null;
  /** Pages selected in the thumbnails (zero-based, sorted). */
  selected: number[];
  /** Where a Shift+click range selection starts. */
  selectAnchor: number;
  /** A picture being positioned, if any. */
  placing: Placing | null;
  /** What clicking or dragging on a page does. */
  tool: Tool;
  /** Text being edited, if any. */
  editing: Editing | null;
}

/** A picture being positioned on a page before it is placed. */
export interface Placing extends Placement {
  dataUrl: string;
  page: number;
  /** Width / height of the picture, kept while resizing. */
  aspect: number;
}

/** What a click or drag on a page does. */
export type Tool = "none" | "text" | "addtext" | "highlight" | "underline" | "strike" | "redact" | "images";

/** Text being edited in place: an existing block, or a new text box. */
export interface Editing {
  page: number;
  /** The block being changed; null for a new text box. */
  block: TextBlock | null;
  /** Top-left of the edit box, in points. Dragging the box moves it. */
  left: number;
  top: number;
  /** Where the box started, to tell how far it was moved. */
  startLeft: number;
  startTop: number;
  width: number;
  text: string;
  style: StyleChoice;
}

export type Screen = "home" | "doc" | "selftest" | "images";

interface State {
  lang: Lang;
  tabs: Tab[];
  activeId: number | null;
  screen: Screen;
  /** A short message shown at the bottom of the window. */
  toast: string | null;
  /** Files listed in the merge dialog; null when it is closed. */
  merge: string[] | null;
  notify: (text: string) => void;
  setMerge: (files: string[] | null) => void;
  setLang: (l: Lang) => void;
  setScreen: (s: Screen) => void;
  addTab: (doc: DocInfo) => void;
  closeTab: (id: number) => void;
  activate: (id: number) => void;
  update: (id: number, patch: Partial<Tab>) => void;
  /** Replace a tab's document after a change, undo or save. */
  setDoc: (doc: DocInfo, selected?: number[]) => void;
}

function initialLang(): Lang {
  try {
    return localStorage.getItem("akkhor.lang") === "en" ? "en" : "bn";
  } catch {
    return "bn";
  }
}

let toastTimer = 0;

export const useApp = create<State>((set) => ({
  lang: initialLang(),
  tabs: [],
  activeId: null,
  screen: "home",
  toast: null,
  merge: null,
  notify: (text) => {
    set({ toast: text });
    window.clearTimeout(toastTimer);
    toastTimer = window.setTimeout(() => set({ toast: null }), 2500);
  },
  setMerge: (merge) => set({ merge }),
  setLang: (lang) => {
    try {
      localStorage.setItem("akkhor.lang", lang);
    } catch {
      /* per-viewer convenience only */
    }
    set({ lang });
  },
  setScreen: (screen) => set({ screen }),
  addTab: (doc) =>
    set((s) => ({
      tabs: [
        ...s.tabs,
        {
          doc,
          zoom: 1,
          zoomMode: "fit-width",
          page: 0,
          goTo: null,
          query: "",
          hits: [],
          hitIndex: 0,
          sidebar: doc.page_count > 1 ? "pages" : null,
          selected: [],
          selectAnchor: 0,
          placing: null,
          tool: "none",
          editing: null,
        },
      ],
      activeId: doc.id,
      screen: "doc",
    })),
  closeTab: (id) =>
    set((s) => {
      const i = s.tabs.findIndex((t) => t.doc.id === id);
      const tabs = s.tabs.filter((t) => t.doc.id !== id);
      const next = tabs[Math.min(i, tabs.length - 1)];
      const activeId = s.activeId === id ? (next?.doc.id ?? null) : s.activeId;
      return { tabs, activeId, screen: tabs.length ? s.screen : "home" };
    }),
  activate: (id) => set({ activeId: id, screen: "doc" }),
  update: (id, patch) =>
    set((s) => ({ tabs: s.tabs.map((t) => (t.doc.id === id ? { ...t, ...patch } : t)) })),
  setDoc: (doc, selected) =>
    set((s) => ({
      tabs: s.tabs.map((t) => {
        if (t.doc.id !== doc.id) return t;
        const last = Math.max(0, doc.page_count - 1);
        const changed = doc.version !== t.doc.version;
        return {
          ...t,
          doc,
          page: Math.min(t.page, last),
          selected: (selected ?? t.selected).filter((p) => p <= last),
          // Search results point at old page positions after a change.
          ...(changed ? { hits: [], hitIndex: 0, query: "" } : {}),
        };
      }),
    })),
}));

export const useActiveTab = () => useApp((s) => s.tabs.find((t) => t.doc.id === s.activeId) ?? null);
