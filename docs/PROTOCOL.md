# StarEscrow Protocol Specification

This document is the authoritative reference for the StarEscrow escrow protocol running on Stellar via Soroban smart contracts.

---

## 1. Overview

StarEscrow is a single-milestone, bilateral escrow protocol. A **payer** locks funds on-chain when engaging a **freelancer**. The funds are held by the contract until one of four outcomes occurs: approval, cancellation, expiry, or — in a future extension — dispute resolution.

One contract instance manages one escrow. Deploy a new contract instance per engagement.

---

## 2. Roles

| Role | Description |
|------|-------------|
| **Admin** | Initialises the protocol config. Can pause and unpause the contract. Set once via `init()`. |
| **Payer** | Locks funds, approves work, cancels, or reclaims after deadline. |
| **Freelancer** | Performs work and submits it for approval. May transfer the role to another address. |
| **Fee Collector** | Receives the protocol fee on `approve()`. Configured by admin. |

---

## 3. States and Transitions

### 3.1 EscrowStatus

```
Active          — Funds locked. Awaiting work submission.
WorkSubmitted   — Freelancer has submitted work. Awaiting payer approval.
Completed       — Payer approved; funds released to freelancer.
Cancelled       — Payer cancelled before work was submitted; funds returned to payer.
Expired         — Deadline passed; payer reclaimed funds.
```

### 3.2 Valid Transitions

| From | To | Trigger | Who |
|------|----|---------|-----|
| *(none)* | `Active` | `create()` | Payer |
| `Active` | `WorkSubmitted` | `submit_work()` | Freelancer |
| `WorkSubmitted` | `Completed` | `approve()` | Payer |
| `Active` | `Cancelled` | `cancel()` | Payer |
| `Active` | `Expired` | `expire()` (after deadline) | Payer |

`Completed`, `Cancelled`, and `Expired` are terminal states. No further transitions are possible.

---

## 4. Data Structures

### 4.1 EscrowData

```rust
pub struct EscrowData {
    pub payer: Address,                  // Account that locked funds
    pub freelancer: Address,             // Account performing work
    pub token: Address,                  // SEP-41 token contract address
    pub amount: i128,                    // Locked amount (in token's smallest unit)
    pub milestone: String,               // Human-readable work description
    pub status: EscrowStatus,            // Current state
    pub deadline: Option<u64>,           // Unix timestamp; None = no deadline
    pub yield_protocol: Option<Address>, // Optional yield contract address
    pub principal_deposited: i128,       // Amount forwarded to yield protocol
    pub yield_recipient: YieldRecipient, // Who receives accrued yield
}
```

### 4.2 YieldRecipient

```rust
pub enum YieldRecipient {
    Payer,       // Yield goes to payer on release/cancel/expire
    Freelancer,  // Yield goes to freelancer on release/cancel/expire
}
```

### 4.3 ProtocolConfig

```rust
pub struct ProtocolConfig {
    pub admin: Address,          // Privileged admin address
    pub paused: bool,            // Global pause flag
    pub fee_bps: u32,            // Protocol fee in basis points (100 = 1%)
    pub fee_collector: Address,  // Receives protocol fees
}
```

---

## 5. Contract Functions

### 5.1 Administrative

#### `init(admin, fee_bps, fee_collector) → Result<(), EscrowError>`

Initialises the protocol configuration. Must be called exactly once before any escrow is created.

| Parameter | Type | Description |
|-----------|------|-------------|
| `admin` | `Address` | Admin address; required for `pause`/`unpause`. |
| `fee_bps` | `u32` | Protocol fee in basis points (e.g. `250` = 2.5%). |
| `fee_collector` | `Address` | Address that receives collected fees. |

**Effects:** Stores `ProtocolConfig`. Fails with `AlreadyExists` if config exists.
**Auth:** Requires `admin` signature.

---

#### `pause() → Result<(), EscrowError>`

Halts all state-changing operations. Read-only calls (`get_status`, `get_escrow`) remain available.

**Effects:** Sets `paused = true` in `ProtocolConfig`. Emits `contract_paused`.
**Auth:** Requires admin signature.

---

#### `unpause() → Result<(), EscrowError>`

Resumes normal operation.

**Effects:** Sets `paused = false` in `ProtocolConfig`. Emits `contract_unpaused`.
**Auth:** Requires admin signature.

---

### 5.2 Core Escrow

#### `create(payer, freelancer, token, amount, milestone, deadline?, yield_protocol?, yield_recipient) → Result<(), EscrowError>`

Creates the escrow and transfers funds into the contract.

| Parameter | Type | Description |
|-----------|------|-------------|
| `payer` | `Address` | Funds source and approver. |
| `freelancer` | `Address` | Work performer. |
| `token` | `Address` | SEP-41 token contract. |
| `amount` | `i128` | Amount to lock (must be > 0). |
| `milestone` | `String` | Description of the deliverable. |
| `deadline` | `Option<u64>` | Unix timestamp after which `expire()` is allowed. `None` disables expiry. |
| `yield_protocol` | `Option<Address>` | If set, locked funds are deposited into this yield contract. |
| `yield_recipient` | `YieldRecipient` | `Payer` or `Freelancer` — who receives accrued yield. |

**Effects:**
1. Transfers `amount` tokens from `payer` to the contract via `transfer_from`.
2. If `yield_protocol` is set, deposits `amount` into the yield contract and stores `principal_deposited`.
3. Stores `EscrowData` with `status = Active`.
4. Emits `escrow_created`. Emits `yield_deposited` if yield is enabled.

**Auth:** Requires `payer` signature.
**Fails with:** `AlreadyExists` if escrow exists, `InvalidAmount` if `amount ≤ 0`, `Paused` if contract is paused.

---

#### `submit_work() → Result<(), EscrowError>`

Freelancer signals that the deliverable is ready for review.

**Effects:** Sets `status = WorkSubmitted`. Emits `work_submitted`.
**Auth:** Requires `freelancer` signature.
**Fails with:** `NotActive` if status ≠ `Active`, `Paused` if contract is paused.

---

#### `approve() → Result<(), EscrowError>`

Payer confirms that the deliverable meets the milestone criteria, releasing payment.

**Effects:**
1. If yield is enabled, calls `yield_protocol.withdraw(principal_deposited)` to retrieve `(principal, yield_accrued)`.
2. Calculates protocol fee: `fee = amount * fee_bps / 10_000`.
3. Transfers `fee` to `fee_collector`.
4. Transfers `amount - fee` (plus `yield_accrued` if `yield_recipient = Freelancer`) to `freelancer`.
5. Sets `status = Completed`. Emits `payment_released`.

**Auth:** Requires `payer` signature.
**Fails with:** `WorkNotSubmitted` if status ≠ `WorkSubmitted`, `Paused` if paused.

---

#### `cancel() → Result<(), EscrowError>`

Payer aborts the engagement before work is submitted, recovering all locked funds.

**Effects:**
1. If yield is enabled, withdraws `principal + yield_accrued` from yield protocol.
2. Returns all funds (+ yield if `yield_recipient = Payer`) to `payer`.
3. Sets `status = Cancelled`. Emits `escrow_cancelled`.

**Auth:** Requires `payer` signature.
**Fails with:** `NotActive` if status ≠ `Active`, `Paused` if paused.

---

#### `expire() → Result<(), EscrowError>`

Payer reclaims funds after the deadline has passed and work has not been submitted.

**Effects:**
1. Validates `current_ledger_timestamp > deadline`.
2. If yield is enabled, withdraws `principal + yield_accrued` from yield protocol.
3. Returns all funds (+ yield if `yield_recipient = Payer`) to `payer`.
4. Sets `status = Expired`. Emits `escrow_expired`.

**Auth:** Requires `payer` signature.
**Fails with:** `NotActive` if status ≠ `Active`, `NotExpired` if no deadline is set, `DeadlineNotPassed` if deadline has not yet passed, `Paused` if paused.

---

#### `transfer_freelancer(new_freelancer) → Result<(), EscrowError>`

Current freelancer transfers the role to a new address (e.g. to delegate to a sub-contractor).

| Parameter | Type | Description |
|-----------|------|-------------|
| `new_freelancer` | `Address` | Address taking over the freelancer role. |

**Effects:** Updates `escrow.freelancer`. Emits `freelancer_transferred`. Does not change `status`.
**Auth:** Requires current `freelancer` signature.
**Fails with:** `Paused` if paused.

---

### 5.3 Queries

#### `get_status() → EscrowStatus`

Returns the current `EscrowStatus`. Panics if the escrow has not been created.
**Auth:** None.

---

#### `get_escrow() → EscrowData`

Returns the full `EscrowData` struct. Panics if the escrow has not been created.
**Auth:** None.

---

## 6. Events

All events are emitted via `env.events().publish(topics, data)`.

| Event | Topics | Data | Emitted by |
|-------|--------|------|------------|
| `escrow_created` | `("escrow_created",)` | `(payer, freelancer, amount, milestone)` | `create()` |
| `yield_deposited` | `("yield_deposited",)` | `(protocol, amount)` | `create()` when yield enabled |
| `work_submitted` | `("work_submitted",)` | `(freelancer,)` | `submit_work()` |
| `payment_released` | `("payment_released",)` | `(freelancer, amount)` | `approve()` |
| `escrow_cancelled` | `("escrow_cancelled",)` | `(payer, amount)` | `cancel()` |
| `escrow_expired` | `("escrow_expired",)` | `(payer, amount)` | `expire()` |
| `freelancer_transferred` | `("freelancer_transferred",)` | `(old_freelancer, new_freelancer)` | `transfer_freelancer()` |
| `contract_paused` | `("contract_paused",)` | `(admin,)` | `pause()` |
| `contract_unpaused` | `("contract_unpaused",)` | `(admin,)` | `unpause()` |

---

## 7. Error Codes

| Code | Name | Meaning |
|------|------|---------|
| 1 | `AlreadyExists` | Config or escrow already initialised |
| 2 | `NotActive` | Operation requires `Active` status |
| 3 | `WorkNotSubmitted` | Operation requires `WorkSubmitted` status |
| 4 | `InvalidAmount` | `amount ≤ 0` |
| 5 | `DeadlineNotPassed` | `expire()` called before deadline |
| 6 | `NotExpired` | `expire()` called but no deadline was set |
| 7 | `Unauthorized` | Caller lacks required authentication |
| 8 | `Paused` | Contract is paused; operation rejected |

---

## 8. Fee Mechanism

Protocol fees are applied only on successful approval:

```
fee        = amount * fee_bps / 10_000
net_payout = amount - fee
```

`fee_bps = 250` corresponds to a 2.5% fee. `fee_bps = 0` disables fees. The maximum meaningful value is `10_000` (100%).

Fees are **not** applied on `cancel()` or `expire()` — the payer receives a full refund.

---

## 9. Yield Protocol Integration

When a `yield_protocol` address is provided at creation:

1. `create()` calls `yield_protocol.deposit(amount)` immediately after transferring funds into the contract.
2. The deposited `principal` is tracked in `EscrowData.principal_deposited`.
3. On any terminal transition (`approve`, `cancel`, `expire`), the contract calls `yield_protocol.withdraw(principal_deposited)`, which returns `(principal_returned, yield_accrued)`.
4. Yield is routed according to `yield_recipient`:
   - `Payer` — yield added to payer's refund
   - `Freelancer` — yield added to freelancer's payout (on `approve`) or sent directly (on `cancel`/`expire`)

The yield protocol interface:

```rust
trait YieldProtocol {
    fn deposit(env: &Env, amount: &i128);
    fn withdraw(env: &Env, requested: &i128) -> (i128, i128); // (principal, yield)
}
```

---

## 10. Security Assumptions

### 10.1 Authentication

- All state-changing operations require the authenticated signature of the relevant party (`payer`, `freelancer`, or `admin`).
- Soroban's `Address::require_auth()` is used throughout; no operation can be spoofed.

### 10.2 Trust Model

- **Payer and freelancer do not need to trust each other** for fund safety. The protocol enforces the rules automatically.
- **Admin is trusted** for pause/unpause and fee configuration. Compromising the admin key allows pausing the contract and changing fee parameters for new escrows but cannot steal locked funds from existing escrows.
- **Fee collector is trusted** only to receive fees; it has no contract permissions.
- **Yield protocol is trusted** for the full locked amount. Use only audited, well-known yield protocols.

### 10.3 Cancel Race Condition

`cancel()` is only permitted while `status = Active` (before `submit_work()`). Once the freelancer submits work, the payer cannot unilaterally cancel — they must `approve()`. This prevents payers from cancelling after accepting work off-chain.

### 10.4 No Dispute Mechanism

The current protocol version has no on-chain dispute resolution. Off-chain mediation and the `transfer_freelancer` role-transfer function are the available escape hatches. A future version may add a neutral arbitrator role.

### 10.5 Deadline

Deadlines are expressed as ledger timestamps (`u64` Unix seconds). Soroban's `env.ledger().timestamp()` is used for comparison. Deadline enforcement is approximate to ledger close time (~5–6 seconds on Stellar mainnet).

### 10.6 Single Escrow Per Contract

Each deployed contract instance manages exactly one escrow. Deploy a separate contract instance for each engagement.

### 10.7 Token Safety

Only SEP-41 compliant tokens should be used. Using non-standard tokens may result in unexpected transfer behaviour. Validate the token address before creating an escrow.

### 10.8 Integer Overflow

Soroban compiles with `overflow-checks = true` in the release profile (see root `Cargo.toml`). Arithmetic overflows will panic rather than wrap silently.
