# Contributing to StarEscrow

Thanks for your interest in contributing! This guide covers everything you need to get a PR merged.

## Getting Started

1. Fork the repository and clone your fork:
   ```bash
   git clone https://github.com/<your-username>/StarEscrow.git
   cd StarEscrow
   ```
2. Install prerequisites: Rust (stable + `wasm32-unknown-unknown` target) and the [Stellar CLI](https://developers.stellar.org/docs/tools/developer-tools/cli/stellar-cli).

## Branch Naming

| Type | Pattern | Example |
|------|---------|---------|
| Feature | `feat/<short-description>` | `feat/dispute-resolution` |
| Bug fix | `fix/<short-description>` | `fix/deadline-overflow` |
| Docs | `docs/<short-description>` | `docs/update-readme` |
| Chore | `chore/<short-description>` | `chore/bump-deps` |

## Workflow

```bash
git checkout -b feat/your-feature
# make changes
git commit -m "feat: describe your change"
git push origin feat/your-feature
```

Then open a Pull Request against `main`.

## Code Style

Run these before pushing — CI enforces formatting (`cargo fmt --check --workspace`) and clippy:

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
```

### Formatting Configuration

The [`rustfmt.toml`](../rustfmt.toml) file at the repository root defines the project's formatting standards, including:
- Edition 2021
- 100-character line width
- Module-level import granularity
- Consistent trailing commas

To format your code:
```bash
cargo fmt --all
```

To check formatting without making changes:
```bash
cargo fmt --all -- --check
```

## PR Checklist

Before requesting review, confirm:

- [ ] `cargo fmt --all` passes with no changes
- [ ] `cargo clippy --all-targets -- -D warnings` passes
- [ ] `cargo test -p escrow` passes
- [ ] New behaviour is covered by tests
- [ ] Relevant docs updated (if applicable)

## Finding Something to Work On

Browse [open issues](../../issues) — issues tagged **`good first issue`** are a great starting point for first-time contributors, including those joining via OnlyDust or hackathons.
