// Printing (FR-1.7): draw every page at 150 dpi, then open the Windows print
// dialog, which offers page ranges, copies, printer and fit-to-page.
import { renderPagePng, type DocInfo } from "./api";

const PRINT_SCALE = 150 / 72;

export async function printDocument(doc: DocInfo, onProgress: (done: number) => void): Promise<void> {
  const root = document.getElementById("print-root");
  if (!root) return;
  root.innerHTML = "";
  const urls: string[] = [];
  try {
    for (let i = 0; i < doc.page_count; i++) {
      const blob = await renderPagePng(doc.id, i, PRINT_SCALE);
      const url = URL.createObjectURL(blob);
      urls.push(url);
      const img = document.createElement("img");
      img.src = url;
      img.alt = "";
      root.appendChild(img);
      onProgress(i + 1);
    }
    await Promise.all(
      Array.from(root.querySelectorAll("img")).map((img) =>
        img.complete ? Promise.resolve() : new Promise((r) => (img.onload = img.onerror = r)),
      ),
    );
    window.print();
  } finally {
    root.innerHTML = "";
    urls.forEach(URL.revokeObjectURL);
  }
}
