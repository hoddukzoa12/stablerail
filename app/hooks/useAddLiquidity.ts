"use client";

/**
 * Hook: execute an on-chain add_liquidity transaction.
 *
 * Instruction layout:
 *   - 8 bytes discriminator: [181, 157, 89, 67, 143, 182, 52, 72]
 *   - 64 bytes amounts: [u64; 8] LE — first n_assets used, rest zeroed
 *
 * Accounts (from IDL):
 *   [0] provider       — writable + signer (role 3)
 *   [1] pool           — writable (role 1)
 *   [2] position       — writable (role 1) — init PDA
 *   [3] system_program — readonly (role 0)
 *   [4] token_program  — readonly (role 0)
 *
 * Remaining accounts (2 × n_assets + optional tick):
 *   [0..n)  = vault token accounts (writable)
 *   [n..2n) = provider ATAs (writable)
 *   [2n]    = optional tick account (writable, for concentrated liquidity)
 */

import { useState, useCallback } from "react";
import { useWalletConnection, useSendTransaction } from "@solana/react-hooks";
import { type Address, getProgramDerivedAddress, getAddressEncoder } from "@solana/kit";
import { PROGRAM_ID, POOL_PDA } from "../lib/devnet-config";
import { TOKEN_PROGRAM_ID, deriveAta } from "../lib/ata-utils";
import { TOKENS } from "../lib/tokens";
import type { PoolState } from "../lib/stablerail-math";

const ADD_LIQ_DISCRIMINATOR = new Uint8Array([181, 157, 89, 67, 143, 182, 52, 72]);
const SYSTEM_PROGRAM_ID = "11111111111111111111111111111111" as Address;

export interface AddLiquidityExecuteParams {
  /** Base-unit deposit amounts per token (3 entries for our pool) */
  amounts: bigint[];
  /** Optional tick address for concentrated liquidity */
  tickAddress?: string;
}

/**
 * Encode AddLiquidityParams: discriminator(8) + amounts([u64; 8]) = 72 bytes
 */
function encodeAddLiquidityInstruction(amounts: bigint[]): Uint8Array {
  const buf = new ArrayBuffer(72);
  const bytes = new Uint8Array(buf);
  const view = new DataView(buf);

  bytes.set(ADD_LIQ_DISCRIMINATOR, 0);

  for (let i = 0; i < 8; i++) {
    view.setBigUint64(8 + i * 8, i < amounts.length ? amounts[i] : 0n, true);
  }

  return bytes;
}

export function useAddLiquidity() {
  const { wallet } = useWalletConnection();
  const { send, isSending } = useSendTransaction();
  const [error, setError] = useState<Error | null>(null);

  const execute = useCallback(
    async (params: AddLiquidityExecuteParams, pool: PoolState): Promise<string> => {
      const userAddress = wallet?.account.address;
      if (!userAddress) throw new Error("Wallet not connected");

      setError(null);

      const encoder = getAddressEncoder();

      // Derive position PDA: seeds = [b"position", pool, provider, position_count]
      const positionCountBytes = new Uint8Array(8);
      new DataView(positionCountBytes.buffer).setBigUint64(0, BigInt(pool.positionCount), true);

      const [positionPda] = await getProgramDerivedAddress({
        programAddress: PROGRAM_ID as Address,
        seeds: [
          new TextEncoder().encode("position"),
          encoder.encode(POOL_PDA as Address),
          encoder.encode(userAddress),
          positionCountBytes,
        ],
      });

      // Derive user ATAs for all 3 tokens
      const userAtas = await Promise.all(
        TOKENS.map((t) => deriveAta(userAddress, t.mint as Address)),
      );

      const data = encodeAddLiquidityInstruction(params.amounts);

      const instruction = {
        programAddress: PROGRAM_ID as Address,
        accounts: [
          { address: userAddress, role: 3 as const },
          { address: POOL_PDA as Address, role: 1 as const },
          { address: positionPda, role: 1 as const },
          { address: SYSTEM_PROGRAM_ID, role: 0 as const },
          { address: TOKEN_PROGRAM_ID, role: 0 as const },
          // remaining_accounts: [vault0..n, ata0..n, optional tick]
          ...TOKENS.map((t) => ({ address: t.vault as Address, role: 1 as const })),
          ...userAtas.map((ata) => ({ address: ata, role: 1 as const })),
          // Append tick account if concentrated liquidity
          ...(params.tickAddress
            ? [{ address: params.tickAddress as Address, role: 1 as const }]
            : []),
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
