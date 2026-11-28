# Akkhor PDF Editor (অক্ষর PDF Editor)

A **free, open-source, offline** PDF editor for Windows that handles **Bangla and English text correctly**.

- Bangla conjuncts (যুক্তাক্ষর) stay joined, and vowel signs such as ি and ে stay in the right place, even after editing and saving.
- Everything happens on your own PC. The app contains **no network code**, so your files never leave your computer.
- Free forever, licensed under **AGPL-3.0-or-later**.

> **Status: Phase 8A — image tools.** Resize, convert and shrink pictures,
> and remove plain backgrounds; plus everything from Phases 1–7. See the
> [roadmap](#roadmap).

## What you can do (v0.8)

### Image tools (new in v0.8)

On the home screen, **Image tools** works on picture files directly (no PDF needed):

- **Resize** by pixels (keep the shape, or crop to an exact size) or by percent; presets such as passport photo 300×300 and signature 300×80.
- **Convert** between JPG, PNG and WebP. Choose the JPG quality and the colour behind transparent parts.
- **Shrink to a file size** ("at most 100 KB") for online application forms: the quality is lowered step by step, then the size if needed — never below 64 pixels.
- **Many pictures at once**: save them all to a folder (nothing is overwritten).
- **Remove a plain background** (white wall, paper, one-colour backdrop): picked automatically or by clicking it, with tolerance and soft-edge sliders, keeping same-coloured parts inside the subject, and an option to trim. Saves a transparent PNG.
- Side-by-side preview with the result's size before saving. Everything runs on your PC; no AI model, no internet.

### Bijoy to Unicode and OCR

- **Bijoy text is found automatically** by its font (SutonnyMJ and other "MJ" fonts) and outlined in orange in Edit text mode.
- **Convert from Bijoy**: one block in the text editor, or every Bijoy block in the document (More → Convert all Bijoy text to Unicode). The text becomes real Unicode Bangla, so it can be searched, copied and edited. Undo works.
- **Paste Bijoy text** into a text box and it goes in as Unicode automatically (with an "undo conversion" link).
- **OCR — make scanned pages searchable** (More → Make searchable): recognises Bangla and English with Tesseract, inside the app, on your PC. The page looks the same, with the recognised text invisible underneath. Pages that already have text are skipped.
- **Read with OCR**: for text that can't be read (broken fonts), recognise it from the page instead.
- **OCR language pack**: the Bangla + English models (about 22 MB) are a separate, free, offline installer, `Akkhor-OCR-Pack-ben-eng-<version>.exe`, on the GitHub Releases page. The app never downloads anything. Build it with `scripts\build-ocr-pack.ps1`.

The Bijoy converter is ported from [bijoy2unicode](https://github.com/Mad-FOX/bijoy2unicode) by Rajib Biswas (MPL-2.0); `src-tauri/src/engine/bijoy.rs` stays under the MPL-2.0. OCR uses [Tesseract](https://github.com/tesseract-ocr/tesseract) (Apache-2.0), compiled into MuPDF, and the [tessdata_best](https://github.com/tesseract-ocr/tessdata_best) models (Apache-2.0).

### Create, convert and forms

- **New blank PDF**: A4, Letter, Legal, A5 or a custom size, portrait or landscape, any number of pages.
- **Pictures to PDF**: one picture per page, in your order; centred on A4 pages (landscape for wide pictures) or pages the size of each picture.
- **Save pages as pictures**: PNG or JPG at 72–600 dpi, for all or selected pages.
- **Compress**: low (tidy the file), medium (pictures above 150 dpi reduced) or high (above 100 dpi, for email). Shows the size before and after. Text stays searchable.
- **Password**: save a copy protected with AES-256, or a copy without the password.
- **Page numbers, header, footer and watermark** in Bangla or English, with Bangla digits (পৃষ্ঠা ১ / ৩), any size and colour; the watermark is see-through and can be turned.
- **Fill in PDF forms**: text fields, checkboxes, radio buttons and drop-downs, typed straight into the page. Bangla answers are drawn with proper shaping, so they show correctly in Adobe Reader and Chrome too. **Flatten** makes the answers part of the page.
- **Word to PDF**, if LibreOffice (free) or Microsoft Word is installed on your PC. Akkhor runs it hidden on your PC; nothing is uploaded.
- **Move text**: drag the ✥ handle of a text box (new or existing) to a new place.

### Text editing

- **Edit text**: click a block of text, change it, Apply (Ctrl+Enter). The old text is removed (only the text: pictures and lines under it stay) and the new text is shaped by HarfBuzz, so conjuncts such as ক্ষ্ম and vowel signs such as ি and ে come out right. It reads back as the exact Unicode you typed, so copy and search work.
- Bangla and English in one line: each character gets the right font automatically.
- Font, size, colour, bold, italic and alignment. The original font is matched (SRS 6.3): the same font if it is installed (Arial, Times New Roman, Calibri, SolaimanLipi, Kalpurush, Nikosh…), else a same-width substitute (Times, Helvetica/Arial widths) or a bundled Bangla font.
- Longer text wraps inside its box. A single line may grow to the page margin; when a paragraph no longer fits, the app asks: shrink the text to fit, or let it run past the box.
- **Add text**: click anywhere on a page to type a new text box; drag its right edge to set the width.
- **Highlight, underline, strikethrough**: drag over text.
- **Redact**: drag over an area to remove text, pictures and lines there for good, under a black box. The removed words are not in the saved file at all.
- Text that was not read correctly (old Bijoy fonts or broken font maps) is marked so you can check and retype it. Converting Bijoy comes in Phase 7.
- Every change can be undone (Ctrl+Z).

Known limits: one style per block (a block with mixed bold and normal words becomes all one style); text on pages turned 90° can't be edited yet; Bangla has no italic font, so italic applies to English only.

### Pictures, stamps and signatures

- Insert PNG, JPG or WebP pictures. Very large photos are shrunk automatically (longest side 2400 px), so the PDF stays small.
- Before placing: drag to move, drag the corner to resize (keeps the shape), drag the round handle to turn (snaps to straight angles; Shift for 15° steps), set opacity for logos and watermarks. Enter places it, Esc cancels, arrow keys nudge.
- **Edit images**: click an image already in the PDF to delete it or replace it with another picture (same place and size). Deleted pictures are removed from the saved file.
- **Stamps**: অনুমোদিত / APPROVED, পরিশোধিত / PAID, গৃহীত / RECEIVED, খসড়া / DRAFT, গোপনীয় / CONFIDENTIAL, বাতিল / REJECTED, optionally with today's date, plus your own round seal or rectangular stamp in any colour.
- **Signatures**: draw with mouse, touchpad or pen (pen pressure changes the line), type your name in a handwriting font (Dancing Script, Great Vibes, or Bangla), or use a photo of your signature on paper (the paper is made transparent). Choose black, blue or any ink colour, and add your name and today's date below.
- **My signatures**: keep signatures for next time. They are encrypted with Windows' own data protection (DPAPI), tied to your Windows login, and never leave your PC.
- Everything above can be undone (Ctrl+Z).

### Page tools

- Select pages in the thumbnails (Ctrl/Shift+click for several, Ctrl+A for all)
- Drag thumbnails to put pages in a new order
- Rotate left or right (Ctrl+[ / Ctrl+]), delete (Delete key), insert a blank page, or insert pages from another PDF; right-click a thumbnail for the same menu
- Extract the selected pages to a new PDF
- Split a PDF every N pages, one file per page, or by ranges such as `1-3, 4-10, 11-` (Bangla digits work too)
- Merge several PDFs in the order you choose
- Undo and redo every page change (Ctrl+Z / Ctrl+Y), up to 50 steps
- Save (Ctrl+S) or Save As (Ctrl+Shift+S). Saving writes a temporary file first and then replaces the original, so a crash or a full disk never leaves a half-written PDF. The app asks before closing a file with unsaved changes.
- Password-protected PDFs stay protected after editing
- Bookmarks follow their pages when pages move; bookmarks of deleted pages are removed

### Viewer

- Open PDFs with the Open button, Ctrl+O, drag and drop, or the recent files list
- Open several PDFs in tabs; open password-protected PDFs
- Scroll smoothly, zoom 25–400%, fit width or fit page, jump to a page
- Page thumbnails and the PDF's bookmarks in the sidebar; internal links work
- Search Bangla and English text with highlights (Ctrl+F, Enter / Shift+Enter, F3)
- Select and copy text as correct Unicode
- Print (Ctrl+P) with the Windows print dialog
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
| Bundled fonts | Noto Sans Bengali, Noto Serif Bengali, Hind Siliguri; Dancing Script and Great Vibes for typed signatures (all SIL OFL) |
| Pictures | [`image`](https://crates.io/crates/image) crate (PNG, JPEG, WebP) |

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
  extract.rs             read text and positions back (any page rotation)
  viewer.rs              open documents, drawing, search, undo history
  pages.rs               page tools: rotate, delete, reorder, insert, split, merge
  images.rs              pictures: shrink, place (rotation, opacity), find, delete, replace
  signatures.rs          "My signatures", encrypted with Windows DPAPI
  textedit.rs            text blocks, wrapping, replace/add/move text, highlight, redact, page numbers, watermark
  convert.rs             blank PDF, pictures to PDF, pages to pictures, compress, password
  forms.rs               fill and flatten forms, with Bangla appearances
  office.rs              Word to PDF through LibreOffice or Word, if installed
  bijoy.rs               Bijoy (SutonnyMJ) to Unicode (MPL-2.0, from bijoy2unicode)
  ocr.rs                 OCR with Tesseract: searchable pages, read an area
  imagetools.rs          resize, convert, target file size, plain background remover
ocr-pack/                the separate OCR language pack installer (NSIS)
  edit.rs                replace text in an existing PDF (spike)
  render.rs              page → PNG
  selftest.rs            SRS 6.4 shaping check + edit spike
src-tauri/tests/         acceptance tests and reference images
src-tauri/fonts/         bundled OFL fonts and their licences
scripts/                 helper scripts (no-network check)
```

## Roadmap

1. Setup, shaping test and text-edit spike ✅
2. PDF viewer and printing ✅
3. Page tools (merge, split, rotate, delete, reorder) ✅
4. Images and signatures ✅
5. Text editing, Bangla and English (core feature) ✅
6. Create, convert and forms ✅
7. Bijoy → Unicode and OCR ✅
8A. **Image tools (resize, convert, background remover)** ✅ ← you are here
8. (Later) Digital certificate signatures

## Contributing

Issues and pull requests are welcome, in Bangla or English. Please run `npm test` before opening a pull request.

## Licence

Akkhor PDF Editor is licensed under the **GNU Affero General Public License v3.0 or later**. See [LICENSE](LICENSE).
MuPDF is © Artifex Software and is used under the AGPL. The bundled fonts are under the SIL Open Font License. Their licence files are in [`src-tauri/fonts/`](src-tauri/fonts/) and [`src/assets/fonts/`](src/assets/fonts/).
