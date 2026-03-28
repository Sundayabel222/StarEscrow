# StarEscrow Contract Benchmarks

Resource consumption measurements for each contract function using the Soroban test environment budget API.

## How to Run

```bash
cargo test -p escrow bench -- --nocapture
```

## Results

> Last measured: _run `cargo test -p escrow bench -- --nocapture` to update_

| Function       | CPU Instructions | Memory (bytes) |
|----------------|-----------------|----------------|
| `create`       | TBD             | TBD            |
| `submit_work`  | TBD             | TBD            |
| `approve`      | TBD             | TBD            |
| `cancel`       | TBD             | TBD            |
| `expire`       | TBD             | TBD            |
| `get_status`   | TBD             | TBD            |

## Regression Threshold

CI will fail if any function exceeds the following limits:

| Function       | Max CPU Instructions | Max Memory (bytes) |
|----------------|---------------------|--------------------|
| `create`       | 150,000,000         | 5,000,000          |
| `submit_work`  | 100,000,000         | 3,000,000          |
| `approve`      | 150,000,000         | 5,000,000          |
| `cancel`       | 150,000,000         | 5,000,000          |
| `expire`       | 150,000,000         | 5,000,000          |
| `get_status`   | 50,000,000          | 1,000,000          |

These thresholds are conservative starting points. Tighten them after establishing a baseline.

## Notes

- Measurements use `soroban_sdk::testutils::budget::Budget` in the test environment.
- CPU instructions and memory are reset before each function call so only that function's cost is captured.
- Yield protocol interactions are not included in the base benchmarks above.
