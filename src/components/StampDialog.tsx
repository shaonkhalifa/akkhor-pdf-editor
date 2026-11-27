// Ready-made stamps and custom office seals (SRS FR-4.6).
import { useEffect, useRef, useState } from "react";
import { drawStamp, STAMPS, today, type StampSpec } from "../canvasArt";
import { t } from "../i18n";
import { startPlacing } from "../insertTools";
import { useApp } from "../store";

function Preview({ spec }: { spec: StampSpec }) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const c = drawStamp(spec);
    c.className = "stamp-canvas";
    el.replaceChildren(c);
  }, [spec.text, spec.sub, spec.color, spec.shape]);
  return <div ref={ref} className="stamp-preview" />;
}

export default function StampDialog({ docId, onClose }: { docId: number; onClose: () => void }) {
  const lang = useApp((s) => s.lang);
  const [withDate, setWithDate] = useState(false);
  const [custom, setCustom] = useState<StampSpec>({ text: "", sub: "", color: "#1a3fb0", shape: "round" });
  const sub = withDate ? today(lang) : undefined;

  const place = (spec: StampSpec) => {
    const c = drawStamp(spec);
    // Stamps start about 6 cm wide (a round seal about 4 cm).
    startPlacing(docId, c.toDataURL("image/png"), c.width, c.height, spec.shape === "round" ? 115 : 170);
    onClose();
  };

  return (
    <div className="overlay" role="dialog" aria-modal="true" aria-labelledby="stamp-title" onKeyDown={(e) => e.key === "Escape" && onClose()}>
      <div className="dialog stamp-dialog">
        <h2 id="stamp-title">{t(lang, "stamp")}</h2>
        <label className="choice">
          <input type="checkbox" checked={withDate} onChange={(e) => setWithDate(e.target.checked)} />
          {t(lang, "addDate")} <span className="muted small">({today(lang)})</span>
        </label>
        <div className="stamp-grid">
          {STAMPS.flatMap((s) =>
            (["bn", "en"] as const).map((l) => {
              const spec: StampSpec = { text: s[l], sub, color: s.color, shape: "box" };
              return (
                <button key={`${s.en}-${l}`} className="stamp-choice" onClick={() => place(spec)} title={s[l]}>
                  <Preview spec={spec} />
                </button>
              );
            }),
          )}
        </div>

        <h3>{t(lang, "customStamp")}</h3>
        <div className="custom-stamp">
          <div className="custom-fields">
            <input type="text" placeholder={t(lang, "stampText")} value={custom.text} onChange={(e) => setCustom({ ...custom, text: e.target.value })} />
            <input
              type="text"
              placeholder={t(lang, "stampSub")}
              value={custom.sub ?? ""}
              onChange={(e) => setCustom({ ...custom, sub: e.target.value })}
            />
            <div className="row">
              <label className="choice">
                <input type="radio" checked={custom.shape === "round"} onChange={() => setCustom({ ...custom, shape: "round" })} />
                {t(lang, "sealRound")}
              </label>
              <label className="choice">
                <input type="radio" checked={custom.shape === "box"} onChange={() => setCustom({ ...custom, shape: "box" })} />
                {t(lang, "sealBox")}
              </label>
              <label className="choice">
                {t(lang, "inkColour")}
                <input type="color" value={custom.color} onChange={(e) => setCustom({ ...custom, color: e.target.value })} />
              </label>
            </div>
          </div>
          {custom.text.trim() && <Preview spec={{ ...custom, text: custom.text.trim(), sub: custom.sub?.trim() || sub }} />}
        </div>

        <div className="row end">
          <button className="ghost" onClick={onClose}>
            {t(lang, "cancel")}
          </button>
          <button
            className="primary"
            disabled={!custom.text.trim()}
            onClick={() => place({ ...custom, text: custom.text.trim(), sub: custom.sub?.trim() || sub })}
          >
            {t(lang, "placeOnPage")}
          </button>
        </div>
      </div>
    </div>
  );
}
