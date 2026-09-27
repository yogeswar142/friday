<div align="center">

# 🌟 FRIDAY
### Fast, Resilient Inter-Device Access Yield

**Ultra-low-latency, cross-platform mouse, keyboard, clipboard, and file sharing across Windows, Linux, and macOS.**

[![Build & Test](https://img.shields.io/badge/build-passing-brightgreen?style=for-the-badge&logo=rust)](https://github.com/yogeswar142/friday)
[![Rust](https://img.shields.io/badge/rust-1.80%2B-orange?style=for-the-badge&logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT%20%2F%20Apache--2.0-blue?style=for-the-badge)](LICENSE)
[![Latency](https://img.shields.io/badge/latency-%3C%201ms-purple?style=for-the-badge)](https://github.com/yogeswar142/friday)
[![Platforms](https://img.shields.io/badge/platforms-Linux%20%7C%20Windows%20%7C%20macOS-lightgrey?style=for-the-badge)](https://github.com/yogeswar142/friday)

<br/>

```text
       ONE MOUSE / KEYBOARD
                │
                ▼
           ┌──────────┐
           │  FRIDAY  │  (Zero-lag UDP protocol)
           └────┬─────┘
                │
    ┌───────────┼───────────┐
    ▼           ▼           ▼
┌───────┐   ┌───────┐   ┌───────┐
│ Linux │   │Windows│   │ macOS │
└───────┘   └───────┘   └───────┘
```

</div>

---

## 📖 Overview

**FRIDAY** is an open-source, hardware-level peripheral-sharing engine written in 100% safe, high-performance Rust. It allows a single physical mouse and keyboard connected to one computer to seamlessly navigate across neighboring screens — regardless of operating system, display scale, or resolution.

No external hardware switches, no cloud dependencies, and zero lag. FRIDAY establishes a lightweight peer-to-peer UDP link directly over local Wi-Fi or Ethernet.

---

## ✨ Key Features

- 🎯 **Exclusive Cursor Ownership** — Only ONE device owns the physical mouse at any moment. Zero input mirroring, zero jitter, and no phantom cursor leakage to inactive screens.
- ⏱️ **Edge Resistance & Dwell Timing** — Configurable dwell timer (default `500ms`) ensures casual gestures or working near scrollbars/window borders never trigger accidental handoffs.
- 📍 **Exact Position Continuity** — Seamlessly resumes from the exact pixel coordinates where each computer's cursor was last left.
- ⚡ **Sub-Millisecond Wire Protocol** — Compact 16-byte fixed-size binary packet framing minimizes network overhead and serialization latency.
- 🛡️ **Held-Input Safety Reset** — Guaranteed automatic release of pressed buttons or modifiers upon edge transitions or network disconnects.
- 🖥️ **Hardware Abstraction Layer (HAL)** — Native input capture and injection via:
  - **Linux X11**: `XQueryPointer` polling, `XTest` injection, `XFixes` cursor invisibility, and `XGrabPointer`.
  - **Windows**: Win32 HAL-level `SendInput` supporting virtual desktop coordinate spaces.
  - **Linux Wayland & macOS**: Extensible trait-based architecture ready for native backend integration.

---

## 🏗️ Architecture & Workspace

FRIDAY is built as a modular Rust workspace designed for maximum separation of concerns:

```text
friday/
├── crates/
│   ├── friday-core/       # Mathematical layout, normalized coords (0..65535), exclusive router
│   ├── friday-platform/   # Cross-platform traits & mock backends for input capture/injection
│   ├── friday-network/    # Ultra-low-latency UDP transport, packet encoding, & framing
│   ├── friday-agent/      # Native background daemon for capture, dwell detection, & injection
│   └── friday-cli/        # CLI utilities, benchmarks, and latency simulation tools
```

---

## 📊 Platform Compatibility Matrix

| OS Target | Input Capture | Input Injection | Status | Notes |
|:---|:---:|:---:|:---:|:---|
| **Linux (X11)** | ✅ | ✅ | **Supported & Tested** | Validated on Ubuntu 24.04 LTS (`XTest` + `XFixes`) |
| **Windows 10 / 11** | ✅ | ✅ | **Supported & Tested** | Validated on Windows 11 x64 (Win32 `SendInput`) |
| **Linux (Wayland)** | 🚧 | 🚧 | *In Development* | Targeted for Phase 4 via `wlroots` / virtual-input portals |
| **macOS (Intel/Apple Silicon)** | 🚧 | 🚧 | *In Development* | Targeted for Phase 4 via `CoreGraphics` / `IOHID` |

> [!NOTE]
> Architecture compatibility is not the same as claiming tested support. FRIDAY only marks platforms as **Supported** after physical, reproducible hardware validation.

---

## 🚀 Quick Start Guide

### 1. Prerequisites

- **Rust toolchain** (1.80 or newer):
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```
- **Linux dependencies** (for X11 capture and injection):
  ```bash
  sudo apt-get install -y libxtst-dev libx11-dev libxfixes-dev x11proto-record-dev gcc-mingw-w64
  ```

### 2. Building

Clone the repository and build release binaries:
```bash
git clone https://github.com/yogeswar142/friday.git
cd friday

# Build native Linux release binary
cargo build -p friday-agent --release

# Cross-compile for Windows (if building from Linux)
CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc \
cargo build -p friday-agent --target x86_64-pc-windows-gnu --release
```

### 3. Running an End-to-End Session

#### On Computer B (Receiver / Target Laptop — e.g. Windows)
```powershell
.\friday-agent.exe receive --bind 0.0.0.0:48700
```

#### On Computer A (Sender / Primary Laptop with Physical Mouse — e.g. Linux)
```bash
DISPLAY=:1 ./target/release/friday-agent send --peer 192.168.1.2:48700 --bind 0.0.0.0:48701 --dwell-ms 500
```

### 4. Configuration Options

| Option | Flag | Default | Description |
|:---|:---|:---:|:---|
| **Peer Address** | `--peer <IP:PORT>` | *Required* | Address of remote FRIDAY agent |
| **Local Bind** | `--bind <IP:PORT>` | `0.0.0.0:48701` | Local UDP bind interface |
| **Dwell Time** | `--dwell-ms <MS>` | `500` | Boundary dwell time required to transfer ownership |
| **Edge Threshold** | `--edge-px <PX>` | `3` | Screen edge trigger zone in pixels |
| **Remote Width** | `--remote-w <PX>` | `1920` | Remote screen width (auto-updated by handshake) |
| **Remote Height** | `--remote-h <PX>` | `1080` | Remote screen height (auto-updated by handshake) |
| **Direct Mode** | `--direct` | `false` | Immediately take remote control without edge wait |

---

## 🗺️ Roadmap & Future Upgrades

```text
Phase 1: Core Foundation           [COMPLETED ✅]
Phase 1.5: Real Hardware Validation[COMPLETED ✅]
Phase 2: Keyboard & Clipboard      [NEXT UP 🚧]
Phase 2.5: Security & Auto-Discovery
Phase 3: Native Desktop GUI
Phase 4: Wayland & macOS Support
```

### 🔹 Phase 1 — Core Routing & Protocol Foundations (Completed ✅)
- Normalized coordinate translation space ($0 \dots 65535$).
- Exclusive ownership router preventing simultaneous multi-device input leakage.
- Held-button safety state machine and UDP network transport.

### 🔹 Phase 1.5 — Physical Hardware Validation (Completed ✅)
- End-to-end physical validation between Linux X11 and Windows 11.
- Symmetrical circular edge navigation across screen borders.
- 500ms edge dwell resistance timer preventing accidental handoffs.
- Inward safety margin and coordinate continuity memory.
- Real-time diagnostic session logging system.

### 🔹 Phase 2 — Keyboard, Clipboard & File Sharing (In Progress 🚧)
- **Cross-Platform Keyboard Synchronization**: Hardware scancode translation table (evdev $\leftrightarrow$ Win32 Virtual-Keys $\leftrightarrow$ macOS KeyCodes).
- **Clipboard Sharing**: Real-time bi-directional synchronization of text, UTF-8 formatting, and images.
- **Drag-and-Drop File Streaming**: Chunked file transfer protocol over parallel TCP/QUIC streams with progress tracking and SHA-256 integrity validation.

### 🔹 Phase 2.5 — Encryption & Peer Auto-Discovery
- **End-to-End Cryptography**: Noise Protocol Framework / DTLS handshake with mutual authentication.
- **Zero-Conf Auto-Discovery**: mDNS / DNS-SD peer discovery for zero-manual-IP setup.
- **Multi-Device Ring Topology**: Support for 3+ devices connected in arbitrary circular or planar geometric configurations.

### 🔹 Phase 3 — Modern Desktop GUI
- **Visual Topology Editor**: Drag-and-drop screen layout canvas to configure physical monitor placements visually.
- **Real-Time Settings Panel**: Runtime sliders for edge dwell time (`dwell_ms`), edge resistance, and acceleration curves.
- **System Tray & Telemetry**: Native tray icon, network latency graphs, and packet loss monitors.

### 🔹 Phase 4 — Platform Expansion
- **Linux Wayland**: Integration with `ext-virtual-input` and desktop portals.
- **macOS Native Engine**: Universal binary support (`x86_64` & `arm64`) using `CoreGraphics` event taps and Accessibility APIs.

---

## 🤝 Contribution Guidelines

We welcome community contributions! To maintain code quality, security, and low-latency performance, please follow these rules:

1. **Safety First**:
   - Keep unsafe code blocks strictly isolated to platform FFI layers (`friday-platform`). All FFI calls must have explicit `// SAFETY:` justifications.
2. **Deterministic Architecture**:
   - Never introduce thread sleeps or unbounded polling in fast paths. Use event-driven channels and reactive async loops.
3. **Hardware Validation Rule**:
   - Any PR adding or modifying OS input backends must include reproducible test logs or verification notes on real hardware.
4. **Code Standards**:
   - Ensure formatting passes:
     ```bash
     cargo fmt --all --check
     ```
   - Ensure all clippy warnings are addressed:
     ```bash
     cargo clippy --all-targets -- -D warnings
     ```
   - Ensure all workspace tests pass:
     ```bash
     cargo test --all
     ```
5. **Git Workflow**:
   - Create focused branches (`feature/keyboard-sync`, `fix/dwell-timer`).
   - Use conventional commit messages (`feat: ...`, `fix: ...`, `docs: ...`, `refactor: ...`).

---

## 📄 License

FRIDAY is dual-licensed under either:
- **MIT License** ([LICENSE-MIT](LICENSE-MIT) or [http://opensource.org/licenses/MIT](http://opensource.org/licenses/MIT))
- **Apache License, Version 2.0** ([LICENSE-APACHE](LICENSE-APACHE) or [http://www.apache.org/licenses/LICENSE-2.0](http://www.apache.org/licenses/LICENSE-2.0))

at your option.
