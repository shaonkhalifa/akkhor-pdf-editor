// Simple line icons (drawn here, so no icon font or download is needed).
const paths = {
  undo: "M9 14 4 9l5-5M4 9h10a6 6 0 0 1 0 12h-3",
  redo: "M15 14l5-5-5-5M20 9H10a6 6 0 0 0 0 12h3",
  save: "M5 3h11l3 3v15H5zM8 3v5h7V3M8 21v-7h8v7",
  rotateLeft: "M3 4v5h5M3.5 9A8.5 8.5 0 1 1 6 15.5",
  rotateRight: "M21 4v5h-5M20.5 9A8.5 8.5 0 1 0 18 15.5",
  trash: "M4 7h16M9 7V4h6v3M6 7l1 14h10l1-14M10 11v6M14 11v6",
  blank: "M6 3h8l4 4v14H6zM14 3v4h4M12 11v6M9 14h6",
  insert: "M6 3h8l4 4v14H6zM14 3v4h4M12 10v7M9 14l3 3 3-3",
  extract: "M6 3h8l4 4v14H6zM14 3v4h4M12 17v-7M9 13l3-3 3 3",
  split: "M4 4h7v16H4zM13 4h7v16h-7M7 9h1M7 13h1M16 9h1M16 13h1",
  merge: "M3 5h7v14H3zM14 5h7v14h-7M10 12h4M12 10l2 2-2 2",
  picture: "M3 5h18v14H3zM3 16l5-5 4 4 3-3 6 6M15.5 9.5h.01",
  signature: "M3 17c3-1 4-9 6-9s0 9 2 9 2-4 4-4 1 3 3 3h3M3 21h18",
  stamp: "M9 3h6l-1 7h4l1 4H5l1-4h4zM5 17h14v3H5z",
  images: "M7 7h14v12H7zM3 3h14v2M3 3v12h2M7 16l4-4 3 3 2-2 5 5",
  editText: "M4 20h4L19 9l-4-4L4 16zM13 7l4 4M4 4h7",
  addText: "M5 5h10M10 5v14M8 19h4M17 13v6M14 16h6",
  highlight: "M4 20h16M7 16l2-6 8-6 3 3-6 8-6 2zM9 10l5 5",
  underline: "M7 4v6a5 5 0 0 0 10 0V4M5 20h14",
  strike: "M5 12h14M16 7c0-2-2-3-4-3s-4 1-4 3 2 3 4 3.5M8 16c0 2 2 4 4 4s4-1 4-3",
  redact: "M3 6h18v12H3zM6 9h12v6H6z",
} as const;

export type IconName = keyof typeof paths;

export default function Icon({ name, size = 18 }: { name: IconName; size?: number }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.8}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
    >
      <path d={paths[name]} />
    </svg>
  );
}
