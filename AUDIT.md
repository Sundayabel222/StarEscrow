# StarEscrow — Self-Audit Checklist & Report

> **Date:** 2026-03-29
> **Auditor:** Self-audit (pre-formal-audit preparation)
> **Scope:** `contracts/escrow` and `contracts/factory` (Soroban / Stellar)
> **Soroban SDK:** soroban-sdk (see `Cargo.toml`)

---

## Legend

| Symbol | Meaning |
|--------|---------|
| ✅ PASS | No issue found |
| ❌ FAIL | Issue confirmed |
| ⚠️ WARN | Concern or improvement recommended |
| N/A | Not applicable to this contract |

---

## 1. Authorization Checks

Ensures every state-changing entry point enforces the correct caller identity via `require_auth()`.

| # | Function | Check | Status | Notes |
|---|----------|-------|--------|-------|
| 1.1 | `init` | Admin must authorise initialization | ✅ PASS | `admin.require_auth()` called before writing config |
| 1.2 | `pause` | Only admin may pause | ✅ PASS | `config.admin.require_auth()` enforced |
| 1.3 | `unpause` | Only admin may unpause | ✅ PASS | `config.admin.require_auth()` enforced |
| 1.4 | `create` | Only payer authorises fund lock | ✅ PASS | `payer.require_auth()` called before token transfer |
| 1.5 | `submit_work` | Only freelancer may submit | ✅ PASS | `data.freelancer.require_auth()` enforced |
| 1.6 | `approve` | Only payer may approve milestone | ✅ PASS | `data.payer.require_auth()` enforced |
| 1.7 | `release_recurring` | No auth required (permissionless) | ⚠️ WARN | Intentional by design ("callable by anyone once per interval") but widens attack surface. A malicious caller could trigger a release exactly at interval boundaries without payer/freelancer consent. Consider restricting to payer or freelancer. |
| 1.8 | `cancel` | Only payer may cancel | ✅ PASS | `data.payer.require_auth()` enforced |
| 1.9 | `expire` | Only payer may claim expired funds | ✅ PASS | `data.payer.require_auth()` enforced after deadline check |
| 1.10 | `transfer_freelancer` | Only current freelancer may transfer role | ✅ PASS | `data.freelancer.require_auth()` enforced |
| 1.11 | `transfer_payer` | Only current payer may transfer role | ✅ PASS | `data.payer.require_auth()` enforced |
| 1.12 | `extend_deadline` | Only payer may extend deadline | ✅ PASS | `data.payer.require_auth()` enforced |
| 1.13 | Factory `submit_work` | Only freelancer may submit | ✅ PASS | `record.freelancer.require_auth()` enforced |
| 1.14 | Factory `approve` | Only payer may approve | ✅ PASS | `record.payer.require_auth()` enforced |
| 1.15 | Factory `cancel` | Only payer may cancel | ✅ PASS | `record.payer.require_auth()` enforced |

---

## 2. Integer Overflow & Arithmetic Safety

Rust's debug builds panic on overflow; release builds (`opt-level ≥ 1`, `#![no_std]`) wrap silently unless `checked_*` / `saturating_*` arithmetic is used explicitly.

| # | Location | Check | Status | Notes |
|---|----------|-------|--------|-------|
| 2.1 | `create` — milestone accumulation | `total_amount += m.amount` | ⚠️ WARN | Unchecked addition of `i128` values. Overflow requires astronomically large amounts (near `i128::MAX ≈ 1.7×10³⁸`), but `checked_add` is best practice. |
| 2.2 | `approve` — fee calculation | `milestone.amount * fee_bps / 10_000` | ⚠️ WARN | Multiplication before division. If `milestone.amount` is very large, intermediate product could overflow. Use `checked_mul`. |
| 2.3 | `release_recurring` — fee calculation | `data.amount * fee_bps / 10_000` | ❌ FAIL | `data.amount` does not exist on `EscrowData` (field is `total_amount`). This is a **compile-time error** — see Known Issues §5.1. |
| 2.4 | `cancel` — remaining amount | `total_amount - released_amount + data.amount * releases_made` | ❌ FAIL | Same undefined `data.amount` field. Also, subtraction could underflow if `released_amount > total_amount` due to rounding (unlikely but unguarded). See §5.1. |
| 2.5 | `expire` — remaining amount | Same expression as `cancel` | ❌ FAIL | Same issue as 2.4. |
| 2.6 | `storage::check_and_update_rate_limit` | Window and count arithmetic | ✅ PASS | Uses `saturating_add` correctly. |
| 2.7 | Factory `next_id` | `id + 1` (u64) | ⚠️ WARN | Unchecked increment. Wraps to 0 after `u64::MAX` escrows — negligible in practice, but `checked_add` + panic is safer. |

---

## 3. Reentrancy

Soroban's host environment serialises contract execution within a single transaction, making classic EVM-style reentrancy impossible. However, ordering of state writes relative to cross-contract calls still matters for logical consistency.

| # | Function | Check | Status | Notes |
|---|----------|-------|--------|-------|
| 3.1 | `create` — token transfer | Transfer before state written | ✅ PASS | Token `transfer` is called, then `save_escrow`. If the transfer reverts, state is never written. |
| 3.2 | `create` — yield deposit | Yield `deposit` before `save_escrow` | ⚠️ WARN | If the yield protocol call panics after the token transfer has settled, `principal_deposited` is never recorded. Consider saving escrow state first, then depositing. |
| 3.3 | `approve` — dual transfer ordering | Fee collector transfer, then freelancer transfer, then state write | ⚠️ WARN | Both token transfers execute before `milestone.status = Approved` is written. In Soroban this cannot cause reentrancy, but a partial transfer failure could leave state inconsistent. Apply checks-effects-interactions pattern: update state before transfers. |
| 3.4 | `cancel` / `expire` — refund ordering | Refund transfer before status set to Cancelled/Expired | ⚠️ WARN | `client.transfer` executes before `data.status = Cancelled`. Safe in Soroban, but deviates from best practice. Update status first. |
| 3.5 | `withdraw_funds` helper | Yield withdrawal and token transfer | ✅ PASS | Yield is withdrawn before token transfer; principal amount sourced from yield response. No state inconsistency. |
| 3.6 | Soroban host reentrancy guard | Same contract cannot re-enter itself | ✅ PASS | Soroban prevents same-contract reentrancy at the host level. |

---

## 4. Storage Exhaustion

Soroban imposes per-entry size limits and charges rent (TTL-based fees) for persistent storage. Instance storage is shared across all keys in a single contract instance.

| # | Check | Status | Notes |
|---|-------|--------|-------|
| 4.1 | Milestone vector size | No upper bound on number of milestones | ❌ FAIL | An attacker or misbehaving client can create an escrow with thousands of milestones. Each milestone is a `String + i128 + enum`, serialised into instance storage. A large milestone array can exceed Soroban's per-entry size limit or inflate rent costs significantly. **Recommendation:** cap milestones at a reasonable maximum (e.g., 50). |
| 4.2 | Factory index growth | `PayerIndex` / `FreelancerIndex` grow unboundedly | ❌ FAIL | Each `create_escrow` appends to a per-address `Vec<u64>` stored in instance storage. A payer creating thousands of escrows will bloat a single storage entry, eventually hitting Soroban's XDR size limit. **Recommendation:** use `Persistent` storage keyed by `(address, page)` or cap index size. |
| 4.3 | Single-instance storage pattern | All escrow data in one instance | ⚠️ WARN | The escrow contract uses `DEFAULT_ESCROW_ID = 0`, meaning one contract instance holds exactly one escrow. This limits reuse but bounds per-instance storage. The factory pattern does not have this limit — see 4.2. |
| 4.4 | TTL extension on every write | `extend_ttl` called after each mutation | ✅ PASS | Escrow data is reliably renewed; no risk of silent expiry during active use. |
| 4.5 | No TTL extension in factory | Factory contract has no `extend_ttl` calls | ❌ FAIL | Factory escrow records and index entries will expire after the default ledger TTL. Long-lived escrows may become inaccessible. Add TTL extension on every write. |
| 4.6 | String milestone descriptions | Unbounded `String` in `Milestone.description` | ⚠️ WARN | No maximum length enforced on milestone description strings. A very long string consumes disproportionate storage rent. Enforce a character limit (e.g., 256 bytes). |
| 4.7 | Rate limiting config | `RateLimitConfig` / `PayerStats` structures present but not wired to main flow | ⚠️ WARN | Rate-limiting helpers exist in `storage.rs` but are marked `#[allow(dead_code)]` and unused in the main contract. Intended DoS protection is not active. |

---

## 5. Known Issues

### 5.1 — CRITICAL: Undefined `data.amount` Field in `release_recurring`, `cancel`, and `expire`

**Severity:** Critical (compile-time error / logic error)

**Affected functions:** `release_recurring` (line 268), `cancel` (line 316), `expire` (line 361)

**Description:**
`EscrowData` contains `total_amount: i128` and `recurrence_count: u32` but no `amount` field. The code references `data.amount` in three places:

```rust
// release_recurring — fee computation
let fee = data.amount * (config.fee_bps as i128) / 10000;

// cancel — remaining balance computation
let remaining = data.total_amount - released_amount
    + if data.recurrence_count > 0 { data.amount * data.releases_made as i128 } else { 0 };

// expire — same as cancel
let remaining = data.total_amount - released_amount
    + if data.recurrence_count > 0 { data.amount * data.releases_made as i128 } else { 0 };
```

The intended semantics are that in recurring mode, each release pays `total_amount / recurrence_count`. This per-release amount is never stored on `EscrowData`.

**Impact:** The contract will not compile as written. Recurring-mode fee collection and refund calculations are broken.

**Fix:** Add a `per_release_amount: i128` field to `EscrowData`, computed at `create` time as `total_amount / recurrence_count` (or stored separately), and replace `data.amount` references accordingly.

---

### 5.2 — HIGH: Type Mismatch in `approve` — Tuple Passed as `i128`

**Severity:** High (compile-time error)

**Affected function:** `approve` (lines 205–216)

**Description:**
The variable `freelancer_amount` is assigned a tuple `(i128, i128)`:

```rust
let freelancer_amount = if storage::has_config(&env) {
    ...
    (milestone.amount - fee, fee)   // (i128, i128)
} else {
    (milestone.amount, 0)           // (i128, {integer})
};
client.transfer(..., &freelancer_amount);  // expects &i128
```

`token::Client::transfer` expects `&i128` as the amount argument. Passing `&(i128, i128)` is a type error that prevents compilation.

**Fix:** Destructure the tuple before the transfer:

```rust
let (net_amount, _fee) = freelancer_amount;
client.transfer(&env.current_contract_address(), &data.freelancer, &net_amount);
```

---

### 5.3 — MEDIUM: `release_recurring` Is Permissionless

**Severity:** Medium

**Description:**
Any account can call `release_recurring` once the interval has elapsed, not just the payer or freelancer. While funds still go to the freelancer, this means third parties can force payment timing that may not align with payer intent.

**Recommendation:** Restrict callers to `data.payer` or `data.freelancer`.

---

### 5.4 — LOW: `cancel` and `expire` Refund Calculation May Be Incorrect

**Severity:** Low (pending fix of §5.1)

**Description:**
The refund expression `total_amount - released_amount + data.amount * releases_made` adds back already-released recurring amounts, which have already left the contract. This would result in attempting to refund more than the contract holds. The correct refund is:

```
remaining = total_amount - released_amount - (per_release_amount * releases_made)
```

---

### 5.5 — LOW: No Validation That `recurrence_count` Divides `total_amount` Evenly

**Severity:** Low

**Description:**
If `total_amount` is not evenly divisible by `recurrence_count`, truncation causes the final release to pay less than expected, and a dust amount remains locked in the contract with no recovery path.

**Recommendation:** Either require `total_amount % recurrence_count == 0` at create time, or credit the remainder to the final release.

---

## 6. Additional Security Observations

| # | Observation | Severity | Recommendation |
|---|-------------|----------|----------------|
| 6.1 | `init` can be called before `require_auth` if `has_config` returns false — the order is correct, but a frontrunning attacker could call `init` with their own admin before the legitimate deployer | Medium | Deploy and call `init` atomically in a single transaction, or use the deployer address as the implicit first admin. |
| 6.2 | No event emitted on `init` | Low | Emit an `initialized` event so off-chain indexers can track contract setup. |
| 6.3 | `expire` checks `timestamp <= deadline` (not `<`), meaning deadline is inclusive; a transaction at exactly the deadline timestamp cannot expire | Low | Use `< deadline` or document the inclusive behaviour explicitly. |
| 6.4 | `withdraw_funds` helper is defined but never called in the main flow | Low | Dead code inflates the WASM binary. Remove or integrate it. |
| 6.5 | Yield protocol address is caller-supplied and not validated against an allowlist | Medium | Validate `yield_protocol` against the same token allowlist or a separate yield protocol allowlist to prevent interaction with malicious yield contracts. |
| 6.6 | `EscrowData` can never be deleted; completed/cancelled escrows remain in storage indefinitely | Low | Add an explicit `close` function that removes the instance entry after final settlement to reclaim ledger space. |

---

## 7. Recommendations for Formal Audit

Before engaging a formal auditor, address the following in priority order:

1. **Fix compile errors** (§5.1 and §5.2) — the contract cannot be deployed as written. These must be resolved before any other testing or audit work.

2. **Add a `per_release_amount` field** to `EscrowData` and correct the `cancel`/`expire` refund logic (§5.4).

3. **Cap milestone array length** and enforce a `String` length limit to eliminate storage exhaustion vectors (§4.1, §4.6).

4. **Add TTL extension to the factory contract** to prevent escrow record expiry (§4.5).

5. **Activate or remove rate limiting** — the scaffolding exists in `storage.rs` but is not enforced (§4.7).

6. **Restrict `release_recurring`** to authorised callers (§5.3 / §1.7).

7. **Apply checks-effects-interactions pattern** throughout — update state before external calls (§3.3, §3.4).

8. **Fuzz testing** — property-based tests (`contracts/escrow/tests/prop_tests.rs`) exist; expand coverage for recurring-mode arithmetic edge cases and milestone boundary conditions.

9. **Formal verification** — consider using the Soroban test framework's `AuthorizationTracker` to assert exact auth trees for every entry point.

10. **Auditor focus areas:**
    - Cross-contract call safety (yield protocol and token interactions)
    - Economic invariants: total locked ≥ sum of pending milestone amounts at all times
    - Factory index unbounded growth under adversarial load
    - Correct TTL management across all storage tiers (`instance`, `persistent`, `temporary`)

---

*This self-audit was conducted against the source code as of commit `07bb284`. It is not a substitute for a professional security audit. Deploy to mainnet only after a formal third-party review.*
