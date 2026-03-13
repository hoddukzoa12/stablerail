/**
 * Associated Token Account (ATA) derivation utility.
 *
 * Derives ATAs using the standard SPL program seeds:
 *   [owner, TOKEN_PROGRAM_ID, mint]
 * with the ATA Program as the deriving program.
 */

import {
  getProgramDerivedAddress,
  getAddressEncoder,
  type Address,
} from "@solana/kit";

/** SPL Token Program ID */
export const TOKEN_PROGRAM_ID =
  "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA" as Address;

/** Associated Token Account Program ID */
export const ATA_PROGRAM_ID =
  "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL" as Address;

/** System Program ID */
export const SYSTEM_PROGRAM_ID =
  "11111111111111111111111111111111" as Address;

/**
 * Derive the Associated Token Account address for a given owner and mint.
 *
 * Seeds: [owner, TOKEN_PROGRAM_ID, mint]
 * Program: ATA_PROGRAM_ID
 *
 * @param owner - Wallet address of the token owner
 * @param mint - SPL token mint address
 * @returns The derived ATA address
 */
export async function deriveAta(
  owner: Address,
  mint: Address,
): Promise<Address> {
  const encoder = getAddressEncoder();

  const [ata] = await getProgramDerivedAddress({
    programAddress: ATA_PROGRAM_ID,
    seeds: [encoder.encode(owner), encoder.encode(TOKEN_PROGRAM_ID), encoder.encode(mint)],
  });

  return ata;
}
