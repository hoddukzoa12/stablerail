"use client";

/**
 * Hook: send a manage_kyc_entry transaction to create/update KYC entries.
 *
 * Instruction layout:
 *   discriminator(8) + member(32) + kyc_status(1) + kyc_expiry(8)
 *   + risk_score(1) + jurisdiction(2) + aml_cleared(1) = 53 bytes
 *
 * Accounts:
 *   [authority(3), policy(0), kyc_entry(1), system_program(0)]
 */

import { useCallback } from "react";
import {
  type Address,
  getProgramDerivedAddress,
  getAddressEncoder,
} from "@solana/kit";
import { PROGRAM_ID, POLICY_PDA } from "../lib/devnet-config";
import { SYSTEM_PROGRAM_ID } from "../lib/ata-utils";
import { useWriteTransaction, type WriteTransactionResult } from "./useWriteTransaction";

// sha256("global:manage_kyc_entry")[..8]
const DISCRIMINATOR = new Uint8Array([177, 145, 128, 217, 214, 243, 153, 144]);

export interface ManageKycEntryParams {
  member: string;
  kycStatus: number; // 0=Pending, 1=Verified, 2=Expired, 3=Revoked
  kycExpiry: number; // unix timestamp
  riskScore: number; // 0-100
  jurisdiction: string; // 2-char ISO code
  amlCleared: boolean;
}

function encodeInstruction(params: ManageKycEntryParams): Uint8Array {
  const encoder = getAddressEncoder();
  const memberBytes = encoder.encode(params.member as Address);

  // Validate jurisdiction is exactly 2 ASCII characters
  if (params.jurisdiction.length !== 2) {
    throw new Error(
      `Jurisdiction must be a 2-character ISO code, got "${params.jurisdiction}"`,
    );
  }

  const buf = new ArrayBuffer(53);
  const bytes = new Uint8Array(buf);
  const view = new DataView(buf);

  bytes.set(DISCRIMINATOR, 0);
  bytes.set(memberBytes, 8); // member pubkey (32 bytes)
  view.setUint8(40, params.kycStatus);
  view.setBigInt64(41, BigInt(params.kycExpiry), true);
  view.setUint8(49, params.riskScore);
  bytes[50] = params.jurisdiction.charCodeAt(0);
  bytes[51] = params.jurisdiction.charCodeAt(1);
  bytes[52] = params.amlCleared ? 1 : 0;

  return bytes;
}

export function useManageKycEntry(): WriteTransactionResult<ManageKycEntryParams> {
  const buildInstruction = useCallback(
    async (authorityAddress: Address, params: ManageKycEntryParams) => {
      const encoder = getAddressEncoder();

      // Derive KYC entry PDA: ["kyc_entry", policy, member]
      const [kycEntryPda] = await getProgramDerivedAddress({
        programAddress: PROGRAM_ID as Address,
        seeds: [
          new TextEncoder().encode("kyc_entry"),
          encoder.encode(POLICY_PDA as Address),
          encoder.encode(params.member as Address),
        ],
      });

      return {
        programAddress: PROGRAM_ID as Address,
        accounts: [
          { address: authorityAddress, role: 3 as const },
          { address: POLICY_PDA as Address, role: 0 as const },
          { address: kycEntryPda, role: 1 as const },
          { address: SYSTEM_PROGRAM_ID, role: 0 as const },
        ],
        data: encodeInstruction(params),
      };
    },
    [],
  );

  return useWriteTransaction(buildInstruction);
}
