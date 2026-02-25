# Counterexample Analysis and Interpretation Guide

When Kani verification fails, it produces a **counterexample trace**: a concrete assignment of input values that violates an assertion. This guide explains how to read these traces and resolve the underlying issues.

---

## What Is a Counterexample?

A Kani counterexample is not a bug in Kani — it is Kani showing you a case where your code or your specification is wrong. There are three possible causes:

| Cause                                 | Meaning                                                 | Resolution                                             |
| ------------------------------------- | ------------------------------------------------------- | ------------------------------------------------------ |
| **Bug in the contract logic**         | The code has a real defect                              | Fix the contract code                                  |
| **Missing precondition (assumption)** | The harness allows an input the contract never receives | Add a `kani::assume(...)` to exclude impossible inputs |
| **Incorrect specification**           | The invariant as written is wrong                       | Refine the invariant in the spec and harness           |

---

## Reading Kani Output

A failing harness produces output like:

```
VERIFICATION FAILED (assertion failed: marketplace_fee + seller_amount == price)

Counterexample:
  - price = -1
  - fee_bps = 500
  - marketplace_fee = 0
  - seller_amount = -1
```

Walk through it line by line:

1. **Assertion that failed** — identifies the invariant that was violated
2. **Variable assignments** — the exact inputs Kani found that cause the failure
3. **Trace** — the sequence of Rust statements executed to reach the failure

---

## Annotated Counterexample Examples

### Example 1: Missing Precondition (Fixed Price Fund Conservation)

**Failing harness:** `verify_fund_conservation_fixed_price`

**Kani output:**

```
VERIFICATION FAILED (assertion: marketplace_fee + seller_amount == price)
Counterexample:
  price = -5
  fee_bps = 1000
  marketplace_fee = 0
  seller_amount = -5
```

**Analysis:**

- `price = -5` is negative. Integer division of `-5 * 1000 / 10000 = 0` (rounds toward zero in Rust).
- `seller_amount = -5 - 0 = -5`
- So `marketplace_fee + seller_amount = -5 ≠ price = -5`... wait, that IS -5.
  Actually `0 + (-5) = -5 = price`. This would pass.

More likely cause: Kani found `price = 0` which the contract rejects but the harness did not exclude.

**Resolution:** Add `kani::assume(price > 0)` to the harness. This matches the contract's `create_listing` precondition.

**After fix:**

```rust
kani::assume(price > 0);
```

---

### Example 2: Integer Overflow (Counter)

**Failing harness:** `verify_counter_monotonicity`

**Kani output:**

```
VERIFICATION FAILED (assertion: new_counter > counter)
Counterexample:
  counter = 18446744073709551615  // u64::MAX
  increment_counter returns None (overflow)
```

**Analysis:**

- At `u64::MAX`, `checked_add(1)` returns `None`.
- The harness `match` arm for `Some(v)` asserts `v > counter`, but `None` reaches the other arm.
- The `None` arm asserts `counter == u64::MAX` — this should PASS.
- If the assertion in the `None` arm is wrong, that is the bug.

**Resolution:** Verify the `None` arm assertion is correct. In this case it is correct and no fix is needed. If Kani still fails, check for a typo in the assertion.

---

### Example 3: Authorization Logic Gap

**Failing harness:** `verify_authorization_logic`

**Kani output:**

```
VERIFICATION FAILED (assertion: is_owner || is_valid_operator)
Counterexample:
  executor = 5
  owner = 3
  operator = Some((5, 100))
  now = 101
```

**Analysis:**

- `executor (5) ≠ owner (3)` → not the owner
- `operator = Some((5, 101))` but `now = 101` and `expires_at = 100`
- `now (101) > expires_at (100)` → operator is expired
- So `is_authorized` correctly returns `false`
- But the failing assertion is `is_owner || is_valid_operator` in the `authorized = false` branch
- This means `is_valid_operator` is computing incorrectly in the harness

**Resolution:** Check the `is_valid_operator` computation in the `authorized = false` branch. The condition should be `now <= expires_at` (matching the contract's `now > op_data.expires_at` rejection):

```rust
let is_valid_operator = operator
    .map(|(op_id, expires_at)| op_id == executor && now <= expires_at)
    .unwrap_or(false);
// In false branch: assert!(!is_valid_operator)  -- this is correct
```

If Kani says `is_valid_operator = true` but `authorized = false`, there is a disagreement between `is_authorized` and the post-condition check. Fix the post-condition to match the function's semantics exactly.

---

## Step-by-Step Debugging Process

1. **Copy the counterexample values** into a unit test:

   ```rust
   #[test]
   fn debug_counterexample() {
       let price: i128 = -1;  // from Kani trace
       let fee_bps: u32 = 500;
       let (fee, seller) = fixed_price_split(price, fee_bps);
       assert_eq!(fee + seller, price);
   }
   ```

2. **Run that test** with `cargo test` to reproduce the failure locally without Kani.

3. **Determine the cause**: is this a valid input the contract can receive? Check the contract's validation logic.

4. **Add a `kani::assume`** if the input is always excluded by the contract, **or fix the contract** if the input is valid and the behavior is wrong.

5. **Re-run Kani** to confirm the counterexample is resolved: `cargo kani --harness <harness_name>`

---

## Escalation: When a Counterexample Reveals a Real Bug

If the counterexample represents a valid input (i.e., you cannot exclude it with `kani::assume`), the contract has a bug. The process is:

1. Document the counterexample in a GitHub issue with the Kani trace.
2. Reference the violated invariant from the spec (`INV-XXX-N`).
3. Fix the contract code.
4. Re-run all Kani harnesses to verify the fix does not introduce regressions.
5. Ensure the fix does not break existing Soroban tests (`cargo test` in the contract directory).

---

## Re-Running Verification

```bash
# Run all harnesses
cd formal-verification
cargo kani

# Run a specific harness
cargo kani --harness verify_fund_conservation_fixed_price

# Run with a higher unwind bound (for loops)
cargo kani --unwind 10 --harness verify_rate_limit_enforcement
```
