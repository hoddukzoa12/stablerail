"use client";

/**
 * Hook: send an execute_settlement transaction.
 *
 * Instruction layout:
 *   discriminator(8) + token_in_index(1) + token_out_index(1)
 *   + amount(8) + min_amount_out(8) + nonce(8)
 *   + Option<TravelRuleData>(1 or 1+168) = 35..203 bytes
 *
 * Named accounts:
 *   [executor(3), pool(1), policy(1), allowlist(0),
 *    settlement(1), audit_entry(1), token_program(0), system_program(0)]
 *
 * remaining_accounts:
 *   [vault_in(1), vault_out(1), executor_ata_in(1), executor_ata_out(1),
 *    kyc_entry?(0) — appended when kycRequired is true]
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

/** Travel Rule payload for settlements above the configured threshold. */
export interface TravelRuleInput {
  originatorName: string;   // max 64 chars
  beneficiaryName: string;  // max 64 chars
  originatorVasp: string;   // max 32 chars (LEI or DID)
  purpose: string;          // max 8 chars (e.g., "TRADE", "SETTL")
}

export interface SettlementExecuteParams {
  tokenInIndex: number;
  tokenOutIndex: number;
  amount: bigint;
  minAmountOut: bigint;
  vaultIn: string;
  vaultOut: string;
  mintIn: string;
  mintOut: string;
  /** When true, derives and appends KYC entry PDA to remaining_accounts */
  kycRequired?: boolean;
  /** Travel Rule data, required when policy enforces Travel Rule above threshold */
  travelRuleData?: TravelRuleInput;
}

/** Encode a string into a fixed-size byte array, truncated by byte length, padded with zeros. */
function stringToFixedBytes(s: string, size: number): Uint8Array {
  const buf = new Uint8Array(size);
  const encoded = new TextEncoder().encode(s);
  buf.set(encoded.length <= size ? encoded : encoded.slice(0, size));
  return buf;
}

function encodeInstruction(
  params: SettlementExecuteParams,
  nonce: bigint,
): Uint8Array {
  // Base: disc(8) + token_in(1) + token_out(1) + amount(8) + min_out(8) + nonce(8) = 34
  // + Option<TravelRuleData>: None = 1 byte, Some = 1 + 64+64+32+8 = 169 bytes
  const hasTravelRule = !!params.travelRuleData;
  const totalSize = 34 + (hasTravelRule ? 169 : 1);

  const buf = new ArrayBuffer(totalSize);
  const bytes = new Uint8Array(buf);
  const view = new DataView(buf);

  bytes.set(DISCRIMINATOR, 0);
  view.setUint8(8, params.tokenInIndex);
  view.setUint8(9, params.tokenOutIndex);
  view.setBigUint64(10, params.amount, true);
  view.setBigUint64(18, params.minAmountOut, true);
  view.setBigUint64(26, nonce, true);

  if (hasTravelRule) {
    const tr = params.travelRuleData!;
    bytes[34] = 1; // Option::Some
    bytes.set(stringToFixedBytes(tr.originatorName, 64), 35);
    bytes.set(stringToFixedBytes(tr.beneficiaryName, 64), 99);
    bytes.set(stringToFixedBytes(tr.originatorVasp, 32), 163);
    bytes.set(stringToFixedBytes(tr.purpose, 8), 195);
  } else {
    bytes[34] = 0; // Option::None
  }

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

      // Base remaining_accounts: vaults + ATAs
      const accounts: Array<{ address: Address; role: 0 | 1 | 2 | 3 }> = [
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
      ];

      // When KYC is required, append the executor's KYC entry PDA
      // as remaining_accounts[4] (readonly). On-chain execute_settlement
      // reads this to verify KYC status, expiry, risk score, and AML.
      if (params.kycRequired) {
        const encoder = getAddressEncoder();
        const [kycEntryPda] = await getProgramDerivedAddress({
          programAddress: PROGRAM_ID as Address,
          seeds: [
            new TextEncoder().encode("kyc_entry"),
            encoder.encode(POLICY_PDA as Address),
            encoder.encode(executorAddress),
          ],
        });
        accounts.push({ address: kycEntryPda, role: 0 as const });
      }

      return {
        programAddress: PROGRAM_ID as Address,
        accounts,
        data: encodeInstruction(params, nonce),
      };
    },
    [],
  );

  return useWriteTransaction(buildInstruction);
}
