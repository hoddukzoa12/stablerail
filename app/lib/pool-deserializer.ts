/**
 * Pool account deserializer — converts raw on-chain bytes to PoolState.
 *
 * Byte layout (Borsh, sequential, no padding):
 *
 *   Offset  Field                    Type          Size
 *   ------  -----                    ----          ----
 *   0       Anchor discriminator     [u8; 8]       8
 *   8       bump                     u8            1
 *   9       authority                Pubkey        32
 *   41      sphere.radius.raw        i128 (LE)     16
 *   57      sphere.n                 u8            1
 *   58      reserves[0..8].raw       [i128; 8]     128
 *   186     n_assets                 u8            1
 *   187     token_mints[0..8]        [Pubkey; 8]   256
 *   443     token_vaults[0..8]       [Pubkey; 8]   256
 *   699     vault_bumps[0..8]        [u8; 8]       8
 *   707     fee_rate_bps             u16 (LE)      2
 *   709     total_interior_liq.raw   i128 (LE)     16
 *   725     total_boundary_liq.raw   i128 (LE)     16
 *   741     alpha_cache.raw          i128 (LE)     16
 *   757     w_norm_sq_cache.raw      i128 (LE)     16
 *   773     tick_count               u16 (LE)      2
 *   775     is_active                bool          1
 *   776     total_volume.raw         i128 (LE)     16
 *   792     total_fees.raw           i128 (LE)     16
 *   808     created_at               i64 (LE)      8
 *   816     position_count           u64 (LE)      8
 *   824     token_decimals[0..8]     [u8; 8]       8
 *   832     _reserved                [u8; 104]     104
 *   ------  Total: 936 bytes
 */

import { Q6464 } from "./stablerail-math";
import type { PoolState } from "./stablerail-math";

/** Anchor discriminator for PoolState: sha256("account:PoolState")[..8] */
const POOL_DISCRIMINATOR = new Uint8Array([247, 237, 227, 245, 215, 195, 222, 70]);

/** Minimum account data size for a valid PoolState */
const MIN_POOL_SIZE = 936;

/**
 * Read a little-endian i128 from a DataView as a BigInt.
 * i128 = two u64 limbs: low 8 bytes + high 8 bytes (signed).
 */
function readI128LE(view: DataView, offset: number): bigint {
  const lo = view.getBigUint64(offset, true);
  const hi = view.getBigInt64(offset + 8, true); // signed for high limb
  return (hi << 64n) | lo;
}

/**
 * Deserialize raw account bytes into a PoolState for the swap calculator.
 *
 * @param data - Raw account data bytes (Uint8Array)
 * @returns PoolState ready for computeSwapQuote()
 * @throws If discriminator mismatch or data too short
 */
export function deserializePoolState(data: Uint8Array): PoolState {
  if (data.length < MIN_POOL_SIZE) {
    throw new Error(
      `PoolState: expected >= ${MIN_POOL_SIZE} bytes, got ${data.length}`,
    );
  }

  // Verify Anchor discriminator
  for (let i = 0; i < 8; i++) {
    if (data[i] !== POOL_DISCRIMINATOR[i]) {
      throw new Error("PoolState: invalid account discriminator");
    }
  }

  const view = new DataView(data.buffer, data.byteOffset, data.byteLength);

  // sphere.radius at offset 41
  const radiusRaw = readI128LE(view, 41);
  const radius = new Q6464(radiusRaw);

  // n_assets at offset 186
  const nAssets = data[186];

  // reserves at offset 58, each i128 = 16 bytes, array of 8
  const reserves: Q6464[] = [];
  for (let i = 0; i < nAssets; i++) {
    const raw = readI128LE(view, 58 + i * 16);
    reserves.push(new Q6464(raw));
  }

  // fee_rate_bps at offset 707 (u16 LE)
  const feeRateBps = view.getUint16(707, true);

  // token_decimals at offset 824, array of 8 u8
  const tokenDecimals: number[] = [];
  for (let i = 0; i < nAssets; i++) {
    tokenDecimals.push(data[824 + i]);
  }

  return {
    radius,
    reserves,
    nAssets,
    feeRateBps,
    tokenDecimals,
  };
}
