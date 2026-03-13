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

function encodeInstruction(params: UpdatePolicyParams): Uint8Array {
  return concatBytes(
    DISCRIMINATOR,
    encodeBorshOptionU64(params.maxTradeAmount),
    encodeBorshOptionU64(params.maxDailyVolume),
    encodeBorshOptionBool(params.isActive),
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
