/**
 * Patch @solana/codecs-strings browser ESM bundle to inline the base58
 * alphabet constant. Turbopack code-splits the module and loses the
 * reference to `alphabet2`, causing "ReferenceError: alphabet4 is not
 * defined" at runtime.
 *
 * This replaces `alphabet2` references in getBase58Encoder/Decoder/Codec
 * with the literal string, so the constant lives in the same scope as
 * the functions that use it.
 */
import { readFileSync, writeFileSync } from "fs";
import { resolve } from "path";

const BASE58_ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

const files = [
  "node_modules/@solana/codecs-strings/dist/index.browser.mjs",
  "node_modules/@solana/codecs-strings/dist/index.node.mjs",
  "node_modules/@solana/codecs-strings/dist/index.native.mjs",
];

let patched = 0;
for (const rel of files) {
  const filePath = resolve(rel);
  let src;
  try {
    src = readFileSync(filePath, "utf8");
  } catch {
    continue; // file may not exist in all environments
  }

  // Replace: var getBase58Encoder = () => getBaseXEncoder(alphabet2);
  // With:   var getBase58Encoder = () => getBaseXEncoder("123456789ABCDEF...");
  const replaced = src
    .replace(
      /var getBase58Encoder = \(\) => getBaseXEncoder\(alphabet2\)/g,
      `var getBase58Encoder = () => getBaseXEncoder("${BASE58_ALPHABET}")`
    )
    .replace(
      /var getBase58Decoder = \(\) => getBaseXDecoder\(alphabet2\)/g,
      `var getBase58Decoder = () => getBaseXDecoder("${BASE58_ALPHABET}")`
    )
    .replace(
      /var getBase58Codec = \(\) => getBaseXCodec\(alphabet2\)/g,
      `var getBase58Codec = () => getBaseXCodec("${BASE58_ALPHABET}")`
    );

  if (replaced !== src) {
    writeFileSync(filePath, replaced, "utf8");
    patched++;
    console.log(`✓ Patched ${rel}`);
  } else {
    console.log(`· ${rel} (no changes needed)`);
  }
}

console.log(`Done. ${patched} file(s) patched.`);
