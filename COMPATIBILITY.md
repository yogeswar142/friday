# FRIDAY Platform Compatibility Matrix

This document tracks verified feature support across operating systems and architectures. 

> [!NOTE]
> Architecture capability is NOT claimed as supported until actual testing or reproducible validation is performed.

| Operating System | Arch | Mouse | Keyboard | Clipboard | Files | Status / Backend Notes |
| :--- | :--- | :---: | :---: | :---: | :---: | :--- |
| **Linux (Ubuntu / Debian / Fedora)** | x64 | 🟡 | 🟡 | 🟡 | 🟡 | Initial development target; X11/Wayland backends in progress. |
| **Linux** | ARM64 | ❓ | ❓ | ❓ | ❓ | Untested |
| **Windows 10 / 11** | x64 | ❓ | ❓ | ❓ | ❓ | Untested; Win32 / RawInput backend planned. |
| **Windows 10 / 11** | ARM64 | ❓ | ❓ | ❓ | ❓ | Untested |
| **macOS** | Intel x64 | ❓ | ❓ | ❓ | ❓ | Untested; CGEvent backend planned. |
| **macOS** | Apple Silicon ARM64 | ❓ | ❓ | ❓ | ❓ | Untested |

**Legend:**
- `✓` Tested & Verified Working
- `🟡` In Progress / Partial Mock & Simulation Validation
- `❓` Untested / Unverified Architecture
