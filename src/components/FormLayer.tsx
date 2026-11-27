// Filling in PDF forms (SRS Module 8): an input over every field. Text is
// committed when the field loses focus or Enter is pressed; ticks and choices
// at once. Each change goes into the PDF and can be undone.
import { useEffect, useState } from "react";
import { message } from "@tauri-apps/plugin-dialog";
import { formFields, setField, type FormField } from "../api";
import { t } from "../i18n";
import { useApp } from "../store";

function TextInput({ field, scale, onCommit }: { field: FormField; scale: number; onCommit: (v: string) => void }) {
  const [value, setValue] = useState(field.value);
  useEffect(() => setValue(field.value), [field.value]);
  const h = (field.rect[3] - field.rect[1]) * scale;
  const common = {
    value,
    disabled: field.readonly,
    title: field.name,
    "aria-label": field.name,
    style: { fontSize: Math.max(9, Math.min(h * 0.6, 16 * scale)) },
    onChange: (e: React.ChangeEvent<HTMLInputElement | HTMLTextAreaElement>) => setValue(e.target.value),
    onBlur: () => value !== field.value && onCommit(value),
  };
  return field.multiline ? (
    <textarea {...common} />
  ) : (
    <input
      type="text"
      {...common}
      onKeyDown={(e) => {
        if (e.key === "Enter") (e.target as HTMLInputElement).blur();
      }}
    />
  );
}

export default function FormLayer({ docId, page, version, scale }: { docId: number; page: number; version: number; scale: number }) {
  const lang = useApp((s) => s.lang);
  const setDoc = useApp((s) => s.setDoc);
  const [fields, setFields] = useState<FormField[]>([]);

  useEffect(() => {
    let cancelled = false;
    formFields(docId, page)
      .then((f) => !cancelled && setFields(f))
      .catch(() => !cancelled && setFields([]));
    return () => {
      cancelled = true;
    };
  }, [docId, page, version]);

  const commit = async (f: FormField, value: string) => {
    try {
      setDoc(await setField(docId, page, f.xref, value));
    } catch (e) {
      await message(String(e).includes("FIELD_READ_ONLY") ? t(lang, "fieldReadOnly") : String(e), { kind: "error", title: f.name });
    }
  };

  return (
    <div className="form-layer">
      {fields.map((f) => {
        const [x0, y0, x1, y1] = f.rect.map((v) => v * scale);
        const style = { left: x0, top: y0, width: x1 - x0, height: y1 - y0 };
        if (f.kind === "text") {
          return (
            <div key={f.xref} className="form-field text" style={style}>
              <TextInput field={f} scale={scale} onCommit={(v) => commit(f, v)} />
            </div>
          );
        }
        if (f.kind === "checkbox" || f.kind === "radio") {
          return (
            <button
              key={f.xref}
              className={`form-field tick ${f.kind}${f.checked ? " on" : ""}`}
              style={style}
              role={f.kind}
              aria-checked={f.checked}
              aria-label={f.name}
              title={f.name}
              disabled={f.readonly}
              onClick={() => commit(f, f.checked ? "false" : (f.on_value ?? "true"))}
            />
          );
        }
        if (f.kind === "combo" || f.kind === "list") {
          return (
            <div key={f.xref} className="form-field choice-field" style={style}>
              <select
                value={f.value}
                disabled={f.readonly}
                aria-label={f.name}
                title={f.name}
                style={{ fontSize: Math.max(9, Math.min((y1 - y0) * 0.6, 16 * scale)) }}
                onChange={(e) => commit(f, e.target.value)}
              >
                {!f.options.some(([v]) => v === f.value) && <option value={f.value}>{f.value}</option>}
                {f.options.map(([v, label]) => (
                  <option key={v} value={v}>
                    {label}
                  </option>
                ))}
              </select>
            </div>
          );
        }
        return null;
      })}
    </div>
  );
}
