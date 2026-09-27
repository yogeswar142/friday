# Engineering & Implementation Guidelines (`gemini.md`)

This document outlines the strict engineering standards, architectural rules, code quality guidelines, and security requirements for **FRIDAY**.

---

## 1. High Performance & Low Latency Core

1. **Zero unnecessary allocations on hot paths**: High-frequency mouse movement and keyboard input events must be allocated using value types, stack storage, or pre-allocated ring buffers.
2. **Compact binary serialization**: Use fast, zero-copy or minimal-overhead binary formats (e.g. custom packed binary structs, `bincode`, or `zerocopy`) rather than JSON or verbose text formats on real-time channels.
3. **Data/Control plane separation**: The input routing engine and network packet pipeline must remain entirely decoupled from any UI/frontend components. Control plane events (pairing, configuration) must never block or delay data plane input events.
4. **Lockless & Non-blocking I/O**: Use lock-free channels (such as `tokio::sync::mpsc`, `crossbeam-channel`, or atomics) for input routing. Never perform blocking system calls or thread locks on the main event loops.

---

## 2. Production-Grade Code Quality

1. **Strict Error Handling**:
   - Zero `panic!`, `unwrap()`, or `expect()` in production input or network paths.
   - All errors must be explicitly typed using `thiserror` or custom domain error enums and properly logged/handled.
2. **Platform Abstraction**:
   - Implement clean backend traits (`InputBackend`, `ScreenBackend`, `CursorBackend`).
   - Business logic, routing engines, coordinate normalization, and network protocol code must be 100% platform-agnostic. Platform-specific APIs (`x11`, `wayland`, `win32`, `cocoa`) must be strictly encapsulated inside platform module implementations.
3. **Safety & Memory Hygiene**:
   - Limit `unsafe` Rust blocks exclusively to native OS hardware/API interactions where mandatory. Every `unsafe` block must include a clear `// SAFETY:` rationale.
4. **Clean Code & Maintainability**:
   - Keep functions focused and modular.
   - Comprehensive unit and simulation testing for core components.

---

## 3. Security Guidelines

1. **Authenticated & Encrypted Transport**:
   - All cross-device transport (real-time input, control, bulk clipboard/files) must use modern authenticated encryption (TLS 1.3 / QUIC / Noise protocol).
2. **Input Isolation & Emergency Hotkeys**:
   - Local input override mechanisms must run asynchronously and un-bypassably to restore local machine control instantly if connection fails or misbehaves.
3. **File Transfer & Path Safety**:
   - Strictly sanitize received filenames. Prevent directory traversal attacks (`../`), arbitrary overwrites, and unsafe execution bits.
4. **Clipboard Security**:
   - Enforce origin tracking to prevent infinite clipboard loops.
   - Require user authorization/settings for clipboard and file sharing.

---

## 4. Repository & Maintenance Workflow

1. **Atomic Commits**: Push incremental, verified, tested code changes for each feature or vertical slice.
2. **Continuous Verification**: Ensure formatting (`cargo fmt`), linting (`cargo clippy`), and tests (`cargo test`) pass before committing.
3. **Compatibility Tracking**: Update the compatibility matrix (`COMPATIBILITY.md`) based on actual tested features per platform target.
