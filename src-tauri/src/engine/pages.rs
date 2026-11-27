//! Page tools (SRS Module 3): rotate, delete, reorder, insert, extract,
//! split and merge.
//!
//! Every function works on a `PdfDocument` in memory. Pages are moved as
//! whole PDF objects, so their text (including Bangla ActualText), images,
//! links and annotations are kept exactly as they were.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use mupdf::pdf::{InsertPdfOptions, InsertPosition, PageSelection, PdfDocument, PdfWriteOptions};
use mupdf::{Outline, Size};

fn count(doc: &PdfDocument) -> Result<usize> {
    Ok(doc.page_count()?.max(0) as usize)
}

/// Check page numbers (zero-based), drop duplicates and sort them.
fn checked(doc: &PdfDocument, pages: &[i32]) -> Result<Vec<usize>> {
    let n = count(doc)?;
    if pages.is_empty() {
        bail!("no pages selected");
    }
    let mut set = BTreeSet::new();
    for &p in pages {
        if p < 0 || p as usize >= n {
            bail!("page {} does not exist", p + 1);
        }
        set.insert(p as usize);
    }
    Ok(set.into_iter().collect())
}

/// Turn pages by `degrees` (a multiple of 90; positive is clockwise).
pub fn rotate(doc: &mut PdfDocument, pages: &[i32], degrees: i32) -> Result<()> {
    if degrees % 90 != 0 {
        bail!("pages can only be turned by a multiple of 90 degrees");
    }
    for p in checked(doc, pages)? {
        let mut page = doc.load_pdf_page(p as i32)?;
        let now = page.rotation()?;
        page.set_rotation((now + degrees).rem_euclid(360))?;
    }
    Ok(())
}

/// Delete pages. A PDF must keep at least one page.
///
/// Bookmarks that pointed at a deleted page are removed (MuPDF would
/// otherwise send them to page 1); the rest keep pointing at their pages.
pub fn delete(doc: &mut PdfDocument, pages: &[i32]) -> Result<()> {
    let pages = checked(doc, pages)?;
    if pages.len() >= count(doc)? {
        bail!("CANNOT_DELETE_ALL");
    }
    let outline = doc.outlines().unwrap_or_default();
    let gone: BTreeSet<u32> = pages.iter().map(|&p| p as u32).collect();
    doc.delete_pages(PageSelection::Pages(pages))?;
    if outline_mentions(&outline, &gone) {
        doc.set_outlines(&outline_without(outline, &gone))?;
    }
    Ok(())
}

fn outline_mentions(items: &[Outline], gone: &BTreeSet<u32>) -> bool {
    items.iter().any(|o| {
        o.dest.is_some_and(|d| gone.contains(&d.loc.page_number)) || outline_mentions(&o.down, gone)
    })
}

/// The outline after deleting `gone` pages: renumbered, with bookmarks to
/// deleted pages dropped (kept as plain headings if they have children).
fn outline_without(items: Vec<Outline>, gone: &BTreeSet<u32>) -> Vec<Outline> {
    items
        .into_iter()
        .filter_map(|mut o| {
            o.down = outline_without(std::mem::take(&mut o.down), gone);
            match o.dest {
                Some(d) if gone.contains(&d.loc.page_number) => {
                    if o.down.is_empty() {
                        return None;
                    }
                    o.dest = None;
                }
                Some(ref mut d) => {
                    let before = gone.range(..d.loc.page_number).count() as u32;
                    d.loc.page_number -= before;
                    d.loc.page_in_chapter = d.loc.page_number;
                    d.loc.chapter = 0;
                }
                None => {}
            }
            Some(o)
        })
        .collect()
}

/// Put the pages in a new order. `order[i]` is the current number of the page
/// that should end up at position `i`; it must list every page exactly once.
pub fn reorder(doc: &mut PdfDocument, order: &[i32]) -> Result<()> {
    let n = count(doc)?;
    let mut seen = vec![false; n];
    if order.len() != n {
        bail!("the new order must list all {n} pages");
    }
    for &p in order {
        if p < 0 || p as usize >= n || std::mem::replace(&mut seen[p as usize], true) {
            bail!("the new order is not a list of every page once");
        }
    }
    // `now[k]` is the original number of the page currently at position k.
    let mut now: Vec<usize> = (0..n).collect();
    for (target, &want) in order.iter().enumerate() {
        let from = now.iter().position(|&p| p == want as usize).expect("checked above");
        if from != target {
            doc.move_page(from, target)?;
            let page = now.remove(from);
            now.insert(target, page);
        }
    }
    Ok(())
}

/// The order that moves `pages` (kept in their current order) so they sit
/// just before the page now at position `before` (`before == count` means at
/// the end). Used for drag and drop in the thumbnails.
pub fn order_for_move(page_count: i32, pages: &[i32], before: i32) -> Vec<i32> {
    let moving: BTreeSet<i32> = pages.iter().copied().filter(|p| (0..page_count).contains(p)).collect();
    let rest: Vec<i32> = (0..page_count).filter(|p| !moving.contains(p)).collect();
    let at = rest.iter().filter(|&&p| p < before).count();
    let mut order = rest[..at].to_vec();
    order.extend(moving.iter().copied());
    order.extend_from_slice(&rest[at..]);
    order
}

/// Insert an empty page of the given size (points) at position `at`.
pub fn insert_blank(doc: &mut PdfDocument, at: i32, size: [f32; 2]) -> Result<()> {
    let n = count(doc)? as i32;
    let at = at.clamp(0, n);
    let (w, h) = (size[0].clamp(72.0, 14400.0), size[1].clamp(72.0, 14400.0));
    doc.new_page_at(at, Size::new(w, h))?;
    Ok(())
}

/// Insert every page of another PDF at position `at`. Returns how many pages
/// were added.
pub fn insert_document(doc: &mut PdfDocument, at: i32, other: &PdfDocument) -> Result<usize> {
    let n = count(doc)?;
    let at = (at.max(0) as usize).min(n);
    let result = doc.insert_pdf(
        other,
        InsertPdfOptions { target: InsertPosition::Before(at), ..Default::default() },
    )?;
    Ok(result.page_count)
}

/// Open another PDF from disk for inserting or merging. Encrypted files need
/// their password.
pub fn load(path: &Path, password: Option<&str>) -> Result<PdfDocument> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let mut doc = PdfDocument::from_bytes(&bytes)
        .map_err(|e| anyhow!("{} could not be opened as a PDF ({e})", file_name(path)))?;
    if doc.needs_password()? {
        match password {
            Some(pw) if doc.authenticate(pw)? => {}
            Some(_) => bail!("WRONG_PASSWORD"),
            None => bail!("NEEDS_PASSWORD"),
        }
    }
    Ok(doc)
}

fn file_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Options for files written to disk: drop objects no page uses any more
/// (so deleting pages makes the file smaller) and compress new streams.
pub fn save_options() -> PdfWriteOptions {
    let mut opts = PdfWriteOptions::default();
    opts.set_garbage_level(1).set_compress(true);
    opts
}

pub fn to_bytes(doc: &PdfDocument, opts: PdfWriteOptions) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    doc.write_to_with_options(&mut out, opts)?;
    Ok(out)
}

/// Write bytes to `dest` safely: a temporary file in the same folder first,
/// then a rename over the target. A crash or full disk never leaves a
/// half-written PDF in place of the user's file (SRS 4.5).
pub fn write_file_safely(dest: &Path, bytes: &[u8]) -> Result<()> {
    let dir = dest.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let mut tmp = tempfile::Builder::new()
        .prefix(".akkhor-")
        .suffix(".tmp")
        .tempfile_in(dir)
        .with_context(|| format!("cannot write in {}", dir.display()))?;
    std::io::Write::write_all(&mut tmp, bytes)?;
    tmp.as_file().sync_all()?;
    tmp.persist(dest).map_err(|e| anyhow!("saving {}: {}", dest.display(), e.error))?;
    Ok(())
}

/// A new PDF holding only `pages` (in document order), as bytes.
/// `source` is the current document saved to bytes.
pub fn extract(source: &[u8], password: Option<&str>, pages: &[i32]) -> Result<Vec<u8>> {
    let mut copy = PdfDocument::from_bytes(source)?;
    if let Some(pw) = password {
        if copy.needs_password()? {
            copy.authenticate(pw)?;
        }
    }
    let keep = checked(&copy, pages)?;
    copy.select_pages(PageSelection::Pages(keep))?;
    to_bytes(&copy, save_options())
}

/// Parse page ranges typed by the user, such as "1-3, 5, 8-10", into groups
/// of zero-based page numbers. Bangla digits are accepted.
pub fn parse_ranges(text: &str, page_count: i32) -> Result<Vec<Vec<i32>>> {
    let ascii: String = text
        .chars()
        .map(|c| match c {
            '০'..='৯' => char::from(b'0' + (c as u32 - '০' as u32) as u8),
            '–' | '—' => '-',
            '،' | '؛' | ';' => ',',
            other => other,
        })
        .collect();
    let mut groups = Vec::new();
    for part in ascii.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let (a, b) = match part.split_once('-') {
            Some((a, b)) => (a.trim(), b.trim()),
            None => (part, part),
        };
        let parse = |s: &str| s.parse::<i32>().map_err(|_| anyhow!("BAD_RANGE:{part}"));
        let start = parse(a)?;
        let end = if b.is_empty() { page_count } else { parse(b)? };
        if start < 1 || end > page_count || start > end {
            bail!("BAD_RANGE:{part}");
        }
        groups.push((start - 1..end).collect());
    }
    if groups.is_empty() {
        bail!("BAD_RANGE:");
    }
    Ok(groups)
}

/// Groups for "every `n` pages".
pub fn every_n(page_count: i32, n: i32) -> Vec<Vec<i32>> {
    let n = n.max(1);
    (0..page_count).step_by(n as usize).map(|s| (s..(s + n).min(page_count)).collect()).collect()
}

/// A file name in `dir` that does not exist yet: "name.pdf", "name (2).pdf", …
pub fn unused_path(dir: &Path, stem: &str) -> PathBuf {
    let first = dir.join(format!("{stem}.pdf"));
    if !first.exists() {
        return first;
    }
    (2..)
        .map(|i| dir.join(format!("{stem} ({i}).pdf")))
        .find(|p| !p.exists())
        .expect("some name is free")
}

/// Split into one file per group, written into `dir` as "<stem> part N.pdf".
/// Existing files are never overwritten. Returns the files written.
pub fn split(source: &[u8], password: Option<&str>, groups: &[Vec<i32>], dir: &Path, stem: &str) -> Result<Vec<PathBuf>> {
    if groups.is_empty() {
        bail!("nothing to split");
    }
    let mut written = Vec::new();
    for (i, group) in groups.iter().enumerate() {
        let bytes = extract(source, password, group)?;
        let path = unused_path(dir, &format!("{stem} part {}", i + 1));
        write_file_safely(&path, &bytes)?;
        written.push(path);
    }
    Ok(written)
}

/// One file to merge, with its password if it has one.
pub struct MergeInput {
    pub path: PathBuf,
    pub password: Option<String>,
}

/// Join several PDFs, in the given order, into one new PDF (as bytes).
pub fn merge(inputs: &[MergeInput]) -> Result<Vec<u8>> {
    if inputs.len() < 2 {
        bail!("choose at least two files to merge");
    }
    let mut out = PdfDocument::new();
    for input in inputs {
        let other = load(&input.path, input.password.as_deref())
            .map_err(|e| anyhow!("{}: {e:#}", file_name(&input.path)))?;
        let at = count(&out)? as i32;
        insert_document(&mut out, at, &other)?;
    }
    to_bytes(&out, save_options())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn move_order() {
        // Move pages 1 and 3 (zero-based) before page 0.
        assert_eq!(order_for_move(5, &[1, 3], 0), vec![1, 3, 0, 2, 4]);
        // Move page 0 to the end.
        assert_eq!(order_for_move(4, &[0], 4), vec![1, 2, 3, 0]);
        // Move page 0 to just before page 2.
        assert_eq!(order_for_move(4, &[0], 2), vec![1, 0, 2, 3]);
        // Dropping a page onto itself changes nothing.
        assert_eq!(order_for_move(3, &[1], 1), vec![0, 1, 2]);
    }

    #[test]
    fn ranges() {
        assert_eq!(parse_ranges("1-3, 5", 10).unwrap(), vec![vec![0, 1, 2], vec![4]]);
        assert_eq!(parse_ranges("১-২, ৮-", 10).unwrap(), vec![vec![0, 1], vec![7, 8, 9]]);
        assert!(parse_ranges("0-2", 10).is_err());
        assert!(parse_ranges("5-3", 10).is_err());
        assert!(parse_ranges("4-11", 10).is_err());
        assert!(parse_ranges("abc", 10).is_err());
        assert!(parse_ranges(" , ", 10).is_err());
        assert_eq!(every_n(5, 2), vec![vec![0, 1], vec![2, 3], vec![4]]);
    }
}
