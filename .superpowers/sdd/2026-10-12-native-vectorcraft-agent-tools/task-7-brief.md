# Task 7 Brief: Full Quality Gates, Binary Build & End-to-End Smoke Test

## Objective
Execute complete quality gates (`cargo xtask layers`, `cargo clippy`, full tests across `vectorcraft-color`, `vectorcraft-engine`, `vectorcraft-mcp`), compile release binaries (`vectorcraft.exe` and `vectorcraft-cli.exe`), and perform end-to-end smoke verification of the new agent tools.

## Requirements
1. **Layer Checking**:
   - Run `cargo xtask layers` to ensure zero layering violations across all 25 crates.
2. **Clippy Quality Gate**:
   - Run `cargo clippy -p vectorcraft-color -p vectorcraft-engine -p vectorcraft-mcp --all-targets -- -D warnings` and verify zero warnings.
3. **Automated Test Suites**:
   - Run `cargo test -p vectorcraft-color`
   - Run `cargo test -p vectorcraft-engine --test semantic_summary_test --test color_cluster_test --test render_overlay_test`
   - Run `cargo test -p vectorcraft-mcp`
4. **Release Compilation**:
   - Run `cargo build --release -p vectorcraft -p vectorcraft-cli`
   - Verify `target/release/vectorcraft.exe` and `target/release/vectorcraft-cli.exe` are generated.
5. **Live Verification**:
   - Run `target/release/vectorcraft-cli.exe mcp --help` or run an engine test command with the CLI binary to verify runtime viability.
6. **Report**:
   - Write full output and verification summary to `file:///s:/Github/vectorcraft/.superpowers/sdd/2026-10-12-native-vectorcraft-agent-tools/task-7-report.md`.
