# FRIDAY Platform Compatibility Matrix

This document tracks verified feature support across operating systems and architectures. 

> [!IMPORTANT]
> In accordance with project engineering rules, capability is **strictly differentiated**:
> - **Implemented**: Code written and structurally present
> - **Compiled**: Verified compiling under the platform target toolchain
> - **Unit / Sim Tested**: Passing unit and simulated integration test suites
> - **Physically Validated**: Tested on physical machines with actual hardware input

| Operating System | Arch | Zero-Config Discovery | Noise Transport | Mouse Routing | Keyboard | Status & Validation Level |
| :--- | :--- | :---: | :---: | :---: | :---: | :--- |
| **Windows 10 / 11** | x64 | **Physically Validated** | **Physically Validated** | **Physically Validated** | 🟡 Implemented | Primary host environment; SendInput & RawInput backends active. |
| **Windows 10 / 11** | ARM64 | 🟡 Compiled | 🟡 Compiled | 🟡 Compiled | 🟡 Implemented | Compiles cleanly via cross-target; physical hardware pending. |
| **Linux (X11 / Wayland)** | x64 | 🧪 Unit & Sim Tested | 🧪 Unit & Sim Tested | 🧪 Unit & Sim Tested | 🟡 Implemented | mDNS + UDP broadcast discovery and Noise transport fully platform-agnostic. |
| **Linux** | ARM64 | 🧪 Unit & Sim Tested | 🧪 Unit & Sim Tested | 🧪 Unit & Sim Tested | 🟡 Implemented | Platform-agnostic network layer unit-tested. |
| **macOS (Apple Silicon)** | ARM64 | 🧪 Unit & Sim Tested | 🧪 Unit & Sim Tested | 🧪 Unit & Sim Tested | 🟡 Implemented | CGEvent abstraction implemented; network & Noise stack tested. |
| **macOS (Intel)** | x64 | 🧪 Unit & Sim Tested | 🧪 Unit & Sim Tested | 🧪 Unit & Sim Tested | 🟡 Implemented | Core crates compile and pass tests. |

**Legend:**
- **Physically Validated**: Verified working on physical hardware environment with active input devices.
- **🧪 Unit & Sim Tested**: Platform-agnostic core logic validated via automated unit, integration, and loopback simulation tests.
- **🟡 Compiled / Implemented**: Code path complete; awaiting physical hardware target validation.

