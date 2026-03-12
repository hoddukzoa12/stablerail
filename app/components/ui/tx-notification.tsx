"use client";

import { explorerUrl } from "../../lib/format-utils";

interface TxNotificationProps {
  /** Error to display (takes priority over txSignature) */
  error?: { message: string } | null;
  /** Successful transaction signature */
  txSignature?: string | null;
  /** Label for the success message (default: "Transaction confirmed!") */
  successLabel?: string;
}

/**
 * Reusable transaction result notification.
 * Shows either an error or a success message with Explorer link.
 */
export function TxNotification({
  error,
  txSignature,
  successLabel = "Transaction confirmed!",
}: TxNotificationProps) {
  if (!error && !txSignature) return null;

  return (
    <>
      {error && (
        <div className="mt-2 rounded-lg bg-error/10 px-3 py-2 text-center text-xs text-error">
          {error.message}
        </div>
      )}
      {txSignature && (
        <div className="mt-2 rounded-lg bg-success/10 px-3 py-2 text-center text-xs text-success">
          {successLabel}{" "}
          <a
            href={explorerUrl("tx", txSignature)}
            target="_blank"
            rel="noopener noreferrer"
            className="underline underline-offset-2"
          >
            View on Explorer
          </a>
        </div>
      )}
    </>
  );
}
