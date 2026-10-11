# Contributing

[README.md](README.md) is the contract: the API table, the error strings and the deploy steps. A change that breaks a documented command fixes the command or the README in the same commit.

- You need Rust 1.85 or newer. `rust-toolchain.toml` pins the exact toolchain CI uses.
- `make test` and `make lint` pass before you push. `make lint` is `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`.
- One concern per commit, with a [conventional](https://www.conventionalcommits.org) subject: `feat:`, `fix(helm):`, `docs:`, `ci:`.
- One version per release, semver: `Cargo.toml`, `Chart.yaml` (`version`, `appVersion`, the `artifacthub.io/images` tag), `deploy/kustomize/base/kustomization.yaml` and the docs carry the same one, and CI checks it. A release is a commit that bumps all of them, then a tag `v<version>`; CI never publishes a version twice.
- Image tags are that version (`1.2.0`) or `sha-<commit>`. Never a bare integer, never `latest`. Build locally with `make image TAG=dev`.
- A Helm value and a Kustomize component come together: add a toggle to one, add it to the other.

Open an issue or a pull request in plain words: what you ran, what you expected, what happened.
