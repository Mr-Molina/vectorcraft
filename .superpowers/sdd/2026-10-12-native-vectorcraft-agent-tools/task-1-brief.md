# Task 1 Brief: Rust Toolchain Bootstrapping & Environment Setup

## Requirements
1. Install Rustup using Windows Package Manager (`winget`):
   ```powershell
   winget install Rustlang.Rustup -h --accept-source-agreements --accept-package-agreements
   ```
2. Configure environment PATH to include `$HOME\.cargo\bin`.
3. Verify `cargo.exe` and `rustc.exe` are operational:
   ```powershell
   cargo --version
   rustc --version
   ```
4. Verify VectorCraft workspace metadata parses without error:
   ```powershell
   cargo metadata --no-deps --format-version 1
   ```

## Constraints
- Windows PowerShell environment.
- Do not modify existing git repositories or source files during this task.
- Must ensure that any subsequent process or subagent running in this workspace has access to `cargo` and `rustc`.

## Report Contract
Write report to `s:\Github\vectorcraft\.superpowers\sdd\2026-10-12-native-vectorcraft-agent-tools\task-1-report.md`.
Return: Status (DONE / BLOCKED), one-line test summary, and any concerns.
