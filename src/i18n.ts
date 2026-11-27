// Interface text in Bangla and English (SRS FR-13.1).

export type Lang = "bn" | "en";

const strings = {
  appName: { bn: "অক্ষর PDF Editor", en: "Akkhor PDF Editor" },
  tagline: {
    bn: "বিনামূল্যে, অফলাইনে বাংলা ও ইংরেজি PDF এডিটর",
    en: "Free, offline Bangla & English PDF editor",
  },
  offline: { bn: "ইন্টারনেট ব্যবহার করে না", en: "No internet used" },
  langSwitch: { bn: "English", en: "বাংলা" },
  error: { bn: "ত্রুটি", en: "Error" },

  // Home
  openPdf: { bn: "PDF খুলুন", en: "Open PDF" },
  dropHint: { bn: "অথবা এখানে PDF ফাইল টেনে আনুন", en: "or drag a PDF file here" },
  dropNow: { bn: "ছেড়ে দিন — PDF খুলবে", en: "Drop to open the PDF" },
  recent: { bn: "সাম্প্রতিক ফাইল", en: "Recent files" },
  clearRecent: { bn: "তালিকা মুছুন", en: "Clear list" },
  noRecent: { bn: "এখনো কোনো ফাইল খোলা হয়নি", en: "No files opened yet" },
  selfTest: { bn: "বাংলা শেপিং পরীক্ষা", en: "Bangla shaping self-test" },
  shortcuts: {
    bn: "Ctrl+O খুলুন · Ctrl+F খুঁজুন · Ctrl+P প্রিন্ট · Ctrl+W বন্ধ",
    en: "Ctrl+O open · Ctrl+F find · Ctrl+P print · Ctrl+W close",
  },

  // Viewer
  close: { bn: "বন্ধ", en: "Close" },
  zoomIn: { bn: "বড় করুন", en: "Zoom in" },
  zoomOut: { bn: "ছোট করুন", en: "Zoom out" },
  fitWidth: { bn: "প্রস্থ মিলিয়ে", en: "Fit width" },
  fitPage: { bn: "পুরো পাতা", en: "Fit page" },
  page: { bn: "পাতা", en: "Page" },
  of: { bn: "এর", en: "of" },
  search: { bn: "খুঁজুন…", en: "Search…" },
  noMatches: { bn: "পাওয়া যায়নি", en: "No matches" },
  prev: { bn: "আগের", en: "Previous" },
  next: { bn: "পরের", en: "Next" },
  print: { bn: "প্রিন্ট", en: "Print" },
  saveAs: { bn: "আলাদা সেভ", en: "Save As" },
  saved: { bn: "সেভ হয়েছে", en: "Saved" },
  sidebar: { bn: "পাশের তালিকা", en: "Sidebar" },
  thumbnails: { bn: "পাতাগুলো", en: "Pages" },
  bookmarks: { bn: "বুকমার্ক", en: "Bookmarks" },
  noBookmarks: { bn: "এই PDF-এ কোনো বুকমার্ক নেই", en: "This PDF has no bookmarks" },
  preparingPrint: { bn: "প্রিন্টের জন্য পাতা তৈরি হচ্ছে…", en: "Preparing pages for printing…" },
  externalLink: {
    bn: "বাইরের লিংক (অফলাইন অ্যাপ, তাই খোলা হবে না):",
    en: "External link (not opened — this app works offline):",
  },

  // Dialogs
  passwordTitle: { bn: "এই PDF পাসওয়ার্ড দিয়ে সুরক্ষিত", en: "This PDF is password-protected" },
  passwordWrong: { bn: "পাসওয়ার্ড ভুল, আবার চেষ্টা করুন", en: "Wrong password, try again" },
  passwordLabel: { bn: "পাসওয়ার্ড", en: "Password" },
  openBtn: { bn: "খুলুন", en: "Open" },
  cancel: { bn: "বাতিল", en: "Cancel" },
  largeFile: {
    bn: "ফাইলটি অনেক বড় ({size})। খুলতে সময় ও মেমরি বেশি লাগতে পারে। তবুও খুলবেন?",
    en: "This file is very large ({size}). Opening it may be slow and use a lot of memory. Open anyway?",
  },
  manyPages: {
    bn: "এই PDF-এ {n}টি পাতা আছে — চলতে একটু ধীর হতে পারে।",
    en: "This PDF has {n} pages — it may feel slower than usual.",
  },
  cannotOpen: { bn: "ফাইলটি খোলা গেল না", en: "The file could not be opened" },
  pdfFiles: { bn: "PDF ফাইল", en: "PDF files" },

  // Self-test (Phase 1)
  phase: { bn: "বাংলা শেপিং পরীক্ষা", en: "Bangla shaping self-test" },
  intro: {
    bn: "এই পরীক্ষা প্রতিটি বাংলা ফন্টে একটি PDF তৈরি করে, লেখাটি আবার পড়ে দেখে হুবহু একই আছে কিনা, এবং একটি PDF-এর বাংলা লেখা বদলে দেখে।",
    en: "This test makes a PDF with each Bangla font, reads the text back to check it is exactly the same, and edits Bangla words inside a PDF.",
  },
  run: { bn: "পরীক্ষা চালান", en: "Run self-test" },
  running: { bn: "চলছে…", en: "Running…" },
  allPassed: { bn: "সব পরীক্ষা পাস করেছে", en: "All checks passed" },
  someFailed: { bn: "কিছু পরীক্ষা ব্যর্থ হয়েছে", en: "Some checks failed" },
  pass: { bn: "পাস", en: "Pass" },
  fail: { bn: "ব্যর্থ", en: "Fail" },
  fonts: { bn: "ফন্ট অনুযায়ী ফলাফল", en: "Results by font" },
  textBack: { bn: "PDF থেকে পড়া লেখা", en: "Text read back from the PDF" },
  spike: { bn: "লেখা বদলানোর পরীক্ষা", en: "Text-edit test" },
  spikeDesc: {
    bn: "“আমি বাংলায় গান গাই” বদলে “আমরা বিজ্ঞান ও ক্ষমতা শিখি” করা হয়েছে, তারপর সেভ করে আবার খোলা হয়েছে।",
    en: "“আমি বাংলায় গান গাই” was replaced with “আমরা বিজ্ঞান ও ক্ষমতা শিখি”, then saved and reopened.",
  },
  before: { bn: "আগে", en: "Before" },
  after: { bn: "পরে", en: "After" },
  back: { bn: "ফিরে যান", en: "Back" },
} as const;

export type Key = keyof typeof strings;

export function t(lang: Lang, key: Key, vars?: Record<string, string | number>): string {
  let s: string = strings[key][lang];
  if (vars) for (const [k, v] of Object.entries(vars)) s = s.replace(`{${k}}`, String(v));
  return s;
}

/** Show numbers with Bangla digits in the Bangla interface. */
export function num(lang: Lang, n: number | string): string {
  const s = String(n);
  return lang === "bn" ? s.replace(/[0-9]/g, (d) => "০১২৩৪৫৬৭৮৯"[Number(d)]) : s;
}

export function fileSize(lang: Lang, bytes: number): string {
  const mb = bytes / (1024 * 1024);
  return `${num(lang, mb >= 1024 ? (mb / 1024).toFixed(1) : Math.round(mb))} ${mb >= 1024 ? "GB" : "MB"}`;
}
