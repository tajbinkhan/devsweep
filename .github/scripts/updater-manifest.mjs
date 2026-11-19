// Writes latest.json, the file DevSweep's updater reads to learn about a new release.
//
//   node updater-manifest.mjs <dist folder> <version> <tag> <repo> <out file>
//
// Release notes and the publish date come from RELEASE_NOTES and RELEASE_DATE. The signature
// of each download is read from the .sig file `tauri signer sign` wrote next to it.
//
// Each download is listed under two names: the plain `{os}-{arch}` and `{os}-{arch}-{bundle}`.
// The updater looks for the second first. The .deb, .rpm and portable builds never install an
// update themselves, but they find the new version through the plain name and link to the
// download page.

import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const [dist, version, tag, repo, out] = process.argv.slice(2);
if (!out) {
  console.error("usage: updater-manifest.mjs <dist> <version> <tag> <repo> <out>");
  process.exit(2);
}

const files = readdirSync(dist);
function entry(pattern, what) {
  const name = files.find((f) => pattern.test(f));
  if (!name) throw new Error(`No ${what} in ${dist}`);
  const signature = readFileSync(join(dist, `${name}.sig`), "utf8").trim();
  const url = `https://github.com/${repo}/releases/download/${encodeURIComponent(tag)}/${encodeURIComponent(name)}`;
  return { signature, url };
}

const windows = entry(/-setup\.exe$/, "Windows installer");
const linux = entry(/\.AppImage$/, "AppImage");

const manifest = {
  version,
  notes: process.env.RELEASE_NOTES || "",
  pub_date: process.env.RELEASE_DATE || new Date().toISOString(),
  platforms: {
    "windows-x86_64": windows,
    "windows-x86_64-nsis": windows,
    "linux-x86_64": linux,
    "linux-x86_64-appimage": linux,
  },
};
writeFileSync(out, JSON.stringify(manifest, null, 2) + "\n");
console.log(`Wrote ${out} for ${version}`);
