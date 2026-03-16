/**
 * TickState account deserializer — converts raw on-chain bytes to TickInfo.
 *
 * Byte layout (Borsh, sequential, no padding):
 *
 *   Offset  Field                    Type          Size
 *   ------  -----                    ----          ----
 *   0       Anchor discriminator     [u8; 8]       8
 *   8       bump                     u8            1
 *   9       pool                     Pubkey        32
 *   41      k.raw                    i128 (LE)     16
 *   57      status                   u8            1
 *   58      liquidity.raw            i128 (LE)     16
 *   74      sphere_radius.raw        i128 (LE)     16
 *   90      depeg_price.raw          i128 (LE)     16
 *   106     x_min.raw                i128 (LE)     16
 *   122     x_max.raw                i128 (LE)     16
 *   138     capital_efficiency.raw   i128 (LE)     16
 *   154     owner                    Pubkey        32
 *   186     created_at               i64 (LE)      8
 *   194     reserves[0..8].raw       [i128; 8]     128
 *   322     _reserved                [u8; 32]      32
 *   ------  Total: 354 bytes (= TickState::SIZE)
 */

import { readI128LE, q6464ToNumber } from "./format-utils";

/** Anchor discriminator for TickState: sha256("account:TickState")[..8] */
const TICK_DISCRIMINATOR = new Uint8Array([137, 76, 253, 128, 85, 226, 97, 148]);

/** Minimum account data size for a valid TickState */
const MIN_TICK_SIZE = 354;

/** Maximum assets supported (matches on-chain MAX_ASSETS) */
const MAX_ASSETS = 8;

export type TickStatus = "Interior" | "Boundary";

export interface TickInfo {
  /** Account public key */
  address: string;
  /** Pool this tick belongs to */
  pool: string;
  /** Plane constant k (Q64.64 raw) */
  kRaw: bigint;
  /** Plane constant k as display number */
  kDisplay: number;
  /** Interior or Boundary */
  status: TickStatus;
  /** Total liquidity in this tick (Q64.64 raw) */
  liquidityRaw: bigint;
  /** Liquidity as display number */
  liquidityDisplay: number;
  /** Boundary sphere radius (Q64.64 raw) */
  sphereRadiusRaw: bigint;
  /** Depeg price at max imbalance (display number) */
  depegPrice: number;
  /** Minimum reserve within tick (display number) */
  xMin: number;
  /** Maximum reserve within tick (display number) */
  xMax: number;
  /** Capital efficiency ratio (display number) */
  capitalEfficiency: number;
  /** Creator address */
  owner: string;
  /** Creation timestamp (unix seconds) */
  createdAt: number;
  /** Per-tick reserves as display numbers (first nAssets entries) */
  reserves: number[];
  /** Per-tick reserves as raw bigint (first nAssets entries) */
  reservesRaw: bigint[];
}

/**
 * Decode a base58 Pubkey from 32 bytes at the given offset.
 * Returns the raw hex string — callers can convert to base58 if needed.
 * For simplicity in this MVP, we encode as hex and use the address string
 * passed separately from getProgramAccounts.
 */
function readPubkey(data: Uint8Array, offset: number): string {
  const bytes = data.slice(offset, offset + 32);
  return Array.from(bytes)
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

/**
 * Deserialize raw account bytes into a TickInfo object.
 *
 * @param address - Account public key (from getProgramAccounts)
 * @param data - Raw account data bytes (Uint8Array)
 * @param nAssets - Number of active assets in the pool (for reserves slicing)
 * @returns TickInfo ready for UI display
 * @throws If discriminator mismatch or data too short
 */
export function deserializeTickState(
  address: string,
  data: Uint8Array,
  nAssets: number = 3,
): TickInfo {
  if (data.length < MIN_TICK_SIZE) {
    throw new Error(
      `TickState: expected >= ${MIN_TICK_SIZE} bytes, got ${data.length}`,
    );
  }

  // Verify Anchor discriminator
  for (let i = 0; i < 8; i++) {
    if (data[i] !== TICK_DISCRIMINATOR[i]) {
      throw new Error("TickState: invalid account discriminator");
    }
  }

  const view = new DataView(data.buffer, data.byteOffset, data.byteLength);

  // pool at offset 9 (32 bytes)
  const pool = readPubkey(data, 9);

  // k at offset 41 (i128 LE)
  const kRaw = readI128LE(view, 41);

  // status at offset 57 (u8: 0 = Interior, 1 = Boundary)
  const statusByte = data[57];
  const status: TickStatus = statusByte === 0 ? "Interior" : "Boundary";

  // liquidity at offset 58 (i128 LE)
  const liquidityRaw = readI128LE(view, 58);

  // sphere_radius at offset 74 (i128 LE)
  const sphereRadiusRaw = readI128LE(view, 74);

  // depeg_price at offset 90 (i128 LE)
  const depegPriceRaw = readI128LE(view, 90);

  // x_min at offset 106 (i128 LE)
  const xMinRaw = readI128LE(view, 106);

  // x_max at offset 122 (i128 LE)
  const xMaxRaw = readI128LE(view, 122);

  // capital_efficiency at offset 138 (i128 LE)
  const capitalEfficiencyRaw = readI128LE(view, 138);

  // owner at offset 154 (32 bytes) — we don't convert, use address from RPC
  const owner = readPubkey(data, 154);

  // created_at at offset 186 (i64 LE)
  const createdAt = Number(view.getBigInt64(186, true));

  // reserves at offset 194, each i128 = 16 bytes, array of MAX_ASSETS
  const reservesRaw: bigint[] = [];
  const reserves: number[] = [];
  for (let i = 0; i < Math.min(nAssets, MAX_ASSETS); i++) {
    const raw = readI128LE(view, 194 + i * 16);
    reservesRaw.push(raw);
    reserves.push(q6464ToNumber(raw));
  }

  return {
    address,
    pool,
    kRaw,
    kDisplay: q6464ToNumber(kRaw),
    status,
    liquidityRaw,
    liquidityDisplay: q6464ToNumber(liquidityRaw),
    sphereRadiusRaw,
    depegPrice: q6464ToNumber(depegPriceRaw),
    xMin: q6464ToNumber(xMinRaw),
    xMax: q6464ToNumber(xMaxRaw),
    capitalEfficiency: q6464ToNumber(capitalEfficiencyRaw),
    owner,
    createdAt,
    reserves,
    reservesRaw,
  };
}

/** Export discriminator for use in getProgramAccounts filters */
export { TICK_DISCRIMINATOR };
