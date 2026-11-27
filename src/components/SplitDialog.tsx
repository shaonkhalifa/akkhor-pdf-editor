// Split one PDF into several files (SRS FR-3.2): every N pages, each page on
// its own, or ranges typed by the user ("1-3, 4-10, 11-").
import { useMemo, useState } from "react";
import { message, open as openDialog } from "@tauri-apps/plugin-dialog";
import { splitPdf, type DocInfo, type SplitPlan } from "../api";
import { num, t } from "../i18n";
import { useApp } from "../store";

type Mode = "every" | "each" | "ranges";

/** Check typed ranges like the Rust side does, to show problems while typing. */
function countRanges(text: string, pageCount: number): number | string {
  const ascii = text.replace(/[০-৯]/g, (d) => String("০১২৩৪৫৬৭৮৯".indexOf(d))).replace(/[–—]/g, "-");
  const parts = ascii.split(/[,;]/).map((p) => p.trim()).filter(Boolean);
  if (!parts.length) return "";
  for (const part of parts) {
    const m = /^(\d+)\s*(?:-\s*(\d*))?$/.exec(part);
    if (!m) return part;
    const start = Number(m[1]);
    const end = m[2] === undefined ? start : m[2] === "" ? pageCount : Number(m[2]);
    if (start < 1 || end > pageCount || start > end) return part;
  }
  return parts.length;
}

export default function SplitDialog({ doc, onClose }: { doc: DocInfo; onClose: () => void }) {
  const lang = useApp((s) => s.lang);
  const notify = useApp((s) => s.notify);
  const [mode, setMode] = useState<Mode>("every");
  const [every, setEvery] = useState(Math.min(10, Math.max(1, Math.ceil(doc.page_count / 2))));
  const [ranges, setRanges] = useState("");
  const [busy, setBusy] = useState(false);
  const stem = doc.name.replace(/\.pdf$/i, "");

  const result = useMemo(() => {
    if (mode === "each") return doc.page_count;
    if (mode === "every") return every >= 1 ? Math.ceil(doc.page_count / every) : "";
    return countRanges(ranges, doc.page_count);
  }, [mode, every, ranges, doc.page_count]);
  const ok = typeof result === "number" && result >= 1;

  const run = async () => {
    const plan: SplitPlan =
      mode === "ranges" ? { mode: "ranges", text: ranges } : { mode: "every", n: mode === "each" ? 1 : every };
    const dir = await openDialog({ directory: true, multiple: false, defaultPath: doc.path.replace(/[\\/][^\\/]*$/, "") });
    if (typeof dir !== "string") return;
    setBusy(true);
    try {
      const files = await splitPdf(doc.id, doc.page_count, plan, dir, stem);
      onClose();
      notify(t(lang, "splitDone", { n: num(lang, files.length), folder: dir }));
    } catch (e) {
      await message(String(e), { kind: "error", title: t(lang, "splitTitle") });
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="overlay" role="dialog" aria-modal="true" aria-labelledby="split-title" onKeyDown={(e) => e.key === "Escape" && onClose()}>
      <form
        className="dialog wide"
        onSubmit={(e) => {
          e.preventDefault();
          if (ok && !busy) run();
        }}
      >
        <h2 id="split-title">
          {t(lang, "splitTitle")} — {doc.name}
        </h2>

        <label className="choice">
          <input type="radio" name="split" checked={mode === "every"} onChange={() => setMode("every")} />
          {t(lang, "splitEvery")}
          <input
            type="number"
            className="small-num"
            min={1}
            max={doc.page_count}
            value={every}
            onFocus={() => setMode("every")}
            onChange={(e) => setEvery(Math.max(1, Math.floor(Number(e.target.value) || 1)))}
          />
          {t(lang, "splitEveryTail")}
        </label>
        <label className="choice">
          <input type="radio" name="split" checked={mode === "each"} onChange={() => setMode("each")} />
          {t(lang, "splitEach")}
        </label>
        <label className="choice">
          <input type="radio" name="split" checked={mode === "ranges"} onChange={() => setMode("ranges")} />
          {t(lang, "splitRanges")}
          <input
            type="text"
            className="grow"
            placeholder={t(lang, "rangesHint")}
            value={ranges}
            onFocus={() => setMode("ranges")}
            onChange={(e) => setRanges(e.target.value)}
          />
        </label>

        <p className={typeof result === "string" && result ? "error" : "muted"}>
          {typeof result === "number"
            ? t(lang, "splitWillMake", { n: num(lang, result) })
            : result
              ? t(lang, "badRange", { r: result, n: num(lang, doc.page_count) })
              : t(lang, "rangesHint")}
        </p>
        <p className="muted small">{t(lang, "splitNote", { stem })}</p>
        {doc.dirty && <p className="muted small">{t(lang, "splitIncludesChanges")}</p>}

        <div className="row end">
          <button type="button" className="ghost" onClick={onClose}>
            {t(lang, "cancel")}
          </button>
          <button type="submit" className="primary" disabled={!ok || busy}>
            {busy ? t(lang, "working") : t(lang, "chooseFolder")}
          </button>
        </div>
      </form>
    </div>
  );
}
