"use client";

/**
 * Hook: execute an on-chain remove_liquidity transaction.
 *
 * Instruction layout:
 *   - 8 bytes discriminator: [80, 85, 209, 72, 24, 206, 177, 108]
 *   - 16 bytes liquidity_raw: i128 LE
 *
 * Accounts (from IDL):
 *   [0] provider       — writable + signer (role 3)
 *   [1] pool           — writable (role 1)
 *   [2] position       — writable (role 1)
 *   [3] token_program  — readonly (role 0)
 *
 * Remaining accounts (2 × n_assets):
 *   [0..n)  = vault token accounts (writable)
 *   [n..2n) = provider ATAs (writable)
 */

import { useState, useCallback } from "react";
import { useWalletConnection, useSendTransaction } from "@solana/react-hooks";
import { type Address } from "@solana/kit";
import { PROGRAM_ID, POOL_PDA } from "../lib/devnet-config";
import { TOKEN_PROGRAM_ID, deriveAta } from "../lib/ata-utils";
import { TOKENS } from "../lib/tokens";

const REMOVE_LIQ_DISCRIMINATOR = new Uint8Array([80, 85, 209, 72, 24, 206, 177, 108]);

export interface RemoveLiquidityExecuteParams {
  /** Position account address (PDA) */
  positionAddress: string;
  /** Full liquidity amount to withdraw (i128 raw value from PositionState) */
  liquidityRaw: bigint;
}

/**
 * Encode RemoveLiquidityParams: discriminator(8) + liquidity_raw(i128 LE) = 24 bytes
 */
function encodeRemoveLiquidityInstruction(liquidityRaw: bigint): Uint8Array {
  const buf = new ArrayBuffer(24);
  const bytes = new Uint8Array(buf);
  const view = new DataView(buf);

  bytes.set(REMOVE_LIQ_DISCRIMINATOR, 0);

  // i128 LE: low u64 + high i64
  const low = liquidityRaw & ((1n << 64n) - 1n);
  const high = liquidityRaw >> 64n;
  view.setBigUint64(8, low, true);
  view.setBigInt64(16, high, true);

  return bytes;
}

export function useRemoveLiquidity() {
  const { wallet } = useWalletConnection();
  const { send, isSending } = useSendTransaction();
  const [error, setError] = useState<Error | null>(null);

  const execute = useCallback(
    async (params: RemoveLiquidityExecuteParams): Promise<string> => {
      const userAddress = wallet?.account.address;
      if (!userAddress) throw new Error("Wallet not connected");

      setError(null);

      // Derive user ATAs for all 3 tokens
      const userAtas = await Promise.all(
        TOKENS.map((t) => deriveAta(userAddress, t.mint as Address)),
      );

      const data = encodeRemoveLiquidityInstruction(params.liquidityRaw);

      const instruction = {
        programAddress: PROGRAM_ID as Address,
        accounts: [
          { address: userAddress, role: 3 as const },
          { address: POOL_PDA as Address, role: 1 as const },
          { address: params.positionAddress as Address, role: 1 as const },
          { address: TOKEN_PROGRAM_ID, role: 0 as const },
          // remaining_accounts: [vault0, vault1, vault2, ata0, ata1, ata2]
          ...TOKENS.map((t) => ({ address: t.vault as Address, role: 1 as const })),
          ...userAtas.map((ata) => ({ address: ata, role: 1 as const })),
        ],
        data,
      };

      try {
        const sig = await send({ instructions: [instruction] });
        return sig ? String(sig) : "";
      } catch (err) {
        const e = err instanceof Error ? err : new Error(String(err));
        setError(e);
        throw e;
      }
    },
    [wallet, send],
  );

  return { execute, isSending, error };
}
