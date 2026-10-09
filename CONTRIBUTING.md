# Contributing

[README.md](README.md) is the contract: the API table, the error strings and the deploy steps. A change that breaks a documented command fixes the command or the README in the same commit.

- You need Rust 1.85 or newer. `rust-toolchain.toml` pins the exact toolchain CI uses.
- `make test` and `make lint` pass before you push. `make lint` is `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`.
- One concern per commit, with a [conventional](https://www.conventionalcommits.org) subject: `feat:`, `fix(helm):`, `docs:`, `ci:`.
- Image tags stay plain integers: `1`, `2`. Do not add `v` prefixes or semver to image tags.
- The chart version in `deploy/helm/arith-rust/Chart.yaml` is semver. Bump it in the commit that changes a template or a default; CI will not publish a version twice.
- A Helm value and a Kustomize component come together: add a toggle to one, add it to the other.

Open an issue or a pull request in plain words: what you ran, what you expected, what happened.
