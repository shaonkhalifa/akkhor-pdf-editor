// Opening files: file dialog, drag and drop, recent list, password prompt.
import { ask, message, open as openDialog } from "@tauri-apps/plugin-dialog";
import { openPdf, type DocInfo } from "./api";
import { fileSize, t } from "./i18n";
import { useApp } from "./store";

/** Set by the password dialog component; returns null if the user cancels. */
let promptPassword: ((name: string, wrong: boolean) => Promise<string | null>) | null = null;
export function registerPasswordPrompt(fn: typeof promptPassword) {
  promptPassword = fn;
}

function baseName(path: string) {
  return path.split(/[\\/]/).pop() ?? path;
}

export async function openPath(path: string): Promise<DocInfo | null> {
  const { lang, tabs, activate, addTab } = useApp.getState();
  const already = tabs.find((tab) => tab.doc.path === path);
  if (already) {
    activate(already.doc.id);
    return already.doc;
  }

  let password: string | undefined;
  let force = false;
  for (;;) {
    try {
      const doc = await openPdf(path, password, force);
      addTab(doc);
      if (doc.page_count > 2000) {
        await message(t(lang, "manyPages", { n: doc.page_count }), { kind: "info", title: doc.name });
      }
      return doc;
    } catch (e) {
      const err = String(e);
      if (err.startsWith("LARGE_FILE:")) {
        const size = fileSize(lang, Number(err.split(":")[1]));
        const yes = await ask(t(lang, "largeFile", { size }), { kind: "warning", title: baseName(path) });
        if (!yes) return null;
        force = true;
        continue;
      }
      if ((err === "NEEDS_PASSWORD" || err === "WRONG_PASSWORD") && promptPassword) {
        const pw = await promptPassword(baseName(path), err === "WRONG_PASSWORD");
        if (pw === null) return null;
        password = pw;
        continue;
      }
      await message(`${t(lang, "cannotOpen")}\n\n${err}`, { kind: "error", title: baseName(path) });
      return null;
    }
  }
}

export async function pickAndOpen() {
  const { lang } = useApp.getState();
  const chosen = await openDialog({
    multiple: true,
    directory: false,
    filters: [{ name: t(lang, "pdfFiles"), extensions: ["pdf"] }],
  });
  const paths = Array.isArray(chosen) ? chosen : chosen ? [chosen] : [];
  for (const p of paths) await openPath(p);
}
