/**
 * Patch @solana/codecs-strings to inline the base58 alphabet constant.
 * Turbopack code-splits alphabet2 into a separate chunk, causing
 * "ReferenceError: alphabet4 is not defined" at runtime.
 */
import { readFileSync, writeFileSync } from "fs";
import { resolve } from "path";

const BASE58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

const files = [
  "node_modules/@solana/codecs-strings/dist/index.browser.mjs",
  "node_modules/@solana/codecs-strings/dist/index.node.mjs",
  "node_modules/@solana/codecs-strings/dist/index.native.mjs",
  "node_modules/@solana/codecs-strings/dist/index.node.cjs",
];

let patched = 0;
for (const rel of files) {
  let src;
  try { src = readFileSync(resolve(rel), "utf8"); } catch { continue; }

  const out = src
    .replace(/getBaseXEncoder\(alphabet2\)/g, `getBaseXEncoder("${BASE58}")`)
    .replace(/getBaseXDecoder\(alphabet2\)/g, `getBaseXDecoder("${BASE58}")`)
    .replace(/getBaseXCodec\(alphabet2\)/g, `getBaseXCodec("${BASE58}")`);

  if (out !== src) { writeFileSync(resolve(rel), out, "utf8"); patched++; console.log(`patched ${rel}`); }
}
console.log(`${patched} file(s) patched`);
