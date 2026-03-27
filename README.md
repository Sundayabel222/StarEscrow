# StarEscrow

Programmable escrow protocol for freelance and marketplace payments on Stellar using Soroban smart contracts.

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

---

## Quick Start

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) with `wasm32-unknown-unknown` target
- [Stellar CLI](https://developers.stellar.org/docs/tools/developer-tools/cli/stellar-cli) (`stellar`)

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
