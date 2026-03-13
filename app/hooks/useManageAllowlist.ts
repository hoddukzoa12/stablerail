"use client";

/**
 * Hook: send a manage_allowlist transaction (add or remove address).
 *
 * Instruction layout:
 *   discriminator(8) + action(1) + address(32) = 41 bytes
 *
 * Accounts: [authority(3), policy(0), allowlist(1), system_program(0)]
 */

import { useCallback } from "react";
import { type Address, getAddressEncoder } from "@solana/kit";
import { PROGRAM_ID, POLICY_PDA, ALLOWLIST_PDA } from "../lib/devnet-config";
import { SYSTEM_PROGRAM_ID } from "../lib/ata-utils";
import { useWriteTransaction, type WriteTransactionResult } from "./useWriteTransaction";

const DISCRIMINATOR = new Uint8Array([177, 83, 8, 223, 156, 5, 115, 40]);

export type AllowlistAction = "Add" | "Remove";

export interface ManageAllowlistParams {
  action: AllowlistAction;
  address: string;
}

function encodeInstruction(params: ManageAllowlistParams): Uint8Array {
  const encoder = getAddressEncoder();
  const addressBytes = encoder.encode(params.address as Address);

  const buf = new Uint8Array(41);
  buf.set(DISCRIMINATOR, 0);
  buf[8] = params.action === "Add" ? 0 : 1;
  buf.set(addressBytes, 9);
  return buf;
}

export function useManageAllowlist(): WriteTransactionResult<ManageAllowlistParams> {
  const buildInstruction = useCallback(
    (signerAddress: Address, params: ManageAllowlistParams) => ({
      programAddress: PROGRAM_ID as Address,
      accounts: [
        { address: signerAddress, role: 3 as const },
        { address: POLICY_PDA as Address, role: 0 as const },
        { address: ALLOWLIST_PDA as Address, role: 1 as const },
        { address: SYSTEM_PROGRAM_ID, role: 0 as const },
      ],
      data: encodeInstruction(params),
    }),
    [],
  );

  return useWriteTransaction(buildInstruction);
}
