"use client";

/**
 * Hook: execute an on-chain swap transaction via execute_swap instruction.
 *
 * Builds the instruction manually (discriminator + SwapParams encoding)
 * following the same pattern as vault-card.tsx.
 *
 * Instruction layout:
 *   - 8 bytes discriminator: [56, 182, 124, 215, 155, 140, 157, 102]
 *   - 1 byte  token_in_index (u8)
 *   - 1 byte  token_out_index (u8)
 *   - 8 bytes amount_in (u64 LE)
 *   - 8 bytes expected_amount_out (u64 LE)
 *   - 8 bytes min_amount_out (u64 LE)
 *
 * Accounts (from IDL):
 *   [0] user          — writable + signer (role 3)
 *   [1] pool          — writable (role 1)
 *   [2] token_program — readonly (role 0)
 *
 * Remaining accounts (order matters):
 *   [3] vault_in      — writable (role 1)
 *   [4] vault_out     — writable (role 1)
 *   [5] user_ata_in   — writable (role 1)
 *   [6] user_ata_out  — writable (role 1)
 */

import { useState, useCallback } from "react";
import { useWalletConnection, useSendTransaction } from "@solana/react-hooks";
import { type Address } from "@solana/kit";
import { PROGRAM_ID, POOL_PDA, TICK_ADDRESSES } from "../lib/devnet-config";
import { TOKEN_PROGRAM_ID } from "../lib/ata-utils";

/** execute_swap instruction discriminator */
const SWAP_DISCRIMINATOR = new Uint8Array([56, 182, 124, 215, 155, 140, 157, 102]);

export interface SwapExecuteParams {
  tokenInIndex: number;
  tokenOutIndex: number;
  /** Amount in, SPL base units (u64) */
  amountIn: bigint;
  /** Expected output from off-chain quote, SPL base units (u64) */
  expectedAmountOut: bigint;
  /** Minimum acceptable output after slippage, SPL base units (u64) */
  minAmountOut: bigint;
  /** Vault address for the input token */
  vaultIn: string;
  /** Vault address for the output token */
  vaultOut: string;
  /** User's ATA for the input token */
  userAtaIn: string;
  /** User's ATA for the output token */
  userAtaOut: string;
  /** Dynamic tick account addresses from usePoolTicks (falls back to static config) */
  tickAddresses?: string[];
}

/**
 * Encode SwapParams into instruction data bytes.
 *
 * Layout: discriminator(8) + token_in_index(1) + token_out_index(1) +
 *         amount_in(8) + expected_amount_out(8) + min_amount_out(8) = 34 bytes
 */
function encodeSwapInstruction(params: SwapExecuteParams): Uint8Array {
  const buf = new ArrayBuffer(34);
  const bytes = new Uint8Array(buf);
  const view = new DataView(buf);

  // Discriminator
  bytes.set(SWAP_DISCRIMINATOR, 0);

  // SwapParams
  view.setUint8(8, params.tokenInIndex);
  view.setUint8(9, params.tokenOutIndex);
  view.setBigUint64(10, params.amountIn, true);
  view.setBigUint64(18, params.expectedAmountOut, true);
  view.setBigUint64(26, params.minAmountOut, true);

  return bytes;
}

export function useExecuteSwap() {
  const { wallet } = useWalletConnection();
  const { send, isSending } = useSendTransaction();
  const [signature, setSignature] = useState<string | null>(null);
  const [error, setError] = useState<Error | null>(null);

  const execute = useCallback(
    async (params: SwapExecuteParams): Promise<string> => {
      const userAddress = wallet?.account.address;
      if (!userAddress) {
        throw new Error("Wallet not connected");
      }

      setError(null);
      setSignature(null);

      const data = encodeSwapInstruction(params);

      const instruction = {
        programAddress: PROGRAM_ID as Address,
        accounts: [
          // Named accounts from IDL
          { address: userAddress, role: 3 as const },          // user: WritableSigner
          { address: POOL_PDA as Address, role: 1 as const },  // pool: Writable
          { address: TOKEN_PROGRAM_ID, role: 0 as const },     // token_program: Readonly
          // Remaining accounts (vault_in, vault_out, user_ata_in, user_ata_out, ...ticks)
          { address: params.vaultIn as Address, role: 1 as const },
          { address: params.vaultOut as Address, role: 1 as const },
          { address: params.userAtaIn as Address, role: 1 as const },
          { address: params.userAtaOut as Address, role: 1 as const },
          // All tick accounts must be provided (writable) when pool has ticks.
          // On-chain guard: tick_accounts.len() == pool.tick_count
          ...(params.tickAddresses ?? TICK_ADDRESSES).map((addr) => ({
            address: addr as Address,
            role: 1 as const,
          })),
        ],
        data,
      };

      try {
        const sig = await send({ instructions: [instruction] });
        const sigStr = sig ? String(sig) : "";
        setSignature(sigStr);
        return sigStr;
      } catch (err) {
        const e = err instanceof Error ? err : new Error(String(err));
        setError(e);
        throw e;
      }
    },
    [wallet, send],
  );

  return { execute, isSending, signature, error };
}
