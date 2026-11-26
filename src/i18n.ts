// Interface text in Bangla and English (SRS FR-13.1).

export type Lang = "bn" | "en";

const strings = {
  appName: { bn: "অক্ষর PDF Editor", en: "Akkhor PDF Editor" },
  tagline: {
    bn: "বিনামূল্যে, অফলাইনে বাংলা ও ইংরেজি PDF এডিটর",
    en: "Free, offline Bangla & English PDF editor",
  },
  phase: { bn: "ধাপ ১: বাংলা শেপিং পরীক্ষা", en: "Phase 1: Bangla shaping self-test" },
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
  offline: { bn: "ইন্টারনেট ব্যবহার করে না", en: "No internet used" },
  error: { bn: "ত্রুটি", en: "Error" },
  langSwitch: { bn: "English", en: "বাংলা" },
} as const;

export type Key = keyof typeof strings;

export function t(lang: Lang, key: Key): string {
  return strings[key][lang];
}
