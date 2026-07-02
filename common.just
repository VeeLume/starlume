# common.just — shared VeeLume recipes (template-owned; `copier update` keeps this in sync).
# Imported by the repo's justfile. Add repo-local recipes in justfile, not here.

dev:
    pnpm tauri dev

build:
    pnpm tauri build

test:
    cargo test --workspace

fmt:
    cargo fmt --all

clippy:
    cargo clippy --workspace --all-targets -- -D warnings

# What CI runs.
check:
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets -- -D warnings
    pnpm check

clean:
    cargo clean
    rm -rf build .svelte-kit node_modules
