#!/usr/bin/env node
// Fails if the Rust app pulls in any networking library (SRS NFR-3.1).
// Usage: node scripts/check-no-network.mjs   (run from the repository root)

import { execFileSync } from "node:child_process";

const FORBIDDEN = [
  "reqwest", "hyper", "ureq", "curl", "curl-sys", "isahc", "surf", "attohttpc",
  "h2", "h3", "quinn", "tungstenite", "tokio-tungstenite", "native-tls", "openssl",
  "rustls", "tauri-plugin-http", "tauri-plugin-updater", "tauri-plugin-websocket",
  "tauri-plugin-upload", "tauri-plugin-opener", "tauri-plugin-shell",
];

const out = execFileSync(
  "cargo",
  ["tree", "--edges", "normal", "--prefix", "none", "--format", "{p}"],
  { cwd: "src-tauri", encoding: "utf8", maxBuffer: 64 * 1024 * 1024 },
);

const crates = new Set(
  out.split(/\r?\n/).map((l) => l.trim().split(" ")[0]).filter(Boolean),
);
const found = FORBIDDEN.filter((name) => crates.has(name));

if (found.length > 0) {
  console.error(`Networking crates found in the app: ${found.join(", ")}`);
  console.error("Akkhor PDF Editor must contain no network code (SRS NFR-3.1).");
  process.exit(1);
}
console.log(`OK: ${crates.size} crates checked, no networking crates.`);
