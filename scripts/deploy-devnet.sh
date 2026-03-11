#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
ANCHOR_DIR="${ROOT_DIR}/anchor"
SO_PATH="${ANCHOR_DIR}/target/deploy/orbital.so"
KEYPAIR_PATH="${ANCHOR_DIR}/target/deploy/orbital-keypair.json"
PROGRAM_ID="C7dFX4QVV8QCdzP4fZi3Vcx8oP1cYhTaXD7kvvat8W1w"

echo "=== Orbital Devnet Deploy ==="
echo ""

# ── 1. Verify or build .so ──
if [ ! -f "${SO_PATH}" ]; then
  echo "[1/4] .so not found — building SBF..."
  (cd "${ANCHOR_DIR}" && anchor build)
else
  echo "[1/4] Found orbital.so — skipping build"
fi

# ── 2. Verify keypair matches program ID ──
ACTUAL_PUBKEY=$(solana-keygen pubkey "${KEYPAIR_PATH}" 2>/dev/null)
if [ "${ACTUAL_PUBKEY}" != "${PROGRAM_ID}" ]; then
  echo "ERROR: Keypair pubkey ${ACTUAL_PUBKEY} does not match program ID ${PROGRAM_ID}"
  echo "       Check anchor/target/deploy/orbital-keypair.json"
  exit 1
fi
echo "       Keypair verified: ${PROGRAM_ID}"

# ── 3. Set to devnet ──
solana config set --url devnet > /dev/null
echo "[2/4] Solana CLI set to devnet"

# ── 4. Generate deployer wallet if needed ──
WALLET_PATH="${HOME}/.config/solana/id.json"
if [ ! -f "${WALLET_PATH}" ]; then
  echo "       Generating deployer keypair at ${WALLET_PATH}..."
  solana-keygen new --no-bip39-passphrase --outfile "${WALLET_PATH}"
fi
DEPLOYER=$(solana-keygen pubkey "${WALLET_PATH}")
echo "       Deployer: ${DEPLOYER}"

# ── 5. Airdrop with retry ──
echo "[3/4] Requesting SOL airdrop..."
ATTEMPTS=0
MAX_ATTEMPTS=5
while [ "${ATTEMPTS}" -lt "${MAX_ATTEMPTS}" ]; do
  if solana airdrop 5 "${DEPLOYER}" --url devnet 2>/dev/null; then
    break
  else
    ATTEMPTS=$((ATTEMPTS + 1))
    echo "       Airdrop attempt ${ATTEMPTS}/${MAX_ATTEMPTS} failed — retrying in 5s..."
    sleep 5
  fi
done

BALANCE=$(solana balance "${DEPLOYER}" --url devnet 2>/dev/null | awk '{print int($1)}')
if [ "${BALANCE}" -lt 2 ]; then
  echo "ERROR: Insufficient balance (${BALANCE} SOL). Need at least 2 SOL for deploy."
  echo "       Try: solana airdrop 5 ${DEPLOYER} --url devnet"
  exit 1
fi
echo "       Balance: ${BALANCE} SOL"

# ── 6. Deploy ──
echo "[4/4] Deploying orbital program to devnet..."
(cd "${ANCHOR_DIR}" && anchor deploy)

echo ""
echo "=== Deploy complete ==="
echo "Program ID: ${PROGRAM_ID}"
echo "Deployer:   ${DEPLOYER}"
echo ""
echo "Next step:"
echo "  cd scripts && npm install && npm run bootstrap"
