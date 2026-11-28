import { useEffect, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { closePdf } from "./api";
import ChoiceDialog from "./components/ChoiceDialog";
import DocView from "./components/DocView";
import Home from "./components/Home";
import ImageTools from "./components/ImageTools";
import MergeDialog from "./components/MergeDialog";
import PasswordDialog from "./components/PasswordDialog";
import SelfTest from "./components/SelfTest";
import { t } from "./i18n";
import { openPath, pickAndOpen } from "./open";
import { confirmClose } from "./pageTools";
import { useActiveTab, useApp } from "./store";
import "./App.css";

export default function App() {
  const { lang, setLang, tabs, activeId, screen, activate, closeTab, setScreen, toast, merge } = useApp();
  const active = useActiveTab();
  const [dragging, setDragging] = useState(false);

  useEffect(() => {
    document.documentElement.lang = lang;
  }, [lang]);

  // Drag and drop PDF files onto the window (FR-1.1).
  useEffect(() => {
    const unlisten = getCurrentWebview().onDragDropEvent(async (event) => {
      const p = event.payload;
      if (p.type === "enter" || p.type === "over") setDragging(true);
      else if (p.type === "leave") setDragging(false);
      else if (p.type === "drop") {
        setDragging(false);
        for (const path of p.paths) if (path.toLowerCase().endsWith(".pdf")) await openPath(path);
      }
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  const close = async (id: number) => {
    if (!(await confirmClose([id]))) return;
    closePdf(id).catch(() => {});
    closeTab(id);
  };

  // Closing the window: offer to save unsaved changes first (SRS 4.5).
  useEffect(() => {
    const unlisten = getCurrentWindow().onCloseRequested(async (event) => {
      const ids = useApp.getState().tabs.map((tab) => tab.doc.id);
      if (!(await confirmClose(ids))) event.preventDefault();
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  // App-wide shortcuts (SRS 4.4).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!e.ctrlKey || document.querySelector(".overlay")) return;
      const k = e.key.toLowerCase();
      if (k === "o") {
        e.preventDefault();
        pickAndOpen();
      } else if (k === "w" && activeId !== null) {
        e.preventDefault();
        close(activeId);
      } else if (k === "tab" && tabs.length > 1) {
        e.preventDefault();
        const i = tabs.findIndex((t) => t.doc.id === activeId);
        const next = tabs[(i + (e.shiftKey ? -1 : 1) + tabs.length) % tabs.length];
        activate(next.doc.id);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  return (
    <div className="app">
      <header className="topbar">
        <button className="brand" onClick={() => setScreen("home")} title={t(lang, "appName")}>
          <span className="logo" aria-hidden="true">
            অ
          </span>
          <span className="brand-name">{t(lang, "appName")}</span>
        </button>

        <nav className="tabs" role="tablist">
          {tabs.map((tab) => (
            <div
              key={tab.doc.id}
              role="tab"
              aria-selected={screen === "doc" && tab.doc.id === activeId}
              className={screen === "doc" && tab.doc.id === activeId ? "tab active" : "tab"}
              title={tab.doc.path}
            >
              <button className="tab-name" onClick={() => activate(tab.doc.id)}>
                {tab.doc.dirty && (
                  <span className="dirty-dot" title={t(lang, "unsavedTitle")} aria-label={t(lang, "unsavedTitle")}>
                    ●{" "}
                  </span>
                )}
                {tab.doc.name}
              </button>
              <button className="tab-close" aria-label={`${t(lang, "close")} ${tab.doc.name}`} onClick={() => close(tab.doc.id)}>
                ×
              </button>
            </div>
          ))}
          <button className="tab-add" onClick={() => pickAndOpen()} title={`${t(lang, "openPdf")} (Ctrl+O)`}>
            +
          </button>
        </nav>

        <div className="top-actions">
          <span className="chip">● {t(lang, "offline")}</span>
          <button className="ghost" onClick={() => setLang(lang === "bn" ? "en" : "bn")}>
            {t(lang, "langSwitch")}
          </button>
        </div>
      </header>

      <main className="main">
        {screen === "selftest" ? (
          <SelfTest />
        ) : screen === "images" ? (
          <ImageTools />
        ) : screen === "doc" && active ? (
          <DocView key={active.doc.id} tab={active} />
        ) : (
          <Home dragging={dragging} />
        )}
        {dragging && screen === "doc" && <div className="drop-overlay">{t(lang, "dropNow")}</div>}
      </main>

      {merge && <MergeDialog initial={merge} />}
      <ChoiceDialog />
      <PasswordDialog />
      {toast && (
        <div className="toast" role="status">
          {toast}
        </div>
      )}
    </div>
  );
}
