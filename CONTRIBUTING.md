# Contributing

Code style and commit conventions are in [AGENTS.md](AGENTS.md) (the same as CLAUDE.md). Read [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) before you start.

## Building from source

macOS needs the Xcode Command Line Tools; Windows needs the Visual Studio 2022 C++ Build Tools and the Windows SDK. The Rust version is pinned in `rust-toolchain.toml`, and rustup picks it up automatically.

```sh
cargo build --release --locked
./target/release/quill                          # uses ~/Documents/Quill
QUILL_HOME=/tmp/lib ./target/release/quill   # uses another library
```

While developing, point `QUILL_HOME` at a scratch directory; don't test against your real library.

The repository is a Cargo workspace: `crates/core` is the document engine (`fdl_core`), which does not depend on GPUI, and the root crate is the app. GPUI, taffy and xattr are locally patched copies in `vendor/`; the changes are listed in [vendor/PATCHES.md](vendor/PATCHES.md). `target/debug` can grow past 10 GB; if you only build release, `cargo clean --profile dev` clears it.

## Testing

```sh
scripts/check.sh                                        # tests + clippy with zero warnings; run before committing
cargo test --release -p fdl_core                        # the document engine only, without building GPUI
FDL_FUZZ_SEEDS=2000 cargo test --release random_edits   # more random edits, for the editing core
```

- The editing core has a random test: starting from a document with every block type, it makes random selections, typing, structural, style, table and undo edits, checks the model's invariants after each step, and saves and reloads every 25 steps.
- The document format has a regression test: `crates/core/tests/fixtures/all-blocks.xml` must read and write back byte for byte.
- UI screenshots and scripted runs are covered in [tools/harness/README.md](tools/harness/README.md).

To run the checks on every commit: `git config core.hooksPath scripts/hooks`.

## Documentation

| Document | Contents |
|---|---|
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | Layers, the editing flow, invariants, undo, rendering, the module map |
| [docs/FORMAT.md](docs/FORMAT.md) | Library layout, index recovery, DocxXML read/write rules and local extensions |
| [docs/THEMES.md](docs/THEMES.md) | Writing document themes in CSS: selectors, properties, named colors |
| [AGENTS.md](AGENTS.md) | Code style, pre-commit checks, testing conventions |

Most of these, AGENTS.md included, are written in Chinese for now.

## Building release packages

Before a release, `scripts/check.sh` must pass and CI must be green on that commit. After packaging, verify the signatures, notarization (macOS) and the SHA-256 of the downloads. The Windows CI build does not replace installing, uninstalling and editing on a real machine.

The declared macOS minimum is 13, not yet verified on real hardware. Intel packages can be cross-compiled on Apple Silicon; real Intel hardware validation is still needed.

### macOS

`scripts/release.sh` builds for the local architecture, generates the app icon, signs with a Developer ID Application certificate, submits for Apple notarization, staples the ticket and runs a Gatekeeper check, then writes a ZIP and its SHA-256. It does not upload to GitHub or launch the app.

Install the signing certificate, with its private key, in the login keychain and set up a notarytool keychain profile, then run:

```sh
SIGNING_IDENTITY="Developer ID Application: …" NOTARY_PROFILE="quill" scripts/release.sh
```

The script stops if signing or notarization fails; whatever is left in the failed run's directory must not be shipped.

To build both architectures sequentially on one Apple Silicon Mac, install the Intel Rust target once, then run the same signing and notarization flow for each architecture:

```sh
rustup target add x86_64-apple-darwin
export SIGNING_IDENTITY="Developer ID Application: …" NOTARY_PROFILE="quill"
scripts/release.sh
MACOS_TARGET=x86_64-apple-darwin scripts/release.sh
```

Each run keeps its output in a separate `dist-release.*` directory. Intel ZIPs use `macos-x86_64`; Apple Silicon ZIPs keep `macos-arm64`. CI builds both on its existing ARM runner and verifies the binary architectures and signatures. `SIGNING_IDENTITY=-` skips notarization and creates ad-hoc signed `-unsigned.zip` validation packages, which must not be distributed.

Never commit credentials, private keys or certificate files.

For a new version, bump the version in both `Cargo.toml` files together, and write the version's highlights in `src/changelog.rs`: existing users see them once at the bottom of the sidebar.
