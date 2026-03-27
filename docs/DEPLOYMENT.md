# Deployment Guide

This guide covers building the StarEscrow WASM contract and deploying it to Stellar testnet and mainnet.

---

## Prerequisites

### 1. Rust Toolchain

Install Rust via [rustup](https://rustup.rs/):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Add the WASM target:

```bash
rustup target add wasm32-unknown-unknown
```

Verify:

```bash
rustc --version
cargo --version
```

### 2. Stellar CLI

Install the Stellar CLI (v21 or later):

```bash
cargo install --locked stellar-cli --features opt
```

Or via the official installer — see [Stellar CLI docs](https://developers.stellar.org/docs/tools/developer-tools/cli/stellar-cli).

Verify:

```bash
stellar --version
```

### 3. Funded Accounts

You need at minimum two Stellar accounts:

| Account | Purpose |
|---------|---------|
| **Deployer** | Deploys the contract and pays transaction fees |
| **Admin** | Passed to `init()` as the protocol admin |

For testnet you can fund accounts with Friendbot (see below). For mainnet you must hold XLM.

---

## Step 1: Build the WASM

From the project root:

```bash
stellar contract build
```

This runs the release profile build (optimised, LTO, stripped) and produces:

```
target/wasm32-unknown-unknown/release/escrow.wasm
```

Verify the output exists:

```bash
ls -lh target/wasm32-unknown-unknown/release/escrow.wasm
```

> **Note:** The release profile is configured in the root `Cargo.toml` with `opt-level = "z"`, `lto = true`, and `strip = "symbols"` to minimise contract size and fee cost.

---

## Step 2: Deploy to Testnet

### 2.1 Configure the Testnet Network

```bash
stellar network add testnet \
  --rpc-url https://soroban-testnet.stellar.org \
  --network-passphrase "Test SDF Network ; September 2015"
```

### 2.2 Create and Fund a Deployer Account

```bash
stellar keys generate deployer --network testnet
stellar keys address deployer
```

Fund via Friendbot:

```bash
stellar keys fund deployer --network testnet
```

Confirm balance:

```bash
stellar account show $(stellar keys address deployer) --network testnet
```

### 2.3 Upload the Contract WASM

```bash
stellar contract upload \
  --wasm target/wasm32-unknown-unknown/release/escrow.wasm \
  --source deployer \
  --network testnet
```

This prints a **WASM hash** — save it:

```
WASM Hash: <hash>
```

### 2.4 Deploy a Contract Instance

```bash
stellar contract deploy \
  --wasm-hash <hash> \
  --source deployer \
  --network testnet
```

This prints the **Contract ID** — save it:

```
Contract deployed: C<contract-id>
```

### 2.5 Initialise the Protocol

```bash
stellar contract invoke \
  --id <contract-id> \
  --source deployer \
  --network testnet \
  -- init \
  --admin <admin-address> \
  --fee_bps 250 \
  --fee_collector <fee-collector-address>
```

Replace:
- `<admin-address>` — the admin's Stellar address (G…)
- `<fee-collector-address>` — where protocol fees should be sent
- `250` — fee in basis points (2.5%); set `0` for no fee

---

## Step 3: Deploy to Mainnet

### 3.1 Configure the Mainnet Network

```bash
stellar network add mainnet \
  --rpc-url https://mainnet.stellar.validationcloud.io/v1/<your-api-key> \
  --network-passphrase "Public Global Stellar Network ; September 2015"
```

> Use a reliable RPC provider. Public options include [Validation Cloud](https://validationcloud.io/), [QuickNode](https://www.quicknode.com/), and [Blockdaemon](https://blockdaemon.com/).

### 3.2 Import Your Deployer Key

Import an existing funded account:

```bash
stellar keys add deployer --secret-key
# Enter your secret key (S…) at the prompt
```

> **Never** commit secret keys to version control.

Confirm the account is funded with sufficient XLM to cover upload, deploy, and invocation fees (recommend ≥ 10 XLM).

### 3.3 Upload the WASM

```bash
stellar contract upload \
  --wasm target/wasm32-unknown-unknown/release/escrow.wasm \
  --source deployer \
  --network mainnet
```

> If the exact WASM was already uploaded (identical hash), this is a no-op and returns the existing hash.

### 3.4 Deploy a Contract Instance

```bash
stellar contract deploy \
  --wasm-hash <hash> \
  --source deployer \
  --network mainnet
```

### 3.5 Initialise the Protocol

```bash
stellar contract invoke \
  --id <contract-id> \
  --source deployer \
  --network mainnet \
  -- init \
  --admin <admin-address> \
  --fee_bps <fee-bps> \
  --fee_collector <fee-collector-address>
```

---

## Step 4: Post-Deployment Verification

### 4.1 Verify Contract Is Live

Retrieve the escrow status (will fail if `init` was not called or escrow does not exist yet — this is expected):

```bash
stellar contract invoke \
  --id <contract-id> \
  --source deployer \
  --network testnet \
  -- get_escrow
```

Expected result for a freshly deployed, uninitialised escrow: contract error or empty return (no escrow created yet).

### 4.2 Run a Smoke Test on Testnet

Create a test escrow end-to-end using the CLI:

```bash
# Set env vars for convenience
export CONTRACT_ID=<contract-id>
export NETWORK=testnet

# Create an escrow (payer must have token balance)
star-escrow create \
  --payer <payer-address> \
  --freelancer <freelancer-address> \
  --token <token-address> \
  --amount 100 \
  --milestone "Deliver project" \
  --source <payer-secret-or-key> \
  --network $NETWORK \
  --contract-id $CONTRACT_ID

# Check status
star-escrow status \
  --contract-id $CONTRACT_ID \
  --network $NETWORK
```

Expected output: `status: Active`.

### 4.3 Verify Events Are Emitted

Query events for the contract to confirm `escrow_created` was emitted:

```bash
stellar events \
  --contract-id <contract-id> \
  --network testnet
```

### 4.4 Verify Fee Configuration

Invoke `approve()` on the test escrow and confirm:
1. The `fee_collector` received the expected fee (`amount * fee_bps / 10_000`).
2. The freelancer received the remainder.

### 4.5 Confirm Admin Controls

Test pause/unpause to confirm admin auth is correctly wired:

```bash
stellar contract invoke \
  --id <contract-id> \
  --source <admin-key> \
  --network testnet \
  -- pause

stellar contract invoke \
  --id <contract-id> \
  --source <admin-key> \
  --network testnet \
  -- unpause
```

---

## Deploying Multiple Escrows

Each `stellar contract deploy` call creates a new, independent contract instance. Deploy one instance per escrow engagement and pass the resulting contract ID to each party.

---

## Using the CLI Tool

Build the CLI:

```bash
cargo build -p cli --release
```

Binary location: `target/release/star-escrow`

Run with `--help` for all available commands:

```bash
./target/release/star-escrow --help
```

See the `clients/cli/` directory for full CLI documentation.

---

## Troubleshooting

| Issue | Likely Cause | Fix |
|-------|-------------|-----|
| `AlreadyExists` on `init` | `init()` already called | Each contract instance can only be initialised once |
| `Paused` error | Admin paused the contract | Call `unpause()` from admin account |
| Insufficient balance | Payer lacks token balance | Fund payer with the token before calling `create()` |
| `DeadlineNotPassed` | `expire()` called too early | Wait until ledger timestamp exceeds deadline |
| `NotActive` on `cancel` | Work already submitted | Cannot cancel after `submit_work()` |
| Upload fails with size error | WASM too large | Ensure you built with `stellar contract build` (release profile) |
