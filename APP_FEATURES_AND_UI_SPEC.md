# FRIDAY — Dynamic Features & UI Design Specification
> **Target Audience / Purpose**: Comprehensive feature breakdown, screen layouts, design tokens, and dynamic data flows for generating a world-class UI using **Gemini Google Stitch AI**.

---

## 1. Product Overview & Core Philosophy

**FRIDAY** is an ultra-low latency, cross-platform software KVM (Keyboard, Video/Mouse sharing) and peer-to-peer workspace mesh. It enables a single physical mouse and keyboard on a **Main Host** (e.g., Linux X11 / Windows) to seamlessly control multiple **Client** machines (e.g., Windows 11 ARM64, macOS, Linux) across a local Wi-Fi / Ethernet network with **sub-millisecond latency**.

### Core Architecture Highlights:
- **Exclusive Ownership Model**: At any moment, exactly ONE device has input ownership. The cursor and keyboard flow seamlessly between screens.
- **Infinite Circular Ring Topology**: Moving off the right edge of screen $N$ instantly wraps around to the left edge of screen $1$ and vice versa.
- **Lock-Free Zero-Allocation Telemetry**: Real-time packet throughput, sub-millisecond RTT tracking, and Wi-Fi stall detection.
- **Noise Protocol Cryptographic Security**: Authenticated peer pairing with 6-digit mutual authentication PINs and encrypted UDP transport.

---

## 2. Design System & Visual Tokens (For Google Stitch AI)

### A. Design Aesthetic
- **Mood**: High-performance developer tool, futuristic yet minimalist (inspired by *Linear, Raycast, Vercel, Warp Terminal*).
- **Theme**: Deep Dark Mode (default) with high-contrast functional accents and refined glassmorphic cards.
- **Borders & Corners**: Subtle semi-transparent borders (`1px solid rgba(255, 255, 255, 0.08)`), rounded corners (`8px` sm, `12px` md, `16px` lg).

### B. Color Palette Tokens
| Token Name | Hex Code | Purpose |
|------------|----------|---------|
| `--bg-canvas` | `#0B0F17` | Root application background |
| `--bg-surface` | `#111827` | Navigation bar, sidebars, modal backdrops |
| `--bg-card` | `#161F30` | Interactive cards, metric panels, container blocks |
| `--bg-card-hover` | `#1C273C` | Card hover state |
| `--border-subtle` | `rgba(255, 255, 255, 0.08)` | Standard divider & card borders |
| `--border-focus` | `#3B82F6` | Active input or focused element border |
| `--accent-cyan` | `#06B6D4` | Primary brand accent, network throughput, PPS |
| `--accent-emerald` | `#10B981` | Active ownership, healthy connection, success, low RTT |
| `--accent-amber` | `#F59E0B` | Wi-Fi jitter, latency warnings, paused engine, pending pairing |
| `--accent-rose` | `#F43F5E` | Wi-Fi stalls, disconnected peers, critical alerts, unpair |
| `--accent-indigo` | `#6366F1` | Host machine role, topology ring paths, encryption badges |
| `--text-primary` | `#F9FAFB` | Primary headers, active values |
| `--text-secondary` | `#9CA3AF` | Labels, descriptions, secondary data |
| `--text-muted` | `#6B7280` | Timestamps, inactive states, unit labels |
| `--font-mono` | `JetBrains Mono, Fira Code, monospace` | Numeric metrics, IP addresses, PINs, logs |

---

## 3. Global App Shell & Navigation

### Global Header / Navigation Bar (`Navbar.tsx`)
Always visible at the top of the application window.

#### Left Section: Brand & Engine Master Control
- **FRIDAY Logo & Brand**: Sleek icon with glowing status dot.
- **Engine Master Switch**:
  - `Running` (Green glow, badge: `RUNNING`)
  - `Stopped` (Dim gray, badge: `STOPPED`)
  - `Paused` (Amber badge: `PAUSED`)
  - Toggle Button: One-click Start/Stop.
  - Pause Button: Quick pause (temporarily locks mouse to local screen without disconnecting peers).

#### Center Section: Navigation Tabs
Clean pill-style navigation tabs with active indicator glow:
1. **Overview** (Dashboard icon)
2. **Devices** (Laptop / Multi-screen icon + Badge for pending pair requests)
3. **Topology** (Circular loop icon)
4. **Diagnostics** (Activity / Pulse icon + Wi-Fi health indicator)
5. **Settings** (Sliders / Cog icon)

#### Right Section: Live Status Pill & Quick Controls
- **Machine Role Pill**: Indicates `Main Host` (Indigo) or `Client Screen` (Cyan).
- **Active Owner Pill**: Shows which physical machine currently has the mouse (`Active: Lenovo Yoga`).
- **Connected Peers Badge**: Counter e.g., `2 Peers Online`.
- **Theme Toggle**: Switch between Dark, Light, and System modes.

---

## 4. Complete Page-by-Page Specifications

---

### Page 1: Overview Dashboard (`Overview.tsx`)
*The command center showing real-time system state, connected devices, and quick actions.*

#### 1. Hero Identity Banner
- **Local Machine Identity**:
  - Device Display Name (e.g., `Lenovo-G50` or `Yogeswar-PC`).
  - OS Badge (Windows 11 / Linux X11 / macOS / Android).
  - Role: `Main Host (Controls Other Screens)` vs `Client (Receives Input)`.
  - Local Endpoint: IP Address & Port (e.g., `192.168.1.10:48700`).
  - Active Owner Indicator: Displays whether the mouse is currently on this machine or controlling a remote screen.

#### 2. Real-Time Telemetry Quick Strip
Four live dynamic stat cards:
- **Throughput Rate**: Live packets per second (`PPS`) and transmission speed (`KB/s`).
- **Round-Trip Latency (RTT)**: Sub-millisecond latency (e.g., `1.85 ms`) with a mini sparkline/dot indicator.
- **Network Health Rating**: `Optimal (Sub-5ms)`, `Good`, or `Stalled`.
- **Active Ring Status**: Number of active devices in the circular chain (e.g., `3 Devices in Ring`).

#### 3. Active Ownership & Quick Handoff Card
- Highlights the machine that currently owns the cursor.
- One-click **"Take Control Back (Local Host)"** button to immediately recall the mouse to the main screen.
- Emergency Escape reminder: Press `Ctrl + Alt + Shift + Esc` to break free instantly.

#### 4. Circular Ring Quick Visualizer Card
- Mini dynamic preview of the circular topology.
- Arrows showing transition flow: `Device A → Device B → Device C → Device A`.
- Button: *"Open Full Topology Editor"*.

#### 5. Connected Devices Quick List
- Cards for each paired peer showing:
  - Online/Offline status dot.
  - Device Name, IP, and Operating System.
  - Per-device toggles: Mouse `[✓]`, Keyboard `[✓]`, Clipboard `[✓]`.
  - Action button: *"Switch Input Here"*.

---

### Page 2: Connected & Discovered Devices (`Devices.tsx`)
*Device pairing, network discovery, per-device permissions, and connection management.*

#### 1. Local Machine Configuration Card
- Editable display name (e.g., rename to "My Workstation").
- **Host Master Sharing Toggles**:
  - Global `Share Mouse` toggle (master switch to enable/disable remote mouse sharing).
  - Global `Share Keyboard` toggle (master switch to enable/disable remote keystroke forwarding).
  - Global `Share Clipboard` toggle (sync copy-paste buffers).

#### 2. Paired Devices List (Registered Ring Members)
Each paired device is displayed in an expansive card with rich state:
- **Header**: Device Name, OS icon (Apple, Windows, Linux, Android), and Connection Badge (`Connected`, `Connecting`, `Offline`).
- **Active Status**: Golden crown badge if this device is the current active screen owner.
- **Endpoint Details**: IP Address, Port, and Device UUID.
- **Input Permissions Toggles (Per-Device)**:
  - `Allow Mouse Input`: If turned off, cursor will not cross into this machine.
  - `Allow Keyboard Input`: If turned off, keystrokes will be dropped with no fallback when cursor is on this machine.
  - `Allow Clipboard Sync`: Bidirectional clipboard sharing.
- **Action Buttons**:
  - `Make Active Owner` (Instantly transfers cursor to this device's screen).
  - `Connect / Disconnect` (Toggles socket connection without unpairing).
  - `Unpair Device` (Destructive action with confirmation, revokes trust token).

#### 3. Local Network Discovered Devices (mDNS / UDP Beacon Scanner)
- **Live Radar / Scanner Header**: "Scanning local Wi-Fi / LAN for FRIDAY devices..."
- Refresh button to re-trigger network broadcast discovery.
- **Discovered Device Cards**:
  - Device Name, IP Address, OS, and signal beacon freshness.
  - **"Pair Device" Button**:
    - Clicking triggers secure Noise handshake.
    - Generates and displays a **6-Digit Cryptographic Authentication PIN**.

#### 4. Manual IP Pairing Section
- For complex multi-subnet networks, VLANs, or when UDP broadcast is disabled by the router:
  - Input field for Remote IP Address (e.g., `192.168.1.15`).
  - Input field for Port (Default: `48700`).
  - Button: *"Initiate Secure Pairing"*.

#### 5. Incoming Pairing Request Modal (`PendingPairRequest`)
- Triggered instantly when a remote device requests pairing:
  - Displays Remote Device Name & IP.
  - **Large 6-Digit PIN Display** (e.g., `8 4 9 2 0 1`) for visual mutual verification between both monitor screens.
  - Confirmation warning: "Only accept if this PIN matches the number shown on the other computer."
  - Buttons: `Accept & Add to Ring` (Emerald) vs `Reject` (Rose).

---

### Page 3: Infinite Circular Ring Topology (`CircularTopologyEditor.tsx`)
*Interactive visual editor for arranging screens in a continuous multi-monitor circle.*

#### 1. Interactive Canvas / Ring Visualizer
- **Visual Display**: Screens arranged around an interactive circular orbit or horizontal infinite loop.
- **Drag-to-Reorder**: Users can drag screen cards around the ring to change physical layout:
  - Example: `[Yoga (Left)] ↔ [G50 Host (Center)] ↔ [MacBook (Right)]`.
- **Directional Loop Arrows**: Glowing animated particles showing seamless wrap-around:
  - Pushing cursor off the far right edge of the rightmost monitor enters the far left edge of the leftmost monitor.
- **Active Monitor Highlight**: Glowing pulsating border around whichever screen currently holds the cursor.

#### 2. Screen Configuration Inspector
- Selecting any screen in the ring opens a side properties drawer:
  - Screen resolution and aspect ratio (e.g., `1920x1080 @ 1.0x scaling`).
  - Transition edge dwell time (e.g., `250 ms` threshold to prevent accidental edge slips).
  - Corner escape deadzones (prevent accidental hopping when clicking window Close/Minimize buttons).

#### 3. Topology Action Bar
- Button: `Auto-Detect Neighboring Screens`.
- Button: `Reverse Ring Direction (Clockwise / Counter-Clockwise)`.
- Button: `Save Topology Configuration`.

---

### Page 4: Real-Time Diagnostics & Network Working Report (`Diagnostics.tsx`)
*Deep observability, performance benchmarking, Wi-Fi stability diagnosis, and one-click copy report.*

#### 1. Real-Time Network & Input Working Report Panel (NEW)
*Designed specifically to troubleshoot Wi-Fi latency, packet drops, and old hardware stutters without impacting CPU.*
- **Top Metrics Grid**:
  - `Session Duration`: Formatted elapsed time (`HH:MM:SS`) and UTC start timestamp.
  - `Real-Time Latency (RTT)`: Current round-trip ping time (e.g., `1.85 ms`), with Min / Max / Avg breakdown.
  - `Wi-Fi / Socket Stalls`: Total count of gaps exceeding 120ms during active input, with maximum stall duration (`Max stall: 145ms`).
  - `Network Health Rating`: Badge e.g., `Optimal (Sub-5ms, Zero Stalls)` or `Stalled / Congested (2 stalls detected)`.
  - `Throughput`: Transmit PPS & KB/s, Receive PPS & KB/s.
- **Input Breakdown Sub-Cards**:
  - **Transmitted Traffic (Host Mode)**:
    - Mouse Motion Packets (e.g., `42,850`).
    - Mouse Clicks & Scrolls (e.g., `182`).
    - Keyboard Keystrokes (e.g., `1,420`).
    - Control & Heartbeat Pings (e.g., `480`).
  - **Received Traffic (Client Mode)**:
    - Mouse Moves Injected (e.g., `42,850`).
    - Clicks & Scrolls Injected (e.g., `182`).
    - Keystrokes Injected (e.g., `1,420`).
    - Control Handshakes Handled (e.g., `480`).
- **One-Click Action Button**:
  - **`"Copy Full Network Report"`**: Compiles the entire session history into a structured Markdown document and copies it to the OS clipboard for sharing in chats, issues, or email.
- **Collapsible Operational Event Timeline**:
  - Chronological history of milestones with color-coded severity tags (`INFO`, `WARN`, `STALL`, `ERROR`):
    - `[12:42:25] INFO  [system] FRIDAY initialized on local machine: YOGESWAR (192.168.1.10)`
    - `[12:42:36] INFO  [ownership] Host mouse entered screen — placed at (960, 600)`
    - `[12:44:10] STALL [network] Wi-Fi packet gap of 145ms detected during active mouse motion`

#### 2. System Diagnostics Suite
- **"Run System Diagnostics" Button**:
  - Runs automated checks on:
    - UDP Socket Binding & Local Loopback Latency.
    - OS Accessibility & Input Injection Permissions (Windows UIPI / Linux XTest / macOS Accessibility).
    - Screen Bounds & DPI Scaling Normalization.
    - Firewall & Local Port Reachability.
  - Status items with Pass/Fail badges and one-click remediation tips.

#### 3. Core Engine Benchmark
- **"Run Engine Benchmark" Button**:
  - Runs 100,000 synthetic event rounds through the serialization and routing pipeline.
  - Displays:
    - Packet Encoding / Decoding Latency (typically `< 0.005 ms`).
    - Theoretical Throughput (e.g., `140,000 packets/sec`).
    - Memory Allocation per Event: `0 bytes` (Zero-allocation validation).

#### 4. Live Engine Log Stream
- Console terminal view of real-time logs.
- Filters: Log Level (`ALL`, `INFO`, `WARN`, `ERROR`), Module Target (`engine`, `keyboard`, `pairing`, `network`).
- Live search query input.
- Controls: `Clear Logs`, `Auto-Scroll Toggle`, `Copy Raw Logs`.

---

### Page 5: Settings & Input Preferences (`Settings.tsx`)
*Fine-tuning network ports, hotkeys, transition sensitivity, and clipboard options.*

#### 1. Network & Protocol Security
- `Peer Port`: Custom UDP port (Default: `48700`).
- `Discovery Port`: Custom broadcast port (Default: `48701`).
- `Encryption Status`: Noise Protocol XX Handshake active (displays local public key fingerprint).
- `Auto-Reconnect`: Toggle automatic reconnection with exponential backoff on network drop.

#### 2. Input Transition & Sensitivity
- `Edge Dwell Time`: Slider (100ms to 1000ms) — amount of time the cursor must press against the screen border before hopping to the next monitor.
- `Corner Escape Margin`: Deadzone slider (0px to 50px) to prevent hopping when clicking top-right window close buttons.
- `Mouse Sensitivity Multiplier`: Fine-tune speed when controlling high-DPI remote screens.
- `Emergency Escape Key Combination`: Configurable hotkey (Default: `Ctrl + Alt + Shift + Escape` or triple-tap Escape) to instantly force-release mouse capture back to local host.

#### 3. Clipboard & File Transfer Preferences
- `Enable Clipboard Sync`: Auto-sync text and images between clipboards.
- `Max Clipboard Size`: Limit buffer size (e.g., `10 MB`).
- `File Transfer Directory`: Destination path for received files.
- `Auto-Accept Files`: From trusted paired devices only.

#### 4. Appearance & System Tray
- `Theme Selection`: Dark, Light, System.
- `Close to System Tray`: Keep running in background when window is closed.
- `Launch at Startup`: Run on system boot.

---

### Page 6: First-Run Onboarding Modal (`FirstRunModal.tsx`)
*Guided 3-step setup wizard shown upon initial launch.*

- **Step 1: Role Selection**:
  - "Is this your Main Computer (with the physical keyboard/mouse) or a Client Screen?"
  - Select `Main Host` or `Client Device`.
- **Step 2: Permissions Check**:
  - Verifies OS accessibility / input injection permissions with a green checkmark.
- **Step 3: Discover & Pair**:
  - Quick scanner to pair the first companion laptop or desktop.

---

## 5. Complete Google Stitch AI Prompt Templates

Use these ready-to-paste prompts in **Gemini Google Stitch AI** to generate the mockups and layout code:

### Master Prompt for Complete App Interface
```text
Design a modern, high-performance desktop application interface called "FRIDAY" — a cross-platform software KVM and workspace mesh for sharing a single mouse and keyboard across multiple computers (Windows, Linux, Mac).

Aesthetic: Sleek dark cyberpunk developer tool style similar to Linear and Raycast. Deep navy-black background (#0B0F17), dark slate cards (#161F30), subtle borders (rgba(255,255,255,0.08)), with bright functional accents: Emerald Green (#10B981) for active device ownership and sub-millisecond latency, Cyan (#06B6D4) for network throughput and PPS metrics, Amber (#F59E0B) for Wi-Fi jitter warnings, and Indigo (#6366F1) for Host roles. Monospaced typography for numbers, IPs, and telemetry.

Structure:
1. Header Navbar:
   - FRIDAY branding with glowing status dot.
   - Master Engine Switch (Running / Stopped / Paused).
   - Navigation tabs: Overview, Devices, Topology, Diagnostics, Settings.
   - Live Status badges: Machine Role ("Main Host"), Active Input Owner ("Active: Lenovo Yoga"), Connected Peers count.
   - Theme toggle button.

2. Page 1: Overview Dashboard:
   - Hero card with Local Device Name, OS badge, IP/Port, and active ownership status.
   - 4-column metric strip: Real-time Packets/sec, Throughput KB/s, Round-Trip Latency (e.g. 1.85 ms), and Wi-Fi Stalls.
   - Active screen handoff card with "Take Control Back" button.
   - Mini circular ring topology preview with animated directional flow.
   - Connected peers quick list with per-device mouse/keyboard/clipboard toggle switches.

3. Page 2: Connected & Discovered Devices:
   - Host Master Input Permissions (Share Mouse, Share Keyboard, Share Clipboard toggles).
   - Paired device cards with OS badges, golden crown for active screen owner, IP address, and per-device permission switches.
   - Discovered devices radar scanning section with "Pair Device" buttons.
   - Pending pairing request modal showing a large 6-digit mutual authentication PIN (e.g. 849 201) with Accept/Reject buttons.

4. Page 3: Infinite Circular Topology Editor:
   - Visual circular canvas with multiple computer screens arranged in a ring.
   - Drag-and-drop screen reordering showing infinite wrap-around (cursor moving off right edge of screen 3 wraps into left edge of screen 1).
   - Glowing aura on the currently active screen.

5. Page 4: Real-Time Diagnostics & Network Working Report:
   - Live network report dashboard: Session Duration, RTT latency (Min/Max/Avg), Wi-Fi stall counter, Network Health badge ("Optimal Sub-5ms").
   - Transmitted vs Received packet breakdown cards for mouse moves, clicks, keystrokes, and control packets.
   - Prominent "Copy Full Network Report" button for one-click markdown clipboard export.
   - Collapsible operational event timeline with color-coded severity tags (INFO, WARN, STALL, ERROR).
   - Live engine diagnostic logs console with search and level filters.

6. Page 5: Settings:
   - Sections for Network & Ports, Edge Dwell Threshold slider (100-1000ms), Mouse Sensitivity, Emergency Escape Hotkey, and System Tray behavior.
```

---

## 6. Summary of Dynamic Features Matrix

| Feature | Dynamic State Source | Interactive Controls |
|---------|----------------------|----------------------|
| **Engine State** | `status.state` (`running`, `stopped`, `paused`) | Start, Stop, Pause toggles |
| **Input Ownership** | `status.active_device_id` | Click to switch owner, Take Control Back |
| **Network Throughput** | `telemetry.packets_per_sec`, `bytes_per_sec` | Real-time 1-second interval refresh |
| **Round-Trip Latency (RTT)** | Embedded microsecond Ping/Pong (`rtt_us`) | Live Min/Max/Avg display |
| **Wi-Fi Stall Detection** | RFC 3550 Inter-packet gap > 120ms | Stall count, max stall duration alert |
| **Packet Breakdown** | Lock-free atomic counters | Moves, Clicks, Keystrokes, Pings |
| **Copy Working Report** | Markdown generator in `network_report.rs` | One-click copy to clipboard |
| **Topology Arrangement** | `topology.ring` (Ordered UUID array) | Drag-to-reorder circular ring |
| **Device Discovery** | UDP beacon listener on port 48701 | Refresh radar, Pair button |
| **Pairing Verification** | Cryptographic 6-digit PIN | Accept / Reject modal |
| **Input Permissions** | Per-device `share_mouse`, `share_keyboard` | Real-time checkboxes (no fallback) |
| **Log Stream** | Bounded 1,000-item event buffer | Filter by level/target, search, clear |
