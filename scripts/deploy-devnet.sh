#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
ANCHOR_DIR="${ROOT_DIR}/anchor"
SO_PATH="${ANCHOR_DIR}/target/deploy/orbital.so"
KEYPAIR_PATH="${ANCHOR_DIR}/target/deploy/orbital-keypair.json"
PROGRAM_ID="CijRTA2wK1qLLHdYLP2jMEyac9NBZVUtWTUbiTYMPJda"

echo "=== Orbital Devnet Deploy ==="
echo ""

# ── 1. Always rebuild to avoid stale artifact deployment ──
echo "[1/4] Building SBF (anchor build)..."
(cd "${ANCHOR_DIR}" && anchor build)

# ── 2. Verify keypair matches program ID ──
if ! ACTUAL_PUBKEY=$(solana-keygen pubkey "${KEYPAIR_PATH}" 2>&1); then
  echo "ERROR: Failed to read keypair at ${KEYPAIR_PATH}:"
  echo "       ${ACTUAL_PUBKEY}"
  exit 1
fi
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
AIRDROP_SUCCESS=false
while [ "${ATTEMPTS}" -lt "${MAX_ATTEMPTS}" ]; do
  if solana airdrop 5 "${DEPLOYER}" --url devnet 2>&1; then
    AIRDROP_SUCCESS=true
    break
  else
    ATTEMPTS=$((ATTEMPTS + 1))
    echo "       Airdrop attempt ${ATTEMPTS}/${MAX_ATTEMPTS} failed — retrying in 5s..."
    sleep 5
  fi
done

if [ "${AIRDROP_SUCCESS}" = false ]; then
  echo "WARNING: All ${MAX_ATTEMPTS} airdrop attempts failed. Checking existing balance..."
fi

if ! BALANCE_OUTPUT=$(solana balance "${DEPLOYER}" --url devnet 2>&1); then
  echo "ERROR: Could not query balance:"
  echo "       ${BALANCE_OUTPUT}"
  exit 1
fi
BALANCE=$(echo "${BALANCE_OUTPUT}" | awk '{print int($1)}')
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
