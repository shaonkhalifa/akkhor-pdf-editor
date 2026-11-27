// A question with several answers, such as Save / Don't save / Cancel.
// The Windows message box only offers two buttons, so this one is drawn by
// the app. Call `choose()` from anywhere; it resolves to the button index.
import { useEffect, useRef, useState } from "react";

export interface Choice {
  title: string;
  message: string;
  /** The first button is the main one; the last one is used for Escape. */
  buttons: string[];
}

interface Pending extends Choice {
  resolve: (index: number) => void;
}

let show: ((c: Pending) => void) | null = null;

export function choose(choice: Choice): Promise<number> {
  return new Promise((resolve) => {
    if (show) show({ ...choice, resolve });
    else resolve(choice.buttons.length - 1);
  });
}

export default function ChoiceDialog() {
  const [pending, setPending] = useState<Pending | null>(null);
  const firstRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    show = setPending;
    return () => {
      show = null;
    };
  }, []);

  useEffect(() => {
    if (pending) firstRef.current?.focus();
  }, [pending]);

  if (!pending) return null;
  const finish = (i: number) => {
    setPending(null);
    pending.resolve(i);
  };

  return (
    <div
      className="overlay"
      role="alertdialog"
      aria-modal="true"
      aria-labelledby="choice-title"
      aria-describedby="choice-text"
      onKeyDown={(e) => {
        if (e.key === "Escape") finish(pending.buttons.length - 1);
      }}
    >
      <div className="dialog">
        <h2 id="choice-title">{pending.title}</h2>
        <p id="choice-text" className="pre">
          {pending.message}
        </p>
        <div className="row end">
          {pending.buttons
            .map((label, i) => (
              <button key={i} ref={i === 0 ? firstRef : undefined} className={i === 0 ? "primary" : "ghost"} onClick={() => finish(i)}>
                {label}
              </button>
            ))
            .reverse()}
        </div>
      </div>
    </div>
  );
}
