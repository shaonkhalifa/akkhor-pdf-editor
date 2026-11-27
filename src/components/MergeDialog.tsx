// Merge several PDFs into one new file (SRS FR-3.1). The files are joined in
// the order shown; the originals are never changed. The result opens in a
// new tab.
import { useEffect, useRef, useState } from "react";
import { message, open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { mergePdfs, probePdf } from "../api";
import { num, t } from "../i18n";
import { askPassword, baseName, openPath } from "../open";
import { useApp } from "../store";

interface Item {
  path: string;
  pages: number | null;
  password: string | null;
  error: string | null;
}

async function inspect(path: string): Promise<Item | null> {
  let password: string | null = null;
  for (;;) {
    try {
      return { path, pages: await probePdf(path, password), password, error: null };
    } catch (e) {
      const err = String(e);
      if (err.includes("NEEDS_PASSWORD") || err.includes("WRONG_PASSWORD")) {
        password = await askPassword(baseName(path), err.includes("WRONG_PASSWORD"));
        if (password === null) return null;
        continue;
      }
      return { path, pages: null, password, error: err };
    }
  }
}

export default function MergeDialog({ initial }: { initial: string[] }) {
  const lang = useApp((s) => s.lang);
  const tabs = useApp((s) => s.tabs);
  const setMerge = useApp((s) => s.setMerge);
  const notify = useApp((s) => s.notify);
  const [items, setItems] = useState<Item[]>([]);
  const [busy, setBusy] = useState(false);
  const close = () => setMerge(null);

  const add = async (paths: string[]) => {
    for (const p of paths) {
      const item = await inspect(p);
      if (item) setItems((list) => [...list, item]);
    }
  };

  // Add the files the dialog was opened with (once, even in React's strict mode).
  const started = useRef(false);
  useEffect(() => {
    if (started.current) return;
    started.current = true;
    add(initial);
  });

  const pick = async () => {
    const chosen = await openDialog({ multiple: true, directory: false, filters: [{ name: t(lang, "pdfFiles"), extensions: ["pdf"] }] });
    add(Array.isArray(chosen) ? chosen : chosen ? [chosen] : []);
  };

  const move = (i: number, by: number) =>
    setItems((list) => {
      const next = [...list];
      const [item] = next.splice(i, 1);
      next.splice(Math.max(0, Math.min(next.length, i + by)), 0, item);
      return next;
    });

  const good = items.filter((i) => i.error === null);
  const unsaved = items
    .map((i) => tabs.find((tab) => tab.doc.path === i.path && tab.doc.dirty))
    .filter((tab) => !!tab);

  const run = async () => {
    const dest = await saveDialog({
      defaultPath: `${t(lang, "merged")}.pdf`,
      filters: [{ name: t(lang, "pdfFiles"), extensions: ["pdf"] }],
    });
    if (!dest) return;
    setBusy(true);
    try {
      await mergePdfs(
        good.map((i) => ({ path: i.path, password: i.password })),
        dest,
      );
      close();
      notify(t(lang, "saved"));
      await openPath(dest);
    } catch (e) {
      const err = String(e);
      await message(err.includes("SAVE_OVER_INPUT") ? t(lang, "saveOverInput") : err, { kind: "error", title: t(lang, "mergeTitle") });
    } finally {
      setBusy(false);
    }
  };

  const totalPages = good.reduce((sum, i) => sum + (i.pages ?? 0), 0);

  return (
    <div className="overlay" role="dialog" aria-modal="true" aria-labelledby="merge-title" onKeyDown={(e) => e.key === "Escape" && close()}>
      <div className="dialog wide">
        <h2 id="merge-title">{t(lang, "mergeTitle")}</h2>
        <p className="muted">{t(lang, "mergeIntro")}</p>

        <ol className="merge-list">
          {items.map((item, i) => (
            <li key={`${item.path}-${i}`} className={item.error ? "bad" : undefined}>
              <span className="file-badge">PDF</span>
              <span className="merge-name" title={item.path}>
                {baseName(item.path)}
                <span className="muted small">
                  {item.error ? item.error : `${num(lang, item.pages ?? 0)} ${t(lang, "pagesSuffix")}`}
                </span>
              </span>
              <button className="tool" title={t(lang, "moveUp")} aria-label={t(lang, "moveUp")} disabled={i === 0} onClick={() => move(i, -1)}>
                ↑
              </button>
              <button
                className="tool"
                title={t(lang, "moveDown")}
                aria-label={t(lang, "moveDown")}
                disabled={i === items.length - 1}
                onClick={() => move(i, 1)}
              >
                ↓
              </button>
              <button
                className="tool danger"
                title={t(lang, "remove")}
                aria-label={t(lang, "remove")}
                onClick={() => setItems((list) => list.filter((_, k) => k !== i))}
              >
                ×
              </button>
            </li>
          ))}
        </ol>
        <div>
          <button className="ghost" onClick={pick}>
            + {t(lang, "addFiles")}
          </button>
        </div>

        {unsaved.map((tab) => (
          <p key={tab.doc.id} className="error small">
            {t(lang, "mergeUnsaved", { name: tab.doc.name })}
          </p>
        ))}
        <p className="muted small">
          {good.length >= 2 ? `${num(lang, good.length)} × PDF → ${num(lang, totalPages)} ${t(lang, "pagesSuffix")}` : t(lang, "mergeNeedTwo")}
        </p>

        <div className="row end">
          <button className="ghost" onClick={close}>
            {t(lang, "cancel")}
          </button>
          <button className="primary" disabled={good.length < 2 || busy} onClick={run}>
            {busy ? t(lang, "working") : t(lang, "mergeSave")}
          </button>
        </div>
      </div>
    </div>
  );
}
