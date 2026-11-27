// Start screen: open a PDF, drop a PDF, or reopen a recent file.
import { useEffect, useState } from "react";
import { open as openDialog, message } from "@tauri-apps/plugin-dialog";
import { clearRecentFiles, officeStatus, recentFiles, wordToPdf } from "../api";
import { NewPdfDialog, PicturesDialog } from "./ToolDialogs";
import { t } from "../i18n";
import { openPath, pickAndOpen } from "../open";
import { useApp } from "../store";
import Icon from "./Icon";

function splitPath(p: string) {
  const parts = p.split(/[\\/]/);
  const name = parts.pop() ?? p;
  return { name, folder: parts.join("\\") };
}

export default function Home({ dragging }: { dragging: boolean }) {
  const lang = useApp((s) => s.lang);
  const setScreen = useApp((s) => s.setScreen);
  const setMerge = useApp((s) => s.setMerge);
  const [recent, setRecent] = useState<string[]>([]);
  const addTab = useApp((s) => s.addTab);
  const [dialog, setDialog] = useState<"new" | "pictures" | null>(null);
  const [office, setOffice] = useState<{ libreoffice: boolean; word: boolean } | null>(null);
  const [converting, setConverting] = useState(false);

  useEffect(() => {
    officeStatus().then(setOffice).catch(() => setOffice({ libreoffice: false, word: false }));
  }, []);

  const convertWord = async () => {
    const path = await openDialog({ multiple: false, directory: false, filters: [{ name: t(lang, "wordFiles"), extensions: ["docx", "doc", "odt", "rtf"] }] });
    if (typeof path !== "string") return;
    setConverting(true);
    try {
      addTab(await wordToPdf(path));
    } catch (e) {
      await message(String(e).includes("NO_OFFICE") ? t(lang, "noOffice") : String(e), { kind: "error" });
    } finally {
      setConverting(false);
    }
  };
  const hasOffice = !!office && (office.libreoffice || office.word);

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
          <div className="row center">
            <button className="primary big" onClick={() => pickAndOpen()}>
              {t(lang, "openPdf")}
            </button>
            <button className="big" onClick={() => setMerge([])}>
              <Icon name="merge" size={18} /> {t(lang, "merge")}
            </button>
          </div>
          <div className="row center">
            <button onClick={() => setDialog("new")}>
              <Icon name="blank" size={16} /> {t(lang, "newPdf")}
            </button>
            <button onClick={() => setDialog("pictures")}>
              <Icon name="picture" size={16} /> {t(lang, "picturesToPdf")}
            </button>
            <button
              onClick={convertWord}
              disabled={!hasOffice || converting}
              title={hasOffice ? t(lang, office!.libreoffice ? "wordViaLibre" : "wordViaWord") : t(lang, "noOffice")}
            >
              <Icon name="insert" size={16} /> {converting ? t(lang, "working") : t(lang, "wordToPdf")}
            </button>
          </div>
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

        {dialog === "new" && <NewPdfDialog onClose={() => setDialog(null)} />}
        {dialog === "pictures" && <PicturesDialog onClose={() => setDialog(null)} />}
        <button className="link" onClick={() => setScreen("selftest")}>
          {t(lang, "selfTest")} →
        </button>
      </div>
    </div>
  );
}
