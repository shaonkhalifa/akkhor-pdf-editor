# Akkhor PDF Editor (অক্ষর PDF Editor)

A **free, open-source, offline** PDF editor for Windows that handles **Bangla and English text correctly**.

- Bangla conjuncts (যুক্তাক্ষর) stay joined, and vowel signs such as ি and ে stay in the right place, even after editing and saving.
- Everything happens on your own PC. The app contains **no network code**, so your files never leave your computer.
- Free forever, licensed under **AGPL-3.0-or-later**.

> **Status: Phase 2 of 8 — a working PDF viewer.** Open, read, zoom, search
> (Bangla and English), copy text, print and save copies of PDFs. Editing
> comes in later phases. See the [roadmap](#roadmap).

## What you can do (v0.2)

- Open PDFs with the Open button, Ctrl+O, drag and drop, or the recent files list
- Open several PDFs in tabs; open password-protected PDFs
- Scroll smoothly, zoom 25–400%, fit width or fit page, jump to a page
- Page thumbnails and the PDF's bookmarks in the sidebar; internal links work
- Search Bangla and English text with highlights (Ctrl+F, Enter / Shift+Enter, F3)
- Select and copy text as correct Unicode
- Print (Ctrl+P) with the Windows print dialog; Save As (Ctrl+S)
- Interface in Bangla or English

## What Phase 1 proves

| Check | How it's tested |
|---|---|
| Bangla text is shaped correctly | A PDF is made with each bundled Bangla font. The text is read back and must be identical Unicode, and each page render must match a stored reference image. |
| Existing Bangla text can be edited | The spike removes Bangla words from an existing PDF, inserts new words shaped by HarfBuzz, saves, reopens and checks the text. |
| Mixed Bangla + English lines | Each character gets the right font. Punctuation a Bangla font lacks falls back to the English font. |
| No network code | CI fails if any networking crate appears in the Rust dependency tree. |

Run the same checks inside the app with the **Run self-test** button.

### How Bangla text stays copyable

A shaped conjunct such as ক্ষ্ম is one glyph that stands for several letters. A vowel sign such as ে is typed after its consonant but drawn before it. A plain glyph-to-letter map cannot describe either, so Akkhor tags every text run with PDF `/ActualText` holding the exact Unicode that was typed. Adobe Reader, Chrome and MuPDF use it for copy and search.

Known limits in Phase 1:
- Each line is laid out on one row. Wrapping comes in Phase 5.
- The edit spike supports unrotated pages whose page box starts at (0, 0).

## Tech stack (all free)

| Part | Technology |
|---|---|
| App framework | [Tauri 2](https://tauri.app) |
| Core | Rust |
| PDF engine | [MuPDF](https://mupdf.com) via the [`mupdf`](https://crates.io/crates/mupdf) crate (AGPL) |
| Text shaping | HarfBuzz (inside MuPDF) |
| UI | React + TypeScript + Vite |
| Bundled fonts | Noto Sans Bengali, Noto Serif Bengali, Hind Siliguri (SIL OFL) |

## Build and run on Windows

You need (all free):

1. [Node.js](https://nodejs.org) 20 or newer
2. [Rust](https://rustup.rs) (stable, MSVC)
3. [Visual Studio 2022 Build Tools](https://visualstudio.microsoft.com/downloads/) with **Desktop development with C++**
4. [LLVM](https://github.com/llvm/llvm-project/releases). MuPDF's bindings need `libclang`. Set `LIBCLANG_PATH` to `C:\Program Files\LLVM\bin` if it isn't found.

Point bindgen at clang's own C headers. Without this, newer LLVM versions fail with ``cannot find type `max_align_t` ``:

```powershell
$env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
$env:BINDGEN_EXTRA_CLANG_ARGS = "-resource-dir `"$((& 'C:\Program Files\LLVM\bin\clang.exe' -print-resource-dir).Replace('\','/'))`""
```

Then:

```powershell
npm install
npm run tauri dev     # run the app
npm test              # run the Rust tests (shaping + spike)
npm run tauri build   # build the Windows installer
```

The first build compiles MuPDF from source and takes 10–20 minutes. Later builds are fast.

### Updating reference images

If you change fonts or layout on purpose, check the new renders in `src-tauri/test-output/`, then refresh the references:

```powershell
cd src-tauri
$env:AKKHOR_UPDATE_REFS = "1"; cargo test --test phase1; Remove-Item Env:AKKHOR_UPDATE_REFS
```

## Project layout

```
src/                     React UI
src-tauri/src/engine/    PDF engine (Rust)
  script.rs              Bangla / English script detection (FR-6.10)
  fonts.rs               bundled fonts and glyph coverage
  layout.rs              text → HarfBuzz-shaped PDF via MuPDF
  extract.rs             read text and positions back
  edit.rs                replace text in an existing PDF (spike)
  render.rs              page → PNG
  selftest.rs            SRS 6.4 shaping check + edit spike
src-tauri/tests/         acceptance tests and reference images
src-tauri/fonts/         bundled OFL fonts and their licences
scripts/                 helper scripts (no-network check)
```

## Roadmap

1. Setup, shaping test and text-edit spike ✅
2. **PDF viewer and printing** ✅ ← you are here
3. Page tools (merge, split, rotate, delete, reorder)
4. Images and signatures
5. Text editing, Bangla and English (core feature)
6. Create, convert and forms
7. Bijoy → Unicode and OCR
8. (Later) Digital certificate signatures

## Contributing

Issues and pull requests are welcome, in Bangla or English. Please run `npm test` before opening a pull request.

## Licence

Akkhor PDF Editor is licensed under the **GNU Affero General Public License v3.0 or later**. See [LICENSE](LICENSE).
MuPDF is © Artifex Software and is used under the AGPL. The bundled fonts are under the SIL Open Font License. Their licence files are in [`src-tauri/fonts/`](src-tauri/fonts/).
