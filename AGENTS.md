# rust-derive-counters

Rust derive that generates counter methods for numeric and atomic struct fields.

- Every release bumps `version` in `Cargo.toml` and gets a matching `v<version>` tag.
- Dependencies are only `proc-macro2`, `quote` and `syn`.
- Log methods live in `derive-selflog`, not here.
- Every function, struct and enum has a one-line `///` doc comment, generated methods included.
- `just lint` passes before every commit.
