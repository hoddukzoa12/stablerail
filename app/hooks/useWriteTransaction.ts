"use client";

/**
 * Shared hook for sending single-instruction Solana transactions.
 *
 * Encapsulates the common pattern: wallet check, state management,
 * instruction building, send, and error handling.
 */

import { useState, useCallback } from "react";
import { useWalletConnection, useSendTransaction } from "@solana/react-hooks";
import { type Address } from "@solana/kit";

interface AccountMeta {
  address: Address;
  role: 0 | 1 | 2 | 3;
}

interface SolanaInstruction {
  programAddress: Address;
  accounts: AccountMeta[];
  data: Uint8Array;
}

export interface WriteTransactionResult<P> {
  execute: (params: P) => Promise<string>;
  isSending: boolean;
  signature: string | null;
  error: Error | null;
}

/**
 * Generic write-transaction hook that eliminates boilerplate across
 * instruction-specific hooks.
 *
 * @param buildInstruction - Receives the signer address and caller params,
 *   returns the fully-built instruction (may be async for PDA derivation).
 */
export function useWriteTransaction<P>(
  buildInstruction: (
    signerAddress: Address,
    params: P,
  ) => SolanaInstruction | Promise<SolanaInstruction>,
): WriteTransactionResult<P> {
  const { wallet } = useWalletConnection();
  const { send, isSending } = useSendTransaction();
  const [signature, setSignature] = useState<string | null>(null);
  const [error, setError] = useState<Error | null>(null);

  const execute = useCallback(
    async (params: P): Promise<string> => {
      const signerAddress = wallet?.account.address;
      if (!signerAddress) throw new Error("Wallet not connected");

      setError(null);
      setSignature(null);

      const instruction = await buildInstruction(signerAddress, params);

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
    [wallet, send, buildInstruction],
  );

  return { execute, isSending, signature, error };
}
