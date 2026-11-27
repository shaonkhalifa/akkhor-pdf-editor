// Start screen: open a PDF, drop a PDF, or reopen a recent file.
import { useEffect, useState } from "react";
import { clearRecentFiles, recentFiles } from "../api";
import { t } from "../i18n";
import { openPath, pickAndOpen } from "../open";
import { useApp } from "../store";

function splitPath(p: string) {
  const parts = p.split(/[\\/]/);
  const name = parts.pop() ?? p;
  return { name, folder: parts.join("\\") };
}

export default function Home({ dragging }: { dragging: boolean }) {
  const lang = useApp((s) => s.lang);
  const setScreen = useApp((s) => s.setScreen);
  const [recent, setRecent] = useState<string[]>([]);

  useEffect(() => {
    recentFiles().then(setRecent).catch(() => setRecent([]));
  }, []);

  return (
    <div className="scroll-page">
      <div className="home">
        <div className={dragging ? "dropzone active" : "dropzone"}>
          <div className="drop-icon" aria-hidden="true">
            PDF
          </div>
          <button className="primary big" onClick={() => pickAndOpen()}>
            {t(lang, "openPdf")}
          </button>
          <p className="muted">{dragging ? t(lang, "dropNow") : t(lang, "dropHint")}</p>
          <p className="muted small">{t(lang, "shortcuts")}</p>
        </div>

        <section className="panel recent">
          <div className="card-head">
            <h2>{t(lang, "recent")}</h2>
            {recent.length > 0 && (
              <button
                className="link"
                onClick={async () => {
                  await clearRecentFiles();
                  setRecent([]);
                }}
              >
                {t(lang, "clearRecent")}
              </button>
            )}
          </div>
          {recent.length === 0 ? (
            <p className="muted">{t(lang, "noRecent")}</p>
          ) : (
            <ul className="recent-list">
              {recent.map((p) => {
                const { name, folder } = splitPath(p);
                return (
                  <li key={p}>
                    <button className="recent-item" onClick={() => openPath(p)} title={p}>
                      <span className="file-badge">PDF</span>
                      <span className="recent-text">
                        <span className="recent-name">{name}</span>
                        <span className="recent-folder">{folder}</span>
                      </span>
                    </button>
                  </li>
                );
              })}
            </ul>
          )}
        </section>

        <button className="link" onClick={() => setScreen("selftest")}>
          {t(lang, "selfTest")} →
        </button>
      </div>
    </div>
  );
}
