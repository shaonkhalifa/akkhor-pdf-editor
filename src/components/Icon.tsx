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
