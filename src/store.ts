// App state: open tabs, the active tab and interface language.
import { create } from "zustand";
import type { DocInfo, SearchHit } from "./api";
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
}

export type Screen = "home" | "doc" | "selftest";

interface State {
  lang: Lang;
  tabs: Tab[];
  activeId: number | null;
  screen: Screen;
  setLang: (l: Lang) => void;
  setScreen: (s: Screen) => void;
  addTab: (doc: DocInfo) => void;
  closeTab: (id: number) => void;
  activate: (id: number) => void;
  update: (id: number, patch: Partial<Tab>) => void;
}

function initialLang(): Lang {
  try {
    return localStorage.getItem("akkhor.lang") === "en" ? "en" : "bn";
  } catch {
    return "bn";
  }
}

export const useApp = create<State>((set) => ({
  lang: initialLang(),
  tabs: [],
  activeId: null,
  screen: "home",
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
}));

export const useActiveTab = () => useApp((s) => s.tabs.find((t) => t.doc.id === s.activeId) ?? null);
