import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { t, type Lang } from "./i18n";
import "./App.css";

interface FontCheck {
  font: string;
  passed: boolean;
  expected: string;
  extracted: string;
}

interface SpikeResult {
  passed: boolean;
  before_text: string;
  after_text: string;
}

interface SelfTestReport {
  fonts: FontCheck[];
  spike: SpikeResult;
  all_passed: boolean;
}

interface AppInfo {
  name: string;
  version: string;
  licence: string;
}

/** Ask the Rust core to render a stored PDF page; returns an object URL for an <img>. */
async function renderToUrl(key: string, zoom = 1.25): Promise<string> {
  const bytes = await invoke<ArrayBuffer>("render_result", { key, zoom });
  return URL.createObjectURL(new Blob([bytes], { type: "image/png" }));
}

function Badge({ ok, lang }: { ok: boolean; lang: Lang }) {
  return <span className={ok ? "badge pass" : "badge fail"}>{ok ? t(lang, "pass") : t(lang, "fail")}</span>;
}

export default function App() {
  const [lang, setLang] = useState<Lang>("bn");
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [report, setReport] = useState<SelfTestReport | null>(null);
  const [images, setImages] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<AppInfo>("app_info").then(setInfo).catch(() => {});
  }, []);

  useEffect(() => {
    document.documentElement.lang = lang;
  }, [lang]);

  async function runSelfTest() {
    setBusy(true);
    setError(null);
    Object.values(images).forEach(URL.revokeObjectURL);
    setImages({});
    try {
      const r = await invoke<SelfTestReport>("run_self_test");
      setReport(r);
      const keys = [...r.fonts.map((_, i) => `font:${i}`), "spike:before", "spike:after"];
      const urls: Record<string, string> = {};
      for (const k of keys) urls[k] = await renderToUrl(k);
      setImages(urls);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="app">
      <header className="topbar">
        <div className="brand">
          <span className="logo" aria-hidden="true">অ</span>
          <div>
            <h1>{t(lang, "appName")}</h1>
            <p className="muted">{t(lang, "tagline")}</p>
          </div>
        </div>
        <div className="top-actions">
          <span className="chip">● {t(lang, "offline")}</span>
          <button className="ghost" onClick={() => setLang(lang === "bn" ? "en" : "bn")}>
            {t(lang, "langSwitch")}
          </button>
        </div>
      </header>

      <main className="content">
        <section className="panel">
          <h2>{t(lang, "phase")}</h2>
          <p className="muted">{t(lang, "intro")}</p>
          <div className="row">
            <button className="primary" onClick={runSelfTest} disabled={busy}>
              {busy ? t(lang, "running") : t(lang, "run")}
            </button>
            {report && (
              <span className={report.all_passed ? "status ok" : "status bad"}>
                {report.all_passed ? t(lang, "allPassed") : t(lang, "someFailed")}
              </span>
            )}
          </div>
          {error && (
            <p className="error">
              {t(lang, "error")}: {error}
            </p>
          )}
        </section>

        {report && (
          <>
            <section className="panel">
              <h2>{t(lang, "fonts")}</h2>
              <div className="grid">
                {report.fonts.map((f, i) => (
                  <article className="card" key={f.font}>
                    <div className="card-head">
                      <h3>{f.font}</h3>
                      <Badge ok={f.passed} lang={lang} />
                    </div>
                    {images[`font:${i}`] && (
                      <img className="page" src={images[`font:${i}`]} alt={`${f.font} test page`} />
                    )}
                    <details>
                      <summary>{t(lang, "textBack")}</summary>
                      <pre>{f.extracted}</pre>
                    </details>
                  </article>
                ))}
              </div>
            </section>

            <section className="panel">
              <div className="card-head">
                <h2>{t(lang, "spike")}</h2>
                <Badge ok={report.spike.passed} lang={lang} />
              </div>
              <p className="muted">{t(lang, "spikeDesc")}</p>
              <div className="grid two">
                <figure>
                  <figcaption>{t(lang, "before")}</figcaption>
                  {images["spike:before"] && <img className="page" src={images["spike:before"]} alt="before edit" />}
                </figure>
                <figure>
                  <figcaption>{t(lang, "after")}</figcaption>
                  {images["spike:after"] && <img className="page" src={images["spike:after"]} alt="after edit" />}
                </figure>
              </div>
            </section>
          </>
        )}
      </main>

      <footer className="footer muted">
        {info ? `${info.name} v${info.version} · ${info.licence}` : ""}
      </footer>
    </div>
  );
}
