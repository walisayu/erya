# Building Erya

Run commands at the repository root containing `Cargo.toml`. In the local workspace this is `Source/`; `Website/` is a separate sibling directory.

## Development and verification

Rust 1.85 or later and the platform's native linker are required. No Docker or external service is needed.

```bash
cargo run --release -- notes.md
cargo fmt --all -- --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all-targets --locked --no-fail-fast
rustup run 1.85.0 cargo check --all-targets --locked
```

The integration tests launch the editor in a real PTY, send keyboard input, resize the terminal, check saved files, and verify terminal restoration. They use temporary files and do not need accounts or network access.

## Release toolchain

`build-release.sh` checks these versions before building:

| Tool | Version |
|---|---|
| Rust | 1.97.1 |
| Minimum-version compiler | 1.85.0 |
| Zig | 0.15.2 |
| cargo-zigbuild | 0.23.0 |
| cargo-xwin | 0.23.1 |
| Microsoft CRT/SDK channel | 17 |

The seven-platform builder runs on Apple Silicon macOS with Apple Command Line Tools. Install the Rust target libraries for all targets below and put the cross-build tools on `PATH`. Microsoft SDK downloads are subject to their applicable license terms.

```bash
./build-release.sh
```

| Target | Compatibility |
|---|---|
| aarch64-apple-darwin | macOS 11.0+ |
| x86_64-apple-darwin | macOS 10.15+ |
| x86_64-unknown-linux-gnu | glibc 2.17+ |
| aarch64-unknown-linux-gnu | glibc 2.17+ |
| armv7-unknown-linux-gnueabihf | glibc 2.17+, hard-float |
| x86_64-pc-windows-msvc | Windows x64 |
| aarch64-pc-windows-msvc | Windows ARM64 |

Archives, individual `.sha256` files, and `SHA256SUMS` are generated in `dist/`, which is excluded from Git. The builder remaps local source and home-directory paths in compiled diagnostics. Cross-platform file-format checks do not replace runtime testing on each target.

For a single installed target, run `./publish.sh <target>`. Set `ERYA_BUILD_TOOL=zigbuild` for Linux or `ERYA_BUILD_TOOL=xwin` for Windows. The default uses native Cargo.

User manuals are maintained in `docs/manual/`; see its README for regeneration commands.
