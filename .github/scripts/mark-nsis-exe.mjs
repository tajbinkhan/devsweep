// Copies devsweep.exe and marks the copy as the one inside the NSIS installer.
//
// `tauri build` writes this marker into the exe just before it packs the installer, and the
// updater reads it to pick which kind of download to install. The release workflow has the exe
// code-signed before `tauri bundle` packs it again with --no-binary-patching, because changing
// a byte afterwards would break the signature. So the marker has to go in here, before signing,
// exactly as the Tauri bundler writes it: the first copy of the placeholder, same length.
//
// Usage: node mark-nsis-exe.mjs <devsweep.exe> <output path>

import { readFileSync, writeFileSync } from "node:fs";

const [input, output] = process.argv.slice(2);
if (!input || !output) {
  console.error("Usage: node mark-nsis-exe.mjs <devsweep.exe> <output path>");
  process.exit(2);
}

const placeholder = Buffer.from("__TAURI_BUNDLE_TYPE_VAR_UNK");
const nsis = Buffer.from("__TAURI_BUNDLE_TYPE_VAR_NSS");

const exe = readFileSync(input);
const at = exe.indexOf(placeholder);
if (at < 0) {
  console.error(`${input} has no bundle type placeholder. Was it already marked or signed?`);
  process.exit(1);
}
nsis.copy(exe, at);
writeFileSync(output, exe);
console.log(`Marked ${output} as the NSIS installer's exe`);
