"use client";

/**
 * Hook: execute an on-chain create_tick transaction.
 *
 * Instruction layout:
 *   - 8 bytes discriminator: [227, 158, 200, 168, 122, 104, 133, 81]
 *   - 16 bytes k_raw: i128 LE — the plane constant k in Q64.64
 *
 * Accounts (from IDL):
 *   [0] creator       — writable + signer (role 3)
 *   [1] pool          — writable (role 1)
 *   [2] tick          — writable (role 1) — init PDA
 *   [3] system_program — readonly (role 0)
 *
 * Tick PDA seeds: ["tick", pool_pubkey, k_raw_le_bytes]
 */

import { useState, useCallback } from "react";
import { useWalletConnection, useSendTransaction } from "@solana/react-hooks";
import { type Address, getProgramDerivedAddress, getAddressEncoder } from "@solana/kit";
import { PROGRAM_ID, POOL_PDA } from "../lib/devnet-config";

const CREATE_TICK_DISCRIMINATOR = new Uint8Array([227, 158, 200, 168, 122, 104, 133, 81]);
const SYSTEM_PROGRAM_ID = "11111111111111111111111111111111" as Address;

export interface CreateTickParams {
  /** Q64.64 raw value for the plane constant k (i128 as bigint) */
  kRaw: bigint;
}

/**
 * Encode CreateTickParams: discriminator(8) + k_raw(16) = 24 bytes
 */
function encodeCreateTickInstruction(kRaw: bigint): Uint8Array {
  const buf = new ArrayBuffer(24);
  const bytes = new Uint8Array(buf);
  const view = new DataView(buf);

  bytes.set(CREATE_TICK_DISCRIMINATOR, 0);

  // Write i128 LE: low 8 bytes at offset 8, high 8 bytes at offset 16
  // Handle signed i128 by treating as unsigned 128-bit in two's complement
  let value = kRaw;
  if (value < 0n) {
    value = (1n << 128n) + value; // two's complement
  }
  const lo = value & ((1n << 64n) - 1n);
  const hi = (value >> 64n) & ((1n << 64n) - 1n);
  view.setBigUint64(8, lo, true);
  view.setBigUint64(16, hi, true);

  return bytes;
}

export function useCreateTick() {
  const { wallet } = useWalletConnection();
  const { send, isSending } = useSendTransaction();
  const [error, setError] = useState<Error | null>(null);

  const execute = useCallback(
    async (params: CreateTickParams): Promise<string> => {
      const userAddress = wallet?.account.address;
      if (!userAddress) throw new Error("Wallet not connected");

      setError(null);

      const encoder = getAddressEncoder();

      // Derive tick PDA: seeds = ["tick", pool_pubkey, k_raw_le_bytes]
      // k_raw is i128 LE (16 bytes) — matches on-chain PDA derivation
      const kRawBytes = new Uint8Array(16);
      const kView = new DataView(kRawBytes.buffer);
      let value = params.kRaw;
      if (value < 0n) {
        value = (1n << 128n) + value; // two's complement
      }
      const lo = value & ((1n << 64n) - 1n);
      const hi = (value >> 64n) & ((1n << 64n) - 1n);
      kView.setBigUint64(0, lo, true);
      kView.setBigUint64(8, hi, true);

      const [tickPda] = await getProgramDerivedAddress({
        programAddress: PROGRAM_ID as Address,
        seeds: [
          new TextEncoder().encode("tick"),
          encoder.encode(POOL_PDA as Address),
          kRawBytes,
        ],
      });

      const data = encodeCreateTickInstruction(params.kRaw);

      const instruction = {
        programAddress: PROGRAM_ID as Address,
        accounts: [
          { address: userAddress, role: 3 as const },
          { address: POOL_PDA as Address, role: 1 as const },
          { address: tickPda, role: 1 as const },
          { address: SYSTEM_PROGRAM_ID, role: 0 as const },
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
