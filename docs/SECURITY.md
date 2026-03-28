# Security & Threat Model

This document describes the trust model, known attack vectors and their mitigations, and the explicit out-of-scope threats for the StarEscrow protocol.

For background on Soroban's security primitives see the [Soroban Security documentation](https://developers.stellar.org/docs/smart-contracts/security).

---

## 1. Trusted Parties and Their Capabilities

| Party | Trust Level | What they can do |
|-------|-------------|-----------------|
| **Admin** | Partially trusted | Initialise protocol config; pause/unpause the contract; update fee settings. Cannot move escrowed funds. |
| **Payer** | Trusted for their own escrow | Create an escrow; approve work; cancel before submission; reclaim after deadline. Cannot act on another party's escrow. |
| **Freelancer** | Trusted for their own escrow | Submit work; transfer their role to another address. Cannot approve or cancel. |
| **Fee Collector** | Untrusted (receives funds only) | Receives the protocol fee on approval. Has no contract permissions. |
| **Yield Protocol** | Partially trusted (external) | Receives deposited principal; must return at least the principal on withdrawal. A malicious or buggy yield protocol could lose or withhold funds. |
| **Stellar Validators** | Trusted (network-level) | Determine ledger close time used for deadline evaluation. |

---

## 2. Attack Vectors and Mitigations

### 2.1 Unauthorised Fund Release
**Threat:** A third party calls `approve()` to release funds to the freelancer without the payer's consent.

**Mitigation:** `approve()` calls `data.payer.require_auth()`. Soroban enforces that the transaction must be signed by the payer's key; any other caller is rejected at the host level.

---

### 2.2 Unauthorised Cancellation
**Threat:** The freelancer or a third party calls `cancel()` to refund the payer and abort the engagement.

**Mitigation:** `cancel()` calls `data.payer.require_auth()`. Only the payer can cancel, and only while status is `Active`.

---

### 2.3 Premature Expiry
**Threat:** The payer calls `expire()` before the deadline to reclaim funds while the freelancer is still working.

**Mitigation:** `expire()` checks `env.ledger().timestamp() > deadline`. If the deadline has not passed the call returns `EscrowError::DeadlineNotPassed`. Ledger timestamps are set by the validator network and cannot be manipulated by a single party.

---

### 2.4 Double-Create / State Overwrite
**Threat:** A second `create()` call overwrites an existing escrow, potentially stealing locked funds.

**Mitigation:** `create()` checks `storage::has_escrow()` and returns `EscrowError::AlreadyExists` if an escrow already exists for this contract instance.

---

### 2.5 Zero or Negative Amount
**Threat:** Creating an escrow with `amount <= 0` to bypass token transfer logic or cause accounting errors.

**Mitigation:** `create()` explicitly checks `amount <= 0` and returns `EscrowError::InvalidAmount`.

---

### 2.6 Disallowed Token
**Threat:** A payer creates an escrow with a malicious or worthless token contract.

**Mitigation:** When an allowlist is configured, `create()` checks the token against `read_allowed_tokens()` and rejects unlisted tokens with `EscrowError::TokenNotAllowed`. Deployments without an allowlist accept any SEP-41 token — operators should configure one for production use.

---

### 2.7 Admin Abuse (Pause Griefing)
**Threat:** A compromised or malicious admin pauses the contract indefinitely, locking funds.

**Mitigation:** Pausing blocks new state changes but does not move funds. Funds remain in the contract and can be released once the contract is unpaused. Operators should use a multisig or governance-controlled admin address to reduce single-key risk.

---

### 2.8 Yield Protocol Rug / Loss
**Threat:** The external yield protocol loses, withholds, or steals the deposited principal.

**Mitigation:** Yield integration is opt-in. If no `yield_protocol` is provided at creation, funds stay in the escrow contract. Operators should only integrate audited, trusted yield protocols. The contract does not verify the yield protocol's return value beyond what the token transfer enforces.

---

### 2.9 Freelancer Role Hijack
**Threat:** An attacker calls `transfer_freelancer()` to redirect payment to their own address.

**Mitigation:** `transfer_freelancer()` calls `data.freelancer.require_auth()`. Only the current freelancer can transfer the role.

---

### 2.10 Re-entrancy
**Threat:** A malicious token or yield contract re-enters the escrow contract during a transfer to manipulate state.

**Mitigation:** Soroban's execution model does not support re-entrancy within a single transaction — cross-contract calls are synchronous and the host prevents recursive invocation of the same contract instance. Status is updated after all transfers complete.

---

## 3. Out-of-Scope Threats

The following are explicitly **not** protected against by the contract:

- **Off-chain disputes** — The contract has no mechanism to evaluate whether the freelancer's work actually meets the milestone description. Dispute resolution is a planned future feature (see [ROADMAP.md](ROADMAP.md)).
- **Payer refusing to approve** — If the payer ignores a legitimate work submission and no deadline is set, funds can be locked indefinitely. Always set a deadline for time-sensitive engagements.
- **Stellar network-level attacks** — Eclipse attacks, validator collusion, or Stellar protocol bugs are outside the scope of this contract.
- **Key compromise** — If the payer's or admin's private key is stolen, an attacker can act as that party. Key management is the operator's responsibility.
- **Token contract bugs** — The escrow contract trusts the token contract to behave correctly per SEP-41. A malicious token could emit false transfer events or fail silently.
- **Front-running** — Stellar's transaction ordering is determined by validators. Sophisticated front-running of `approve()` or `cancel()` calls is theoretically possible but has no meaningful exploit path given the bilateral trust model.

---

## 4. References

- [Soroban Security — Authorization](https://developers.stellar.org/docs/smart-contracts/security/authorization)
- [Soroban Security — Auditing Smart Contracts](https://developers.stellar.org/docs/smart-contracts/security/auditing)
- [SEP-41 Token Interface](https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0041.md)
- [Stellar Consensus Protocol](https://developers.stellar.org/docs/learn/fundamentals/stellar-consensus-protocol)
