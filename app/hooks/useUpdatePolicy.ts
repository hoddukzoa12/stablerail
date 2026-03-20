"use client";

/**
 * Hook: send an update_policy transaction.
 *
 * Instruction layout (Borsh):
 *   discriminator(8) + Option<u64> + Option<u64> + Option<bool>
 *
 * Accounts: [authority(3), policy(1), pool(0)]
 */

import { useCallback } from "react";
import { type Address } from "@solana/kit";
import { PROGRAM_ID, POOL_PDA, POLICY_PDA } from "../lib/devnet-config";
import { concatBytes } from "../lib/format-utils";
import { useWriteTransaction, type WriteTransactionResult } from "./useWriteTransaction";

const DISCRIMINATOR = new Uint8Array([212, 245, 246, 7, 163, 151, 18, 57]);

export interface UpdatePolicyParams {
  maxTradeAmount?: bigint;
  maxDailyVolume?: bigint;
  isActive?: boolean;
  // KYC/KYT/AML compliance fields
  kycRequired?: boolean;
  maxRiskScore?: number; // u8 (0-100)
  requireTravelRule?: boolean;
  travelRuleThreshold?: bigint;
  allowedJurisdictions?: Array<[number, number]>; // ISO 3166-1 alpha-2 as byte pairs
}

function encodeBorshOptionU64(value: bigint | undefined): Uint8Array {
  if (value === undefined) return new Uint8Array([0]);
  const buf = new ArrayBuffer(9);
  const bytes = new Uint8Array(buf);
  const view = new DataView(buf);
  bytes[0] = 1;
  view.setBigUint64(1, value, true);
  return bytes;
}

function encodeBorshOptionBool(value: boolean | undefined): Uint8Array {
  if (value === undefined) return new Uint8Array([0]);
  return new Uint8Array([1, value ? 1 : 0]);
}

function encodeBorshOptionU8(value: number | undefined): Uint8Array {
  if (value === undefined) return new Uint8Array([0]);
  return new Uint8Array([1, value & 0xff]);
}

function encodeBorshOptionVecBytes2(
  value: Array<[number, number]> | undefined,
): Uint8Array {
  if (value === undefined) return new Uint8Array([0]);
  // Borsh Option::Some(Vec<[u8;2]>) = 1 + u32_le(len) + len*2
  const buf = new Uint8Array(1 + 4 + value.length * 2);
  buf[0] = 1;
  new DataView(buf.buffer).setUint32(1, value.length, true);
  for (let i = 0; i < value.length; i++) {
    buf[5 + i * 2] = value[i][0];
    buf[5 + i * 2 + 1] = value[i][1];
  }
  return buf;
}

function encodeInstruction(params: UpdatePolicyParams): Uint8Array {
  return concatBytes(
    DISCRIMINATOR,
    encodeBorshOptionU64(params.maxTradeAmount),
    encodeBorshOptionU64(params.maxDailyVolume),
    encodeBorshOptionBool(params.isActive),
    encodeBorshOptionBool(params.kycRequired),
    encodeBorshOptionU8(params.maxRiskScore),
    encodeBorshOptionBool(params.requireTravelRule),
    encodeBorshOptionU64(params.travelRuleThreshold),
    encodeBorshOptionVecBytes2(params.allowedJurisdictions),
  );
}

export function useUpdatePolicy(): WriteTransactionResult<UpdatePolicyParams> {
  const buildInstruction = useCallback(
    (signerAddress: Address, params: UpdatePolicyParams) => ({
      programAddress: PROGRAM_ID as Address,
      accounts: [
        { address: signerAddress, role: 3 as const },
        { address: POLICY_PDA as Address, role: 1 as const },
        { address: POOL_PDA as Address, role: 0 as const },
      ],
      data: encodeInstruction(params),
    }),
    [],
  );

  return useWriteTransaction(buildInstruction);
}
