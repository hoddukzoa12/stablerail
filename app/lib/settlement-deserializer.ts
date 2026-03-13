/**
 * Settlement account deserializers — converts raw on-chain bytes
 * to PolicyState, AllowlistState, SettlementState, AuditEntryState.
 */

import { getAddressDecoder } from "@solana/kit";
import { readI128LE, q6464ToNumber } from "./format-utils";

// ── Discriminators (sha256("account:<Name>")[..8]) ──

const POLICY_DISCRIMINATOR = new Uint8Array([227, 72, 222, 251, 231, 230, 163, 49]);
const ALLOWLIST_DISCRIMINATOR = new Uint8Array([163, 211, 213, 53, 199, 253, 135, 130]);
const SETTLEMENT_DISCRIMINATOR = new Uint8Array([32, 107, 224, 72, 68, 162, 247, 192]);
const AUDIT_DISCRIMINATOR = new Uint8Array([107, 54, 238, 159, 149, 195, 151, 196]);

// ── Account sizes (from Rust SIZE constants) ──

/** PolicyState: 8 + 1 + 32 + 32 + 16 + 16 + 16 + 8 + 1 + 8 + 8 + 64 = 210 */
const POLICY_SIZE = 210;

/** AllowlistState: 8 + 1 + 32 + 32 + 2 + (32 * 20) + 64 = 779 */
const ALLOWLIST_SIZE = 779;

/** SettlementState: 8 + 1 + 32 + 32 + 32 + 1 + 1 + 16 + 16 + 16 + 1 + 8 + 8 + 64 = 236 */
const SETTLEMENT_SIZE = 236;

/** AuditEntryState: 8 + 1 + 32 + 32 + 32 + 32 + 32 + 16 + 8 + 8 + 64 = 265 */
const AUDIT_SIZE = 265;

const MAX_ALLOWLIST_SIZE = 20;

// ── Shared helpers ──

const addressDecoder = getAddressDecoder();

/** Read 32 bytes as a base58 Solana address string. */
function readPubkey(data: Uint8Array, offset: number): string {
  return addressDecoder.decode(data.slice(offset, offset + 32));
}

/** Verify Anchor discriminator matches expected. */
function verifyDiscriminator(data: Uint8Array, expected: Uint8Array, name: string): void {
  for (let i = 0; i < 8; i++) {
    if (data[i] !== expected[i]) {
      throw new Error(`${name}: invalid account discriminator`);
    }
  }
}

// ── Types ──

export interface PolicyStateData {
  bump: number;
  authority: string;
  pool: string;
  maxTradeAmountRaw: bigint;
  maxTradeAmount: number;
  maxDailyVolumeRaw: bigint;
  maxDailyVolume: number;
  currentDailyVolumeRaw: bigint;
  currentDailyVolume: number;
  lastResetTimestamp: number;
  isActive: boolean;
  createdAt: number;
  updatedAt: number;
}

export interface AllowlistStateData {
  bump: number;
  policy: string;
  authority: string;
  count: number;
  addresses: string[];
}

export type SettlementStatusType = "Pending" | "Executed" | "Failed";

export interface SettlementRecord {
  address: string;
  bump: number;
  pool: string;
  policy: string;
  executor: string;
  tokenInIndex: number;
  tokenOutIndex: number;
  amountInRaw: bigint;
  amountIn: number;
  amountOutRaw: bigint;
  amountOut: number;
  executionPriceRaw: bigint;
  executionPrice: number;
  status: SettlementStatusType;
  executedAt: number;
  nonce: bigint;
}

export interface AuditRecord {
  address: string;
  bump: number;
  settlement: string;
  executor: string;
  pool: string;
  policy: string;
  actionHash: string;
  amountRaw: bigint;
  amount: number;
  timestamp: number;
  sequenceNumber: bigint;
}

// ── Deserializers ──

/**
 * Deserialize PolicyState from raw account bytes.
 *
 * Byte layout:
 *   0   discriminator     [u8; 8]   8
 *   8   bump              u8        1
 *   9   authority         Pubkey    32
 *   41  pool              Pubkey    32
 *   73  max_trade_amount  i128 LE   16  (Q64.64)
 *   89  max_daily_volume  i128 LE   16  (Q64.64)
 *   105 current_daily_vol i128 LE   16  (Q64.64)
 *   121 last_reset_ts     i64 LE    8
 *   129 is_active         bool      1
 *   130 created_at        i64 LE    8
 *   138 updated_at        i64 LE    8
 *   146 _reserved         [u8; 64]  64
 *   Total: 210 bytes
 */
export function deserializePolicyState(data: Uint8Array): PolicyStateData {
  if (data.length < POLICY_SIZE) {
    throw new Error(`PolicyState: expected >= ${POLICY_SIZE} bytes, got ${data.length}`);
  }
  verifyDiscriminator(data, POLICY_DISCRIMINATOR, "PolicyState");

  const view = new DataView(data.buffer, data.byteOffset, data.byteLength);

  const maxTradeAmountRaw = readI128LE(view, 73);
  const maxDailyVolumeRaw = readI128LE(view, 89);
  const currentDailyVolumeRaw = readI128LE(view, 105);

  return {
    bump: data[8],
    authority: readPubkey(data, 9),
    pool: readPubkey(data, 41),
    maxTradeAmountRaw,
    maxTradeAmount: q6464ToNumber(maxTradeAmountRaw),
    maxDailyVolumeRaw,
    maxDailyVolume: q6464ToNumber(maxDailyVolumeRaw),
    currentDailyVolumeRaw,
    currentDailyVolume: q6464ToNumber(currentDailyVolumeRaw),
    lastResetTimestamp: Number(view.getBigInt64(121, true)),
    isActive: data[129] !== 0,
    createdAt: Number(view.getBigInt64(130, true)),
    updatedAt: Number(view.getBigInt64(138, true)),
  };
}

/**
 * Deserialize AllowlistState from raw account bytes.
 *
 * Byte layout:
 *   0   discriminator  [u8; 8]         8
 *   8   bump           u8              1
 *   9   policy         Pubkey          32
 *   41  authority      Pubkey          32
 *   73  count          u16 LE          2
 *   75  addresses      [Pubkey; 20]    640
 *   715 _reserved      [u8; 64]        64
 *   Total: 779 bytes
 */
export function deserializeAllowlistState(data: Uint8Array): AllowlistStateData {
  if (data.length < ALLOWLIST_SIZE) {
    throw new Error(`AllowlistState: expected >= ${ALLOWLIST_SIZE} bytes, got ${data.length}`);
  }
  verifyDiscriminator(data, ALLOWLIST_DISCRIMINATOR, "AllowlistState");

  const view = new DataView(data.buffer, data.byteOffset, data.byteLength);
  const count = view.getUint16(73, true);

  const addresses: string[] = [];
  for (let i = 0; i < Math.min(count, MAX_ALLOWLIST_SIZE); i++) {
    addresses.push(readPubkey(data, 75 + i * 32));
  }

  return {
    bump: data[8],
    policy: readPubkey(data, 9),
    authority: readPubkey(data, 41),
    count,
    addresses,
  };
}

/**
 * Deserialize SettlementState from raw account bytes.
 *
 * Byte layout:
 *   0   discriminator    [u8; 8]    8
 *   8   bump             u8         1
 *   9   pool             Pubkey     32
 *   41  policy           Pubkey     32
 *   73  executor         Pubkey     32
 *   105 token_in_index   u8         1
 *   106 token_out_index  u8         1
 *   107 amount_in        i128 LE    16  (Q64.64)
 *   123 amount_out       i128 LE    16  (Q64.64)
 *   139 execution_price  i128 LE    16  (Q64.64)
 *   155 status           u8 enum    1   (0=Pending, 1=Executed, 2=Failed)
 *   156 executed_at      i64 LE     8
 *   164 nonce            u64 LE     8
 *   172 _reserved        [u8; 64]   64
 *   Total: 236 bytes
 */
export function deserializeSettlementState(
  address: string,
  data: Uint8Array,
): SettlementRecord {
  if (data.length < SETTLEMENT_SIZE) {
    throw new Error(`SettlementState: expected >= ${SETTLEMENT_SIZE} bytes, got ${data.length}`);
  }
  verifyDiscriminator(data, SETTLEMENT_DISCRIMINATOR, "SettlementState");

  const view = new DataView(data.buffer, data.byteOffset, data.byteLength);

  const amountInRaw = readI128LE(view, 107);
  const amountOutRaw = readI128LE(view, 123);
  const executionPriceRaw = readI128LE(view, 139);

  const statusByte = data[155];
  const statusMap: Record<number, SettlementStatusType> = {
    0: "Pending",
    1: "Executed",
    2: "Failed",
  };

  return {
    address,
    bump: data[8],
    pool: readPubkey(data, 9),
    policy: readPubkey(data, 41),
    executor: readPubkey(data, 73),
    tokenInIndex: data[105],
    tokenOutIndex: data[106],
    amountInRaw,
    amountIn: q6464ToNumber(amountInRaw),
    amountOutRaw,
    amountOut: q6464ToNumber(amountOutRaw),
    executionPriceRaw,
    executionPrice: q6464ToNumber(executionPriceRaw),
    status: statusMap[statusByte] ?? "Pending",
    executedAt: Number(view.getBigInt64(156, true)),
    nonce: view.getBigUint64(164, true),
  };
}

/**
 * Deserialize AuditEntryState from raw account bytes.
 *
 * Byte layout:
 *   0   discriminator    [u8; 8]    8
 *   8   bump             u8         1
 *   9   settlement       Pubkey     32
 *   41  executor         Pubkey     32
 *   73  pool             Pubkey     32
 *   105 policy           Pubkey     32
 *   137 action_hash      [u8; 32]   32
 *   169 amount           i128 LE    16  (Q64.64)
 *   185 timestamp        i64 LE     8
 *   193 sequence_number  u64 LE     8
 *   201 _reserved        [u8; 64]   64
 *   Total: 265 bytes
 */
export function deserializeAuditEntryState(
  address: string,
  data: Uint8Array,
): AuditRecord {
  if (data.length < AUDIT_SIZE) {
    throw new Error(`AuditEntryState: expected >= ${AUDIT_SIZE} bytes, got ${data.length}`);
  }
  verifyDiscriminator(data, AUDIT_DISCRIMINATOR, "AuditEntryState");

  const view = new DataView(data.buffer, data.byteOffset, data.byteLength);
  const amountRaw = readI128LE(view, 169);

  // Convert action_hash to hex string for display
  const hashBytes = data.slice(137, 169);
  const actionHash = Array.from(hashBytes)
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");

  return {
    address,
    bump: data[8],
    settlement: readPubkey(data, 9),
    executor: readPubkey(data, 41),
    pool: readPubkey(data, 73),
    policy: readPubkey(data, 105),
    actionHash,
    amountRaw,
    amount: q6464ToNumber(amountRaw),
    timestamp: Number(view.getBigInt64(185, true)),
    sequenceNumber: view.getBigUint64(193, true),
  };
}

// Re-export discriminators for use in getProgramAccounts filters
export {
  POLICY_DISCRIMINATOR,
  ALLOWLIST_DISCRIMINATOR,
  SETTLEMENT_DISCRIMINATOR,
  AUDIT_DISCRIMINATOR,
};
