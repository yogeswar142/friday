# FRIDAY Platform Compatibility Matrix

This document tracks verified feature support across operating systems and architectures.

> [!IMPORTANT]
> In accordance with project engineering rules (`gemini.md`), capability is **strictly differentiated**:
> - **Physically Validated**: Tested and verified on physical machines with actual hardware input devices and live OS environments.
> - **🧪 Unit & Sim Tested**: Passing automated unit, integration, and loopback simulation test suites (89/89 automated tests).
> - **🟡 Compiled / Implemented**: Architecture code paths fully implemented; awaiting physical hardware target validation.

---

## 1. Cross-Platform Support Matrix

| Operating System | Architecture | Zero-Config Discovery | Noise Encrypted Transport | Mouse Routing & Capture | Keyboard Capture & Routing | Clipboard Sync | Status & Validation Level |
| :--- | :--- | :---: | :---: | :---: | :---: | :---: | :--- |
| **Windows 10 / 11** | x64 | **Physically Validated** | **Physically Validated** | **Physically Validated** | **Physically Validated** | **Physically Validated** | Main Host & Client validated; `WH_MOUSE_LL` + `WH_KEYBOARD_LL` capture and Win32 `mouse_event`/`keybd_event` injection active. |
| **Windows 10 / 11** | ARM64 (aarch64) | **Physically Validated** | **Physically Validated** | **Physically Validated** | **Physically Validated** | **Physically Validated** | Physically validated as active Client (Lenovo Yoga ARM64); native Win32 SDK injection and capture support. |
| **Linux (X11)** | x86_64 | **Physically Validated** | **Physically Validated** | **Physically Validated** | **Physically Validated** | **Physically Validated** | Physically validated as Main Host (Lenovo G50); `XGrabPointer` + `XGrabKeyboard` capture, `xclip` clipboard sync, XTest injection. |
| **Linux (X11)** | ARM64 (aarch64) | 🧪 Unit & Sim Tested | 🧪 Unit & Sim Tested | 🧪 Unit & Sim Tested | 🧪 Unit & Sim Tested | 🧪 Unit & Sim Tested | Architecture-independent X11/Xlib/XTest/xclip ABI; verified through platform-agnostic suites. |
| **Linux (Wayland)** | x86_64 / ARM64 | 🧪 Unit & Sim Tested | 🧪 Unit & Sim Tested | 🟡 Implemented | 🟡 Implemented | 🟡 Implemented | Compatible via XWayland bridge; native Wayland requires compositor-specific portals (`xdg-desktop-portal` / `libei`). |
| **macOS (Apple Silicon)** | ARM64 (M1–M4) | 🧪 Unit & Sim Tested | 🧪 Unit & Sim Tested | 🧪 Unit & Sim Tested | 🧪 Unit & Sim Tested | 🟡 Implemented | Core crates, Noise transport, circular topology simulation tested; `pbcopy`/`pbpaste` clipboard sync. |
| **macOS (Intel)** | x86_64 | 🧪 Unit & Sim Tested | 🧪 Unit & Sim Tested | 🧪 Unit & Sim Tested | 🧪 Unit & Sim Tested | 🟡 Implemented | Core crates compile and pass tests; platform abstraction traits implemented. |

---

## 2. Platform Architecture & Subsystem Implementation

### Windows (x64 & ARM64)
- **Input Capture**:
  - `WH_MOUSE_LL` low-level hook capturing movement, left/right/middle clicks, and vertical/horizontal wheel deltas from all OS-visible pointers (USB, Bluetooth, built-in touchpads).
  - `WH_KEYBOARD_LL` low-level hook capturing standard keys, extended keys, modifiers, and function keys with local suppression on Main Host when controlling remote.
  - Emergency local escape (`VK_ESCAPE`) asynchronously restores local host control.
- **Input Injection**:
  - `mouse_event` driver injection with `FRIDAY_INJECTED_MAGIC` discriminator tag.
  - `keybd_event` driver injection with virtual-key translation and hardware scancodes via `MapVirtualKeyW`.
- **Screen & Cursor**:
  - `GetSystemMetrics(SM_CXSCREEN / SM_CYSCREEN)`, `GetCursorPos`, `SetCursorPos`.
  - Micro-jitter wake pulse ensures DWM keeps cursor visible across active ownership transitions.
- **Clipboard**:
  - Win32 clipboard API: `OpenClipboard`, `EmptyClipboard`, `GlobalAlloc`, `SetClipboardData` (`CF_UNICODETEXT`), `GetClipboardData`.

### Linux (X11 & Wayland)
- **Input Capture**:
  - `XGrabPointer` with invisible 1x1 blank pixmap cursor to isolate host pointer movement.
  - `XGrabKeyboard` capturing raw `KeyPress` and `KeyRelease` events; keysym to `KeyCode` translation.
  - Boundary guard re-centering (`XWarpPointer`) ensures unlimited relative cursor movement without edge clamping.
  - Emergency escape via `XQueryKeymap` checking keysym `0xff1b` (XK_Escape).
- **Input Injection**:
  - `XTestFakeRelativeMotionEvent`, `XTestFakeButtonEvent`, `XTestFakeKeyEvent`.
- **Screen & Cursor**:
  - `XOpenDisplay`, `XDefaultScreen`, `XDisplayWidth`, `XQueryPointer`, `XWarpPointer`.
- **Clipboard**:
  - Subprocess pipe integration via `xclip -selection clipboard` (`-in` / `-o`).

### macOS (Apple Silicon & Intel)
- **Transport & Core**:
  - 100% platform-agnostic `friday-core` (circular topology, coordinate normalization, held-input safety) and `friday-network` (Noise encryption, UDP socket transport, zero-config discovery).
- **Clipboard**:
  - Built-in `pbcopy` and `pbpaste` subprocess pipes.
- **Platform Trait Abstractions**:
  - `friday-platform` defines `InputBackend`, `ScreenBackend`, `CursorBackend` for native macOS CoreGraphics / CGEventTap wiring.

---

## 3. Automated Test Coverage

- **Automated Workspace Tests**: **89 / 89 Passing (100%)**
  - `friday-core`: 35 tests (Circular topology, 2/3/N device loops, edge transfer, held key/button release safety, keyboard destination routing, emergency escape).
  - `friday-network`: 18 tests (Noise XX handshake & encrypted transport, replay window, token authentication, discovery records, connection state machine).
  - `friday-network` Integration: 11 tests (Exclusive host model, remote disconnect fallback, reconnection backoff, trust store persistence).
  - `friday-agent`: 20 tests (Windows scancode mapping, virtual key translation, escape pass-through, injection safety, edge detection).
  - `friday-gui`: 4 tests (Diagnostics evaluation, app state init, benchmark execution, log rotation).
  - `friday-platform`: 1 test (Mock backend trait simulation).
