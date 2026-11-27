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
    bn: "Ctrl+O খুলুন · Ctrl+S সেভ · Ctrl+Z ফিরুন · Ctrl+F খুঁজুন · Ctrl+P প্রিন্ট · Ctrl+W বন্ধ",
    en: "Ctrl+O open · Ctrl+S save · Ctrl+Z undo · Ctrl+F find · Ctrl+P print · Ctrl+W close",
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
  saveAs: { bn: "অন্য নামে সেভ", en: "Save As" },
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

  // Page tools (Phase 3)
  undo: { bn: "আগের অবস্থায় ফিরুন", en: "Undo" },
  redo: { bn: "আবার করুন", en: "Redo" },
  save: { bn: "সেভ", en: "Save" },
  rotateLeft: { bn: "বাঁয়ে ঘোরান", en: "Rotate left" },
  rotateRight: { bn: "ডানে ঘোরান", en: "Rotate right" },
  deletePages: { bn: "পাতা মুছুন", en: "Delete pages" },
  insertBlank: { bn: "ফাঁকা পাতা যোগ করুন", en: "Insert blank page" },
  insertFile: { bn: "অন্য PDF-এর পাতা যোগ করুন…", en: "Insert pages from a PDF…" },
  extract: { bn: "বাছাই করা পাতা নতুন PDF-এ…", en: "Extract pages to a new PDF…" },
  split: { bn: "ভাগ করুন", en: "Split" },
  merge: { bn: "PDF জোড়া দিন", en: "Merge PDFs" },
  selectedPages: { bn: "{n}টি পাতা বাছাই করা", en: "{n} pages selected" },
  selectAll: { bn: "সব পাতা বাছাই", en: "Select all pages" },
  dragHint: {
    bn: "ক্লিক করে বাছাই করুন (Ctrl/Shift দিয়ে একাধিক), টেনে নিয়ে ক্রম বদলান",
    en: "Click to select (Ctrl/Shift for more), drag to reorder",
  },
  cannotDeleteAll: {
    bn: "সব পাতা মুছে ফেলা যায় না — PDF-এ অন্তত একটি পাতা থাকতে হয়।",
    en: "You can't delete every page — a PDF needs at least one page.",
  },
  unsavedTitle: { bn: "সেভ করা হয়নি", en: "Unsaved changes" },
  unsavedOne: {
    bn: "“{name}”-এ পরিবর্তন সেভ করা হয়নি। বন্ধ করার আগে সেভ করবেন?",
    en: "“{name}” has changes that are not saved. Save them before closing?",
  },
  unsavedMany: {
    bn: "{n}টি ফাইলে সেভ না করা পরিবর্তন আছে। বন্ধ করার আগে সব সেভ করবেন?",
    en: "{n} files have changes that are not saved. Save them all before closing?",
  },
  saveBtn: { bn: "সেভ করুন", en: "Save" },
  dontSave: { bn: "সেভ করবেন না", en: "Don't save" },
  cannotSave: { bn: "সেভ করা গেল না", en: "Could not save" },
  extracted: { bn: "বাছাই করা পাতাগুলো নতুন PDF-এ সেভ হয়েছে", en: "The selected pages were saved as a new PDF" },
  pagesSuffix: { bn: "পাতা", en: "pages" },

  splitTitle: { bn: "PDF ভাগ করুন", en: "Split PDF" },
  splitEvery: { bn: "প্রতি", en: "Every" },
  splitEveryTail: { bn: "পাতায় একটি ফাইল", en: "pages make one file" },
  splitRanges: { bn: "নিজের মতো পরিসর:", en: "Custom ranges:" },
  rangesHint: { bn: "যেমন: 1-3, 4-10, 11-", en: "for example: 1-3, 4-10, 11-" },
  splitEach: { bn: "প্রতিটি পাতা আলাদা ফাইল", en: "Each page as its own file" },
  splitWillMake: { bn: "{n}টি ফাইল তৈরি হবে", en: "This makes {n} files" },
  badRange: { bn: "পরিসরটি ঠিক নেই: {r} (পাতা ১ থেকে {n})", en: "This range isn't valid: {r} (pages 1 to {n})" },
  chooseFolder: { bn: "ফোল্ডার বেছে নিয়ে ভাগ করুন…", en: "Choose a folder and split…" },
  splitDone: { bn: "{n}টি ফাইল সেভ হয়েছে:\n{folder}", en: "{n} files saved in:\n{folder}" },
  splitNote: {
    bn: "নতুন ফাইলের নাম হবে “{stem} part 1.pdf”, “part 2” … আগের কোনো ফাইল মোছা হবে না।",
    en: "Files are named “{stem} part 1.pdf”, “part 2” … Existing files are never overwritten.",
  },

  splitIncludesChanges: {
    bn: "ফাইলগুলো এখনকার অবস্থা থেকে তৈরি হবে, সেভ না করা পরিবর্তনসহ।",
    en: "The files use the document as it is now, including unsaved changes.",
  },

  mergeTitle: { bn: "PDF জোড়া দিন", en: "Merge PDFs" },
  mergeIntro: {
    bn: "ফাইলগুলো এই ক্রমে একটি নতুন PDF-এ জোড়া লাগবে। মূল ফাইলগুলো বদলাবে না।",
    en: "The files are joined in this order into a new PDF. The original files are not changed.",
  },
  addFiles: { bn: "ফাইল যোগ করুন…", en: "Add files…" },
  moveUp: { bn: "উপরে", en: "Move up" },
  moveDown: { bn: "নিচে", en: "Move down" },
  remove: { bn: "সরান", en: "Remove" },
  mergeSave: { bn: "জোড়া দিয়ে সেভ করুন…", en: "Merge and save…" },
  mergeNeedTwo: { bn: "অন্তত দুটি ফাইল যোগ করুন", en: "Add at least two files" },
  mergeUnsaved: {
    bn: "“{name}”-এর সেভ না করা পরিবর্তন এতে থাকবে না — আগে সেভ করুন।",
    en: "Unsaved changes in “{name}” won't be included — save it first.",
  },
  saveOverInput: {
    bn: "যে ফাইলগুলো জোড়া দিচ্ছেন, তার কোনোটির উপরে সেভ করা যাবে না। অন্য নাম দিন।",
    en: "You can't save over one of the files being merged. Choose another name.",
  },
  merged: { bn: "merged", en: "merged" },
  working: { bn: "কাজ চলছে…", en: "Working…" },

  // Images, stamps and signatures (Phase 4)
  picture: { bn: "ছবি", en: "Picture" },
  insertPictureHint: { bn: "PNG, JPG বা WebP ছবি যোগ করুন", en: "Insert a PNG, JPG or WebP picture" },
  pictureFiles: { bn: "ছবি", en: "Pictures" },
  signature: { bn: "স্বাক্ষর", en: "Signature" },
  stamp: { bn: "সিল", en: "Stamp" },
  editImages: { bn: "ছবি বদল", en: "Edit images" },
  editImagesHint: { bn: "পাতার ছবি মুছুন বা বদলান", en: "Delete or replace images on the pages" },
  editImagesBanner: {
    bn: "পাতার কোনো ছবিতে ক্লিক করে মুছুন বা বদলান। শেষ হলে আবার “ছবি বদল” চাপুন।",
    en: "Click an image on the page to delete or replace it. Press “Edit images” again when done.",
  },
  noImagesHere: { bn: "এই পাতায় কোনো ছবি নেই", en: "No images on this page" },
  imageWord: { bn: "ছবি", en: "Image" },
  replaceImage: { bn: "অন্য ছবি দিয়ে বদলান…", en: "Replace with another picture…" },
  deleteImage: { bn: "ছবি মুছুন", en: "Delete image" },
  imageNotEditable: {
    bn: "এই ছবিটি PDF-এর ভেতরের অন্য অংশের সাথে যুক্ত, তাই আলাদা করে বদলানো যাচ্ছে না।",
    en: "This image is part of another object in the PDF, so it can’t be changed on its own.",
  },
  unsupportedImage: { bn: "শুধু PNG, JPG ও WebP ছবি ব্যবহার করা যায়।", en: "Only PNG, JPG and WebP pictures can be used." },
  noSignatureFound: {
    bn: "ছবিতে কোনো স্বাক্ষর খুঁজে পাওয়া যায়নি। সাদা কাগজে গাঢ় কালিতে করা স্বাক্ষরের ছবি দিন।",
    en: "No signature was found in the picture. Use a photo of a dark-ink signature on light paper.",
  },
  placeHint: {
    bn: "টেনে সরান, কোণ টেনে ছোট-বড় করুন, উপরের গোল বোতাম টেনে ঘোরান। Enter চাপলে বসবে, Esc চাপলে বাতিল।",
    en: "Drag to move, drag the corner to resize, drag the round handle to turn. Enter places it, Esc cancels.",
  },
  rotateHandle: { bn: "টেনে ঘোরান (Shift: ১৫° করে)", en: "Drag to turn (Shift: 15° steps)" },
  resizeHandle: { bn: "টেনে ছোট-বড় করুন", en: "Drag to resize" },
  opacity: { bn: "স্বচ্ছতা", en: "Opacity" },
  place: { bn: "বসান", en: "Place" },
  placeOnPage: { bn: "পাতায় বসান", en: "Place on page" },
  sigDraw: { bn: "আঁকুন", en: "Draw" },
  sigType: { bn: "লিখুন", en: "Type" },
  sigPhoto: { bn: "ছবি থেকে", en: "From photo" },
  sigSaved: { bn: "আমার স্বাক্ষর", en: "My signatures" },
  inkColour: { bn: "কালির রং", en: "Ink colour" },
  inkBlack: { bn: "কালো", en: "Black" },
  inkBlue: { bn: "নীল", en: "Blue" },
  inkCustom: { bn: "অন্য রং", en: "Other colour" },
  drawHint: { bn: "মাউস, টাচপ্যাড বা পেন দিয়ে স্বাক্ষর করুন", en: "Sign with your mouse, touchpad or pen" },
  clear: { bn: "মুছে ফেলুন", en: "Clear" },
  typeYourName: { bn: "আপনার নাম লিখুন", en: "Type your name" },
  photoHint: {
    bn: "সাদা কাগজে করা স্বাক্ষরের ছবি বা স্ক্যান বেছে নিন। কাগজের অংশ স্বচ্ছ করে দেওয়া হবে।",
    en: "Choose a photo or scan of your signature on white paper. The paper is made see-through.",
  },
  choosePhoto: { bn: "ছবি বেছে নিন…", en: "Choose a photo…" },
  otherPhoto: { bn: "অন্য ছবি…", en: "Another photo…" },
  noSavedSignatures: {
    bn: "এখনো কোনো স্বাক্ষর সেভ করা নেই। আঁকা, লেখা বা ছবির স্বাক্ষর বসানোর সময় “আমার স্বাক্ষরে রাখুন” বেছে নিন।",
    en: "No saved signatures yet. Tick “Keep in My signatures” when you place a drawn, typed or photo signature.",
  },
  savedPrivacy: {
    bn: "সেভ করা স্বাক্ষর এই কম্পিউটারে Windows-এর নিজস্ব এনক্রিপশনে রাখা হয় — শুধু আপনার Windows অ্যাকাউন্ট থেকে খোলা যায়।",
    en: "Saved signatures stay on this PC, encrypted by Windows — only your Windows account can open them.",
  },
  useSignature: { bn: "এই স্বাক্ষর বসান", en: "Place this signature" },
  deleteSignature: { bn: "স্বাক্ষর মুছুন", en: "Delete signature" },
  deleteSignatureAsk: { bn: "“{name}” স্বাক্ষরটি স্থায়ীভাবে মুছে ফেলবেন?", en: "Delete the signature “{name}” for good?" },
  signatureSaved: { bn: "স্বাক্ষর “আমার স্বাক্ষর”-এ রাখা হয়েছে", en: "Signature kept in My signatures" },
  signatureEmpty: { bn: "আগে স্বাক্ষর করুন, নাম লিখুন বা ছবি বেছে নিন।", en: "Sign, type your name or choose a photo first." },
  addName: { bn: "নিচে নাম দিন", en: "Add name below" },
  yourName: { bn: "নাম", en: "Name" },
  addDate: { bn: "আজকের তারিখ দিন", en: "Add today’s date" },
  keepSignature: { bn: "“আমার স্বাক্ষর”-এ রাখুন", en: "Keep in My signatures" },
  customStamp: { bn: "নিজের সিল", en: "Your own stamp" },
  stampText: { bn: "সিলের লেখা (যেমন: প্রতিষ্ঠানের নাম)", en: "Stamp text (for example, your office name)" },
  stampSub: { bn: "দ্বিতীয় লাইন (ঐচ্ছিক)", en: "Second line (optional)" },
  sealRound: { bn: "গোল সিল", en: "Round seal" },
  sealBox: { bn: "চারকোনা", en: "Rectangle" },

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
