// Phase 1 Bangla shaping self-test, kept as a panel for checking any install.
import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { t } from "../i18n";
import { useApp } from "../store";

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

async function renderToUrl(key: string, zoom = 1.25): Promise<string> {
  const bytes = await invoke<ArrayBuffer>("render_result", { key, zoom });
  return URL.createObjectURL(new Blob([bytes], { type: "image/png" }));
}

function Badge({ ok }: { ok: boolean }) {
  const lang = useApp((s) => s.lang);
  return <span className={ok ? "badge pass" : "badge fail"}>{ok ? t(lang, "pass") : t(lang, "fail")}</span>;
}

export default function SelfTest() {
  const lang = useApp((s) => s.lang);
  const tabs = useApp((s) => s.tabs);
  const setScreen = useApp((s) => s.setScreen);
  const [report, setReport] = useState<SelfTestReport | null>(null);
  const [images, setImages] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function run() {
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
    <div className="scroll-page">
      <div className="content">
        <section className="panel">
          <div className="card-head">
            <h2>{t(lang, "phase")}</h2>
            <button className="ghost" onClick={() => setScreen(tabs.length ? "doc" : "home")}>
              {t(lang, "back")}
            </button>
          </div>
          <p className="muted">{t(lang, "intro")}</p>
          <div className="row">
            <button className="primary" onClick={run} disabled={busy}>
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
                      <Badge ok={f.passed} />
                    </div>
                    {images[`font:${i}`] && (
                      <img className="page-img" src={images[`font:${i}`]} alt={`${f.font} test page`} />
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
                <Badge ok={report.spike.passed} />
              </div>
              <p className="muted">{t(lang, "spikeDesc")}</p>
              <div className="grid two">
                <figure>
                  <figcaption>{t(lang, "before")}</figcaption>
                  {images["spike:before"] && <img className="page-img" src={images["spike:before"]} alt="before edit" />}
                </figure>
                <figure>
                  <figcaption>{t(lang, "after")}</figcaption>
                  {images["spike:after"] && <img className="page-img" src={images["spike:after"]} alt="after edit" />}
                </figure>
              </div>
            </section>
          </>
        )}
      </div>
    </div>
  );
}
