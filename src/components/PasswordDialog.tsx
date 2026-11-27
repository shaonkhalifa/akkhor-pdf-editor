// Asks for the password of a protected PDF (FR-1.5). The password goes
// straight to the Rust core to unlock the file and is never stored.
import { useEffect, useRef, useState } from "react";
import { t } from "../i18n";
import { registerPasswordPrompt } from "../open";
import { useApp } from "../store";

interface Pending {
  name: string;
  wrong: boolean;
  resolve: (pw: string | null) => void;
}

export default function PasswordDialog() {
  const lang = useApp((s) => s.lang);
  const [pending, setPending] = useState<Pending | null>(null);
  const [value, setValue] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    registerPasswordPrompt(
      (name, wrong) =>
        new Promise((resolve) => {
          setValue("");
          setPending({ name, wrong, resolve });
        }),
    );
    return () => registerPasswordPrompt(null);
  }, []);

  useEffect(() => {
    if (pending) inputRef.current?.focus();
  }, [pending]);

  if (!pending) return null;
  const finish = (pw: string | null) => {
    pending.resolve(pw);
    setPending(null);
    setValue("");
  };

  return (
    <div className="overlay" role="dialog" aria-modal="true" aria-labelledby="pw-title">
      <form
        className="dialog"
        onSubmit={(e) => {
          e.preventDefault();
          finish(value);
        }}
      >
        <h2 id="pw-title">{t(lang, "passwordTitle")}</h2>
        <p className="muted">{pending.name}</p>
        {pending.wrong && <p className="error">{t(lang, "passwordWrong")}</p>}
        <label className="field">
          {t(lang, "passwordLabel")}
          <input ref={inputRef} type="password" value={value} onChange={(e) => setValue(e.target.value)} autoComplete="off" />
        </label>
        <div className="row end">
          <button type="button" className="ghost" onClick={() => finish(null)}>
            {t(lang, "cancel")}
          </button>
          <button type="submit" className="primary">
            {t(lang, "openBtn")}
          </button>
        </div>
      </form>
    </div>
  );
}
