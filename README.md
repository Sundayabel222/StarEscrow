# StarEscrow

[![CI](https://github.com/henry-peters/StarEscrow/actions/workflows/ci.yml/badge.svg)](https://github.com/henry-peters/StarEscrow/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

StarEscrow is a programmable escrow protocol for freelance and marketplace payments on the Stellar network, built with Soroban smart contracts. It locks funds on-chain when a job is created, optionally routes them through a yield protocol while work is in progress, and releases payment to the freelancer upon payer approval — or refunds the payer on cancellation or deadline expiry. A configurable fee (in basis points) is deducted from the released amount and forwarded to a fee collector address.

## Architecture

### Component Diagram

```mermaid
graph TD
    subgraph Clients
        CLI["star-escrow CLI<br/>(clients/cli)"]
        DAPP["dApp / Web Client"]
    end

    subgraph "Stellar Network"
        CONTRACT["StarEscrow Contract<br/>(contracts/escrow)"]
        TOKEN["Token Contract<br/>(SEP-41 / Stellar Asset)"]
        YIELD["Yield Protocol<br/>(optional, external)"]
    end

    CLI -->|"invoke via stellar CLI"| CONTRACT
    DAPP -->|"Soroban RPC"| CONTRACT
    CONTRACT -->|"transfer / transfer_from"| TOKEN
    CONTRACT -->|"deposit / withdraw"| YIELD

    style CONTRACT fill:#1e3a5f,color:#fff
    style TOKEN fill:#2d6a4f,color:#fff
    style YIELD fill:#6b4c11,color:#fff
```

### State Machine

```mermaid
stateDiagram-v2
    [*] --> Active : create()

    Active --> WorkSubmitted : submit_work()\n[freelancer]
    Active --> Cancelled : cancel()\n[payer, before submission]
    Active --> Expired : expire()\n[payer, after deadline]

    WorkSubmitted --> Completed : approve()\n[payer]

    Completed --> [*]
    Cancelled --> [*]
    Expired --> [*]

    note right of Active
        Funds locked in contract.
        Yield accruing (if enabled).
    end note

    note right of Completed
        Funds (minus fee) released
        to freelancer.
    end note

    note right of Cancelled
        Full refund + yield
        returned to payer.
    end note

    note right of Expired
        Full refund + yield
        returned to payer after
        deadline has passed.
    end note
```

---

## Sequence Diagrams

### Happy Path

```mermaid
sequenceDiagram
    actor Payer
    actor Freelancer
    participant Contract as StarEscrow Contract
    participant Token as Token Contract
    participant Yield as Yield Protocol (optional)

    Payer->>Contract: create(payer, freelancer, token, amount, milestone, deadline?, yield_protocol?)
    Contract->>Token: transfer_from(payer → contract, amount)
    alt yield_protocol provided
        Contract->>Yield: deposit(amount)
        Contract-->>Payer: emit yield_deposited
    end
    Contract-->>Payer: emit escrow_created
    Note over Contract: status = Active

    Freelancer->>Contract: submit_work()
    Contract-->>Freelancer: emit work_submitted
    Note over Contract: status = WorkSubmitted

    Payer->>Contract: approve()
    alt yield enabled
        Contract->>Yield: withdraw(principal)
        Yield-->>Contract: (principal, yield_accrued)
    end
    Contract->>Token: transfer(fee → fee_collector)
    Contract->>Token: transfer(amount - fee [+ yield] → freelancer)
    Contract-->>Payer: emit payment_released
    Note over Contract: status = Completed
```

### Cancel Flow

```mermaid
sequenceDiagram
    actor Payer
    actor Freelancer
    participant Contract as StarEscrow Contract
    participant Token as Token Contract
    participant Yield as Yield Protocol (optional)

    Payer->>Contract: create(payer, freelancer, token, amount, milestone, ...)
    Contract->>Token: transfer_from(payer → contract, amount)
    Contract-->>Payer: emit escrow_created
    Note over Contract: status = Active

    Note over Freelancer: Work not yet submitted

    Payer->>Contract: cancel()
    Note over Contract: Only allowed when status = Active
    alt yield enabled
        Contract->>Yield: withdraw(principal)
        Yield-->>Contract: (principal, yield_accrued)
    end
    Contract->>Token: transfer(amount [+ yield] → payer)
    Contract-->>Payer: emit escrow_cancelled
    Note over Contract: status = Cancelled
```

### Expire Flow

```mermaid
sequenceDiagram
    actor Payer
    actor Freelancer
    participant Contract as StarEscrow Contract
    participant Token as Token Contract
    participant Yield as Yield Protocol (optional)

    Payer->>Contract: create(..., deadline=T, ...)
    Contract->>Token: transfer_from(payer → contract, amount)
    Contract-->>Payer: emit escrow_created
    Note over Contract: status = Active

    Note over Freelancer: Deadline T passes without work submission

    Payer->>Contract: expire()
    Note over Contract: Requires current_time > deadline<br/>and status = Active
    alt yield enabled
        Contract->>Yield: withdraw(principal)
        Yield-->>Contract: (principal, yield_accrued)
    end
    Contract->>Token: transfer(amount [+ yield] → payer)
    Contract-->>Payer: emit escrow_expired
    Note over Contract: status = Expired
```

---

## Documentation

- [Protocol Specification](docs/PROTOCOL.md) — States, transitions, functions, events, and security model
- [Deployment Guide](docs/DEPLOYMENT.md) — Build, deploy to testnet and mainnet, post-deployment checks
- [Security & Threat Model](docs/SECURITY.md) — Trusted parties, attack vectors, mitigations, and out-of-scope threats
- [Changelog](CHANGELOG.md) — Version history and notable changes

---

## Quick Start

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) with `wasm32-unknown-unknown` target
- [Stellar CLI](https://developers.stellar.org/docs/tools/developer-tools/cli/stellar-cli) (`stellar`)

### CLI Installation

The StarEscrow CLI (`star-escrow`) provides a convenient interface for interacting with the escrow contract.

#### Build from Source

1. Clone the repository:
   ```bash
   git clone https://github.com/henry-peters/StarEscrow.git
   cd StarEscrow
   ```

2. Build the CLI in release mode:
   ```bash
   cargo build --release -p cli
   ```

3. The binary will be located at:
   ```bash
   ./target/release/star-escrow
   ```

4. (Optional) Install it to your PATH:
   ```bash
   # Linux/macOS
   cp ./target/release/star-escrow /usr/local/bin/
   
   # Or using cargo install (if you have cargo-install)
   cargo install --path clients/cli
   ```

#### Prerequisites for CLI

- [Rust](https://www.rust-lang.org/tools/install) (latest stable version)
- [Stellar CLI](https://developers.stellar.org/docs/tools/developer-tools/cli/stellar-cli) (for contract interactions)

#### Verify Installation

After building, verify the CLI is working:

```bash
./target/release/star-escrow --help
```

You should see usage information for all available commands.

### Build

```bash
stellar contract build
```

### Test

```bash
cargo test -p escrow
```

### Deploy

See [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md) for full instructions.

## Usage

Set required environment variables:

```bash
export ESCROW_CONTRACT_ID=<contract-id>
export ADMIN_SECRET=<admin-secret-key>
export PAYER_SECRET=<payer-secret-key>
export FREELANCER_SECRET=<freelancer-secret-key>
```

Initialize the protocol (admin, one-time):

```bash
star-escrow init \
  --fee-bps 100 \
  --fee-collector <fee-collector-address>
```

Create an escrow and lock funds:

```bash
star-escrow create \
  --freelancer <freelancer-address> \
  --token <token-address> \
  --amount 1000000000 \
  --milestone "Deliver final design assets" \
  --deadline 1800000000
```

Freelancer submits work, payer approves:

```bash
star-escrow submit-work
star-escrow approve
```

Cancel before work is submitted (payer only):

```bash
star-escrow cancel
```

Run `star-escrow --help` for the full command reference.

## License

This project is licensed under the [MIT License](LICENSE).
