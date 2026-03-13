"use client";

/**
 * Hook: send an execute_settlement transaction.
 *
 * Instruction layout:
 *   discriminator(8) + token_in_index(1) + token_out_index(1)
 *   + amount(8) + min_amount_out(8) + nonce(8) = 34 bytes
 *
 * Named accounts:
 *   [executor(3), pool(1), policy(1), allowlist(0),
 *    settlement(1), audit_entry(1), token_program(0), system_program(0)]
 *
 * remaining_accounts:
 *   [vault_in(1), vault_out(1), executor_ata_in(1), executor_ata_out(1)]
 */

import { useCallback } from "react";
import {
  type Address,
  getProgramDerivedAddress,
  getAddressEncoder,
} from "@solana/kit";
import { PROGRAM_ID, POOL_PDA, POLICY_PDA, ALLOWLIST_PDA } from "../lib/devnet-config";
import { TOKEN_PROGRAM_ID, SYSTEM_PROGRAM_ID, deriveAta } from "../lib/ata-utils";
import { useWriteTransaction, type WriteTransactionResult } from "./useWriteTransaction";

const DISCRIMINATOR = new Uint8Array([237, 120, 82, 62, 224, 193, 147, 137]);

export interface SettlementExecuteParams {
  tokenInIndex: number;
  tokenOutIndex: number;
  amount: bigint;
  minAmountOut: bigint;
  vaultIn: string;
  vaultOut: string;
  mintIn: string;
  mintOut: string;
}

function encodeInstruction(
  params: SettlementExecuteParams,
  nonce: bigint,
): Uint8Array {
  const buf = new ArrayBuffer(34);
  const bytes = new Uint8Array(buf);
  const view = new DataView(buf);

  bytes.set(DISCRIMINATOR, 0);
  view.setUint8(8, params.tokenInIndex);
  view.setUint8(9, params.tokenOutIndex);
  view.setBigUint64(10, params.amount, true);
  view.setBigUint64(18, params.minAmountOut, true);
  view.setBigUint64(26, nonce, true);

  return bytes;
}

function encodeU64LE(value: bigint): Uint8Array {
  const bytes = new Uint8Array(8);
  new DataView(bytes.buffer).setBigUint64(0, value, true);
  return bytes;
}

async function deriveSettlementPda(
  pool: Address,
  executor: Address,
  nonce: bigint,
): Promise<Address> {
  const encoder = getAddressEncoder();
  const [pda] = await getProgramDerivedAddress({
    programAddress: PROGRAM_ID as Address,
    seeds: [
      new TextEncoder().encode("settlement"),
      encoder.encode(pool),
      encoder.encode(executor),
      encodeU64LE(nonce),
    ],
  });
  return pda;
}

async function deriveAuditPda(settlement: Address): Promise<Address> {
  const encoder = getAddressEncoder();
  const [pda] = await getProgramDerivedAddress({
    programAddress: PROGRAM_ID as Address,
    seeds: [
      new TextEncoder().encode("audit"),
      encoder.encode(settlement),
    ],
  });
  return pda;
}

export function useExecuteSettlement(): WriteTransactionResult<SettlementExecuteParams> {
  const buildInstruction = useCallback(
    async (executorAddress: Address, params: SettlementExecuteParams) => {
      const nonce = BigInt(Date.now());

      const [settlementPda, executorAtaIn, executorAtaOut] = await Promise.all([
        deriveSettlementPda(POOL_PDA as Address, executorAddress, nonce),
        deriveAta(executorAddress, params.mintIn as Address),
        deriveAta(executorAddress, params.mintOut as Address),
      ]);

      const auditPda = await deriveAuditPda(settlementPda);

      return {
        programAddress: PROGRAM_ID as Address,
        accounts: [
          { address: executorAddress, role: 3 as const },
          { address: POOL_PDA as Address, role: 1 as const },
          { address: POLICY_PDA as Address, role: 1 as const },
          { address: ALLOWLIST_PDA as Address, role: 0 as const },
          { address: settlementPda, role: 1 as const },
          { address: auditPda, role: 1 as const },
          { address: TOKEN_PROGRAM_ID, role: 0 as const },
          { address: SYSTEM_PROGRAM_ID, role: 0 as const },
          { address: params.vaultIn as Address, role: 1 as const },
          { address: params.vaultOut as Address, role: 1 as const },
          { address: executorAtaIn, role: 1 as const },
          { address: executorAtaOut, role: 1 as const },
        ],
        data: encodeInstruction(params, nonce),
      };
    },
    [],
  );

  return useWriteTransaction(buildInstruction);
}
