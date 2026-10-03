//! Windows input capture and injection backend.
//!
//! # Capture — WH_MOUSE_LL + WH_KEYBOARD_LL
//!
//! System-wide low-level hooks intercept ALL OS-visible pointer and keyboard
//! input at the kernel/HAL boundary before it reaches any application.
//! Because Windows merges every HID-compliant device (built-in touchpad, USB
//! mouse, Bluetooth mouse, multiple simultaneous mice; built-in keyboard, USB
//! keyboard, Bluetooth keyboard, multiple simultaneous keyboards) into one
//! virtual cursor and one keyboard HID stream, `WH_MOUSE_LL` and
//! `WH_KEYBOARD_LL` are inherently source-agnostic — they capture from every
//! physically connected pointing and typing device simultaneously with no
//! device enumeration required.
//!
//! # Hot-path constraints
//!
//! Hook callbacks run synchronously in the thread that pumps messages
//! (`GetMessageW`).  They must complete quickly.  This implementation uses
//! only:
//!   - `Ordering::Relaxed` / `SeqCst` atomics for shared state
//!   - `mpsc::Sender::try_send` (non-blocking)
//!   - `GetTickCount64` (a single RDTSC-level system call)
//!
//! No heap allocation, no mutex locking on the hot path.
//!
//! # Loop-prevention
//!
//! `inject_event` stamps every synthetic event with `dwExtraInfo =
//! FRIDAY_INJECTED_MAGIC`.  Both hook callbacks skip any event carrying that
//! sentinel, preventing feedback loops.
//!
//! # Injection — SendInput with mouse_event/keybd_event fallback
//!
//! Unchanged from the Phase-1 implementation.  See `inject_event`.
//!
//! # SAFETY
//!
//! All `unsafe` blocks are limited to Win32 FFI calls whose safety
//! requirements are documented inline.

#[cfg(target_os = "windows")]
use windows::Win32::{
    Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM},
    System::SystemInformation::GetTickCount64,
    UI::{
        Input::KeyboardAndMouse::{
            keybd_event, MapVirtualKeyW, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE,
            KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP,
            MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_HWHEEL, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
            MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN,
            MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_VIRTUALDESK, MOUSEEVENTF_WHEEL, MOUSEINPUT,
            VIRTUAL_KEY,
        },
        WindowsAndMessaging::{
            CallNextHookEx, GetMessageW, GetSystemMetrics, SetCursorPos, SetWindowsHookExW,
            ShowCursor, UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT, MSG, MSLLHOOKSTRUCT,
            SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN,
            WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEHWHEEL,
            WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
            WM_XBUTTONDOWN, WM_XBUTTONUP,
        },
    },
};

use std::sync::{
    atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicU64, AtomicU8, Ordering},
    Arc, OnceLock,
};

use tokio::sync::mpsc;

use crate::control::{EdgeTrigger, ScreenEdge};
use crate::logger::session_log;
use friday_core::{
    DisplayBounds, ElementState, InputEvent, KeyCode, KeyboardEvent, MouseButton, MouseEvent,
};

use crate::error::{AgentError, Result as AgentResult};

// ── Magic sentinel ────────────────────────────────────────────────────────────

/// All events synthesized by FRIDAY carry this value in `dwExtraInfo`.
/// Hook callbacks skip any event bearing this magic to break feedback loops.
pub const FRIDAY_INJECTED_MAGIC: usize = 0x46524944;

// ── Module-level globals shared with hook callbacks ───────────────────────────
//
// Win32 hook callbacks are bare `extern "system"` function pointers — they
// cannot safely capture a Rust closure.  All shared state lives in module-level
// statics that are initialised once before the hooks are installed.

/// Channel to send captured `InputEvent`s into the FRIDAY pipeline.
static CAPTURE_SENDER: OnceLock<mpsc::Sender<InputEvent>> = OnceLock::new();

/// Channel to send `EdgeTrigger`s when the local cursor dwells at a screen edge.
static EDGE_SENDER: OnceLock<mpsc::Sender<EdgeTrigger>> = OnceLock::new();

/// True while a remote device owns the physical input (cursor hidden, all
/// events suppressed locally and forwarded over the network).
static CAPTURE_IS_REMOTE: AtomicBool = AtomicBool::new(false);

/// When false, keyboard events for the active remote device are dropped with
/// no fallback (per-device keyboard permission).
static CAPTURE_KEYBOARD_ENABLED: AtomicBool = AtomicBool::new(true);

/// Screen geometry for the local machine — used for edge detection and
/// cursor-warp-to-centre arithmetic.
static CAPTURE_SCREEN_W: AtomicI32 = AtomicI32::new(1920);
static CAPTURE_SCREEN_H: AtomicI32 = AtomicI32::new(1080);
static CAPTURE_CENTER_X: AtomicI32 = AtomicI32::new(960);
static CAPTURE_CENTER_Y: AtomicI32 = AtomicI32::new(540);

/// Remote screen size — used to clamp the virtual remote cursor.
static CAPTURE_REMOTE_W: AtomicU32 = AtomicU32::new(1920);
static CAPTURE_REMOTE_H: AtomicU32 = AtomicU32::new(1080);

/// Stop flag — when set, the message-pump loop exits and hooks are removed.
static CAPTURE_STOP: AtomicBool = AtomicBool::new(false);

/// Edge-dwell timer: `GetTickCount64` value (ms) when the cursor first touched
/// an edge in local mode.  0 means "cursor is not at an edge".
static EDGE_DWELL_START_MS: AtomicU64 = AtomicU64::new(0);

/// Which edge is currently being dwelled on.
/// 0 = none, 1 = Left, 2 = Right, 3 = Top, 4 = Bottom.
static EDGE_CURRENT: AtomicU8 = AtomicU8::new(0);

/// Edge detection threshold (px from screen boundary).
static CAPTURE_EDGE_THRESHOLD: AtomicI32 = AtomicI32::new(3);

/// Required dwell duration in milliseconds before a handoff fires.
static CAPTURE_DWELL_MS: AtomicU64 = AtomicU64::new(500);

/// Thread ID of the message-pump thread, used to post `WM_QUIT` on shutdown.
static HOOK_THREAD_ID: AtomicU32 = AtomicU32::new(0);

// ── Cursor-warp position sentinel ─────────────────────────────────────────────
// After calling SetCursorPos(center_x, center_y) we store the warp target so
// the resulting synthetic WM_MOUSEMOVE can be identified and suppressed.
// i32::MIN is used as "not set".
static WARP_TARGET_X: AtomicI32 = AtomicI32::new(i32::MIN);
static WARP_TARGET_Y: AtomicI32 = AtomicI32::new(i32::MIN);

// ── Edge encoding helpers ──────────────────────────────────────────────────────

fn encode_edge(e: ScreenEdge) -> u8 {
    match e {
        ScreenEdge::Left => 1,
        ScreenEdge::Right => 2,
        ScreenEdge::Top => 3,
        ScreenEdge::Bottom => 4,
    }
}

// ── Edge detection (called from hook callback — must be allocation-free) ──────

/// Detects which screen edge the cursor is touching, if any.
fn detect_edge_win(x: i32, y: i32, w: i32, h: i32, threshold: i32) -> Option<ScreenEdge> {
    if x <= threshold {
        Some(ScreenEdge::Left)
    } else if x >= w - 1 - threshold {
        Some(ScreenEdge::Right)
    } else if y <= threshold {
        Some(ScreenEdge::Top)
    } else if y >= h - 1 - threshold {
        Some(ScreenEdge::Bottom)
    } else {
        None
    }
}

/// Called from the mouse hook in local mode on every `WM_MOUSEMOVE`.
///
/// Maintains a dwell timer: if the cursor stays at the same edge for
/// `CAPTURE_DWELL_MS`, fires an `EdgeTrigger` and activates remote mode.
fn handle_local_edge(x: i32, y: i32, screen_w: i32, screen_h: i32, threshold: i32) {
    let dwell_required = CAPTURE_DWELL_MS.load(Ordering::Relaxed);

    match detect_edge_win(x, y, screen_w, screen_h, threshold) {
        None => {
            // Cursor left any edge — reset dwell timer
            if EDGE_CURRENT.load(Ordering::Relaxed) != 0 {
                EDGE_DWELL_START_MS.store(0, Ordering::Relaxed);
                EDGE_CURRENT.store(0, Ordering::Relaxed);
            }
        }
        Some(edge) => {
            let enc = encode_edge(edge);
            let prev = EDGE_CURRENT.load(Ordering::Relaxed);
            // SAFETY: GetTickCount64 returns system uptime in ms; always valid.
            #[cfg(target_os = "windows")]
            let now_ms = unsafe { GetTickCount64() };
            #[cfg(not(target_os = "windows"))]
            let now_ms = 0u64;

            if prev == enc {
                // Still on the same edge — check dwell elapsed
                let start = EDGE_DWELL_START_MS.load(Ordering::Relaxed);
                if start != 0 && now_ms.saturating_sub(start) >= dwell_required {
                    // Dwell complete — trigger handoff
                    EDGE_DWELL_START_MS.store(0, Ordering::Relaxed);
                    EDGE_CURRENT.store(0, Ordering::Relaxed);

                    let norm_x = (x as f32 / screen_w as f32).clamp(0.0, 1.0);
                    let norm_y = (y as f32 / screen_h as f32).clamp(0.0, 1.0);

                    if let Some(tx) = EDGE_SENDER.get() {
                        let _ = tx.try_send(EdgeTrigger {
                            edge,
                            norm_x,
                            norm_y,
                        });
                    }

                    // Activate remote mode
                    CAPTURE_IS_REMOTE.store(true, Ordering::SeqCst);
                    session_log(&format!(
                        "[WIN DWELL COMPLETE] {:?} edge at ({}, {}). Switching to remote mode.",
                        edge, x, y
                    ));
                }
            } else {
                // New edge — start/restart dwell timer
                EDGE_CURRENT.store(enc, Ordering::Relaxed);
                EDGE_DWELL_START_MS.store(now_ms, Ordering::Relaxed);
            }
        }
    }
}

// ── WH_MOUSE_LL hook callback ─────────────────────────────────────────────────

/// Low-level mouse hook — called for every OS-level pointer event from every
/// pointing device (touchpad, USB mouse, Bluetooth mouse, or multiple
/// simultaneously).  The hook operates on the unified Windows HID pointer
/// stream; it does not enumerate individual devices.
///
/// # Behaviour
///
/// **Remote mode** (a remote device owns the physical input):
///   - Events synthesized by FRIDAY (`dwExtraInfo == FRIDAY_INJECTED_MAGIC`) or
///     the cursor-warp sentinel are eaten silently.
///   - All other events are suppressed (`LRESULT(1)`) — local apps receive
///     nothing.  The delta relative to the cursor centre is computed and sent
///     to the FRIDAY input pipeline.
///   - After a non-trivial motion, `SetCursorPos` warps the cursor back to the
///     screen centre so subsequent deltas remain valid.
///
/// **Local mode** (this machine controls itself):
///   - Events pass through (`CallNextHookEx`) — all local applications receive
///     them normally.
///   - `WM_MOUSEMOVE` events are inspected for screen-edge proximity; a dwell
///     timer triggers an ownership handoff if the cursor lingers.
#[cfg(target_os = "windows")]
unsafe extern "system" fn mouse_ll_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // Per MSDN: nCode < 0 must pass to CallNextHookEx unmodified.
    if code < 0 {
        return CallNextHookEx(HHOOK::default(), code, wparam, lparam);
    }

    // SAFETY: lParam is a pointer to MSLLHOOKSTRUCT valid for the duration of
    // this callback, as guaranteed by the Win32 hook contract.
    let info = &*(lparam.0 as *const MSLLHOOKSTRUCT);

    // ── Skip FRIDAY's own injected events ─────────────────────────────────────
    if info.dwExtraInfo == FRIDAY_INJECTED_MAGIC {
        return CallNextHookEx(HHOOK::default(), code, wparam, lparam);
    }

    let is_remote = CAPTURE_IS_REMOTE.load(Ordering::Relaxed);
    let ts = info.time;
    let cx = info.pt.x;
    let cy = info.pt.y;

    if is_remote {
        // ── Remote mode ───────────────────────────────────────────────────────

        // Skip the synthetic WM_MOUSEMOVE produced by our own SetCursorPos warp.
        if wparam.0 as u32 == WM_MOUSEMOVE {
            let wx = WARP_TARGET_X.load(Ordering::Relaxed);
            let wy = WARP_TARGET_Y.load(Ordering::Relaxed);
            if wx != i32::MIN && cx == wx && cy == wy {
                // Eat the warp echo and clear the sentinel.
                WARP_TARGET_X.store(i32::MIN, Ordering::Relaxed);
                WARP_TARGET_Y.store(i32::MIN, Ordering::Relaxed);
                return LRESULT(1);
            }
        }

        let center_x = CAPTURE_CENTER_X.load(Ordering::Relaxed);
        let center_y = CAPTURE_CENTER_Y.load(Ordering::Relaxed);

        let msg = wparam.0 as u32;

        match msg {
            WM_MOUSEMOVE => {
                let dx = (cx - center_x).clamp(i16::MIN as i32, i16::MAX as i32) as i16;
                let dy = (cy - center_y).clamp(i16::MIN as i32, i16::MAX as i32) as i16;

                if dx != 0 || dy != 0 {
                    if let Some(tx) = CAPTURE_SENDER.get() {
                        let _ = tx.try_send(InputEvent::Mouse(MouseEvent::MoveRel {
                            dx,
                            dy,
                            timestamp: ts,
                        }));
                    }
                    // Warp cursor back to centre so the next event reports a
                    // fresh delta relative to centre.
                    // SAFETY: SetCursorPos is safe with valid screen coordinates.
                    WARP_TARGET_X.store(center_x, Ordering::Relaxed);
                    WARP_TARGET_Y.store(center_y, Ordering::Relaxed);
                    SetCursorPos(center_x, center_y).ok();
                }
            }

            WM_LBUTTONDOWN => send_mouse_button(MouseButton::Left, ElementState::Pressed, ts),
            WM_LBUTTONUP => send_mouse_button(MouseButton::Left, ElementState::Released, ts),
            WM_RBUTTONDOWN => send_mouse_button(MouseButton::Right, ElementState::Pressed, ts),
            WM_RBUTTONUP => send_mouse_button(MouseButton::Right, ElementState::Released, ts),
            WM_MBUTTONDOWN => send_mouse_button(MouseButton::Middle, ElementState::Pressed, ts),
            WM_MBUTTONUP => send_mouse_button(MouseButton::Middle, ElementState::Released, ts),

            WM_XBUTTONDOWN | WM_XBUTTONUP => {
                // High word of mouseData: 1 = XBUTTON1 (Back), 2 = XBUTTON2 (Forward)
                let xbtn = (info.mouseData >> 16) & 0xFFFF;
                let btn = match xbtn {
                    1 => MouseButton::Back,
                    2 => MouseButton::Forward,
                    _ => MouseButton::Other(xbtn as u8),
                };
                let state = if msg == WM_XBUTTONDOWN {
                    ElementState::Pressed
                } else {
                    ElementState::Released
                };
                send_mouse_button(btn, state, ts);
            }

            WM_MOUSEWHEEL => {
                // High word of mouseData = signed wheel delta (WHEEL_DELTA = 120 per notch).
                // Positive = scroll up (toward user), negative = scroll down.
                let raw_delta = (info.mouseData >> 16) as i16; // signed
                let notches = raw_delta / 120;
                // FRIDAY convention: positive dy = scroll up
                let dy = notches.clamp(i16::MIN, i16::MAX);
                if let Some(tx) = CAPTURE_SENDER.get() {
                    let _ = tx.try_send(InputEvent::Mouse(MouseEvent::Scroll {
                        dx: 0,
                        dy,
                        timestamp: ts,
                    }));
                }
            }

            WM_MOUSEHWHEEL => {
                let raw_delta = (info.mouseData >> 16) as i16;
                let notches = raw_delta / 120;
                let dx = notches.clamp(i16::MIN, i16::MAX);
                if let Some(tx) = CAPTURE_SENDER.get() {
                    let _ = tx.try_send(InputEvent::Mouse(MouseEvent::Scroll {
                        dx,
                        dy: 0,
                        timestamp: ts,
                    }));
                }
            }

            _ => {}
        }

        // Suppress: do not pass to local applications.
        LRESULT(1)
    } else {
        // ── Local mode ────────────────────────────────────────────────────────
        // Buttons and scroll are not forwarded — only mouse position matters
        // for edge detection in local mode.
        if wparam.0 as u32 == WM_MOUSEMOVE {
            let screen_w = CAPTURE_SCREEN_W.load(Ordering::Relaxed);
            let screen_h = CAPTURE_SCREEN_H.load(Ordering::Relaxed);
            let threshold = CAPTURE_EDGE_THRESHOLD.load(Ordering::Relaxed);
            handle_local_edge(cx, cy, screen_w, screen_h, threshold);
        }
        // Pass through to local apps — nothing is suppressed in local mode.
        CallNextHookEx(HHOOK::default(), code, wparam, lparam)
    }
}

/// Convenience function — sends a mouse button event to the capture channel.
#[inline(always)]
fn send_mouse_button(btn: MouseButton, state: ElementState, ts: u32) {
    if let Some(tx) = CAPTURE_SENDER.get() {
        let _ = tx.try_send(InputEvent::Mouse(MouseEvent::Button {
            button: btn,
            state,
            timestamp: ts,
        }));
    }
}

// ── WH_KEYBOARD_LL hook callback ──────────────────────────────────────────────

/// Low-level keyboard hook — called for every OS-level key event from every
/// keyboard device (built-in, USB, Bluetooth, multiple simultaneous).
///
/// # Routing rules
///
/// | Mode | keyboard_enabled | Action |
/// |------|-----------------|--------|
/// | Local active | any | `CallNextHookEx` — key goes to local apps |
/// | Remote active | `true` | Suppress + send to FRIDAY pipeline → remote device |
/// | Remote active | `false` | Suppress + **drop** (no fallback, no automatic re-route) |
///
/// Emergency escape is intentionally NOT detected here.  The application-level
/// hotkey (Ctrl+C / operator-triggered `force_restore_local_ownership`) handles
/// escape.  `Escape` is routed exactly like any other key.
#[cfg(target_os = "windows")]
unsafe extern "system" fn keyboard_ll_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code < 0 {
        return CallNextHookEx(HHOOK::default(), code, wparam, lparam);
    }

    // SAFETY: lParam is a pointer to KBDLLHOOKSTRUCT valid for the hook callback.
    let info = &*(lparam.0 as *const KBDLLHOOKSTRUCT);

    // Skip FRIDAY's own injected keyboard events.
    if info.dwExtraInfo == FRIDAY_INJECTED_MAGIC {
        return CallNextHookEx(HHOOK::default(), code, wparam, lparam);
    }

    let is_remote = CAPTURE_IS_REMOTE.load(Ordering::Relaxed);

    if !is_remote {
        // Local mode: key goes to local apps unchanged.
        return CallNextHookEx(HHOOK::default(), code, wparam, lparam);
    }

    // Remote mode — check per-device keyboard permission.
    let kb_enabled = CAPTURE_KEYBOARD_ENABLED.load(Ordering::Relaxed);
    if !kb_enabled {
        // Keyboard disabled for the active remote device.
        // Drop silently — NO fallback to any other device.
        return LRESULT(1);
    }

    // Keyboard enabled on the active remote device — forward the event.
    let msg = wparam.0 as u32;
    let state = match msg {
        WM_KEYDOWN | WM_SYSKEYDOWN => ElementState::Pressed,
        WM_KEYUP | WM_SYSKEYUP => ElementState::Released,
        _ => return CallNextHookEx(HHOOK::default(), code, wparam, lparam),
    };

    if let Some(key) = vk_to_keycode(info.vkCode) {
        if let Some(tx) = CAPTURE_SENDER.get() {
            let _ = tx.try_send(InputEvent::Keyboard(KeyboardEvent {
                key,
                state,
                timestamp: info.time,
            }));
        }
    }

    // Suppress: do not deliver to local apps while remote owns the keyboard.
    LRESULT(1)
}

// ── VirtualKey → KeyCode translation ─────────────────────────────────────────

/// Maps a Win32 virtual-key code to a FRIDAY `KeyCode`.
/// Returns `None` for VK codes that have no FRIDAY representation (they will
/// be suppressed but not forwarded).
fn vk_to_keycode(vk: u32) -> Option<KeyCode> {
    // Digit row
    if (0x30..=0x39).contains(&vk) {
        return Some(match vk {
            0x30 => KeyCode::Key0,
            0x31 => KeyCode::Key1,
            0x32 => KeyCode::Key2,
            0x33 => KeyCode::Key3,
            0x34 => KeyCode::Key4,
            0x35 => KeyCode::Key5,
            0x36 => KeyCode::Key6,
            0x37 => KeyCode::Key7,
            0x38 => KeyCode::Key8,
            0x39 => KeyCode::Key9,
            _ => unreachable!(),
        });
    }
    // Latin letters A–Z
    if (0x41..=0x5A).contains(&vk) {
        return Some(match vk {
            0x41 => KeyCode::A,
            0x42 => KeyCode::B,
            0x43 => KeyCode::C,
            0x44 => KeyCode::D,
            0x45 => KeyCode::E,
            0x46 => KeyCode::F,
            0x47 => KeyCode::G,
            0x48 => KeyCode::H,
            0x49 => KeyCode::I,
            0x4A => KeyCode::J,
            0x4B => KeyCode::K,
            0x4C => KeyCode::L,
            0x4D => KeyCode::M,
            0x4E => KeyCode::N,
            0x4F => KeyCode::O,
            0x50 => KeyCode::P,
            0x51 => KeyCode::Q,
            0x52 => KeyCode::R,
            0x53 => KeyCode::S,
            0x54 => KeyCode::T,
            0x55 => KeyCode::U,
            0x56 => KeyCode::V,
            0x57 => KeyCode::W,
            0x58 => KeyCode::X,
            0x59 => KeyCode::Y,
            0x5A => KeyCode::Z,
            _ => unreachable!(),
        });
    }
    // Function keys F1–F12
    if (0x70..=0x7B).contains(&vk) {
        return Some(match vk {
            0x70 => KeyCode::F1,
            0x71 => KeyCode::F2,
            0x72 => KeyCode::F3,
            0x73 => KeyCode::F4,
            0x74 => KeyCode::F5,
            0x75 => KeyCode::F6,
            0x76 => KeyCode::F7,
            0x77 => KeyCode::F8,
            0x78 => KeyCode::F9,
            0x79 => KeyCode::F10,
            0x7A => KeyCode::F11,
            0x7B => KeyCode::F12,
            _ => unreachable!(),
        });
    }

    Some(match vk {
        0x08 => KeyCode::Backspace,
        0x09 => KeyCode::Tab,
        0x0D => KeyCode::Enter,
        0x14 => KeyCode::CapsLock,
        0x1B => KeyCode::Escape,
        0x20 => KeyCode::Space,
        0x25 => KeyCode::Left,
        0x26 => KeyCode::Up,
        0x27 => KeyCode::Right,
        0x28 => KeyCode::Down,
        0xA0 => KeyCode::LeftShift,
        0xA1 => KeyCode::RightShift,
        0xA2 => KeyCode::LeftControl,
        0xA3 => KeyCode::RightControl,
        0xA4 => KeyCode::LeftAlt,
        0xA5 => KeyCode::RightAlt,
        0x5B => KeyCode::LeftSuper,
        0x5C => KeyCode::RightSuper,
        // For any unmapped VK, forward as Other so the remote can handle it.
        other => KeyCode::Other(other),
    })
}

// ── Main capture loop ─────────────────────────────────────────────────────────

/// Installs `WH_MOUSE_LL` and `WH_KEYBOARD_LL` system hooks, then runs a
/// Win32 message pump until `stop` is set.
///
/// **Must be called from a dedicated OS thread** (not a tokio task) because
/// Win32 low-level hooks require the installing thread to pump messages via
/// `GetMessageW`.
///
/// All captured events are sent through `input_tx` into the FRIDAY pipeline.
/// Edge triggers are sent through `edge_trigger_tx` for handoff handling.
///
/// # Source agnosticism
///
/// `WH_MOUSE_LL` and `WH_KEYBOARD_LL` capture the unified Windows HID pointer
/// and keyboard streams.  Built-in touchpad, USB mouse, Bluetooth mouse, USB
/// keyboard, Bluetooth keyboard, and any number of simultaneously connected
/// devices all produce events through the same hook — FRIDAY sees them all
/// without any device enumeration or selection.
#[cfg(target_os = "windows")]
#[allow(clippy::too_many_arguments)]
pub fn capture_loop(
    input_tx: mpsc::Sender<InputEvent>,
    stop: Arc<AtomicBool>,
    is_remote_active: Arc<AtomicBool>,
    keyboard_enabled: Arc<AtomicBool>,
    edge_threshold_px: i32,
    screen_width: i32,
    screen_height: i32,
    edge_trigger_tx: mpsc::Sender<EdgeTrigger>,
    remote_screen_w: Arc<AtomicU32>,
    remote_screen_h: Arc<AtomicU32>,
    dwell_ms: Arc<AtomicU64>,
) {
    // ── Initialise module-level globals ────────────────────────────────────────
    // OnceLock: safe to call set() exactly once; subsequent calls are ignored.
    let _ = CAPTURE_SENDER.set(input_tx);
    let _ = EDGE_SENDER.set(edge_trigger_tx);

    CAPTURE_SCREEN_W.store(screen_width, Ordering::SeqCst);
    CAPTURE_SCREEN_H.store(screen_height, Ordering::SeqCst);
    CAPTURE_CENTER_X.store(screen_width / 2, Ordering::SeqCst);
    CAPTURE_CENTER_Y.store(screen_height / 2, Ordering::SeqCst);
    CAPTURE_REMOTE_W.store(remote_screen_w.load(Ordering::Relaxed), Ordering::SeqCst);
    CAPTURE_REMOTE_H.store(remote_screen_h.load(Ordering::Relaxed), Ordering::SeqCst);
    CAPTURE_EDGE_THRESHOLD.store(edge_threshold_px.max(1), Ordering::SeqCst);
    CAPTURE_DWELL_MS.store(dwell_ms.load(Ordering::Relaxed), Ordering::SeqCst);
    CAPTURE_IS_REMOTE.store(is_remote_active.load(Ordering::Relaxed), Ordering::SeqCst);
    CAPTURE_KEYBOARD_ENABLED.store(keyboard_enabled.load(Ordering::Relaxed), Ordering::SeqCst);
    CAPTURE_STOP.store(false, Ordering::SeqCst);

    // Record thread ID so the shutdown path can post WM_QUIT.
    // SAFETY: GetCurrentThreadId is always safe.
    let tid = unsafe { windows::Win32::System::Threading::GetCurrentThreadId() };
    HOOK_THREAD_ID.store(tid, Ordering::SeqCst);

    session_log(&format!(
        "[WIN CAPTURE] Starting hooks on thread {}. Screen={}x{}, centre=({}, {}), edge_threshold={}px, dwell={}ms",
        tid, screen_width, screen_height,
        screen_width / 2, screen_height / 2,
        edge_threshold_px, dwell_ms.load(Ordering::Relaxed),
    ));

    // ── Install system-wide low-level hooks ────────────────────────────────────
    // SAFETY: SetWindowsHookExW is safe when called with a valid function pointer
    // and NULL module handle (required for LL hooks, which run in the hook thread
    // rather than in a DLL).
    let mouse_hook =
        unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_ll_proc), HINSTANCE::default(), 0) };
    let mouse_hook = match mouse_hook {
        Ok(h) => {
            session_log("[WIN CAPTURE] WH_MOUSE_LL installed — capturing ALL pointer devices");
            h
        }
        Err(e) => {
            session_log(&format!(
                "[WIN CAPTURE] FATAL: WH_MOUSE_LL install failed: {:?}",
                e
            ));
            return;
        }
    };

    let kb_hook = unsafe {
        SetWindowsHookExW(
            WH_KEYBOARD_LL,
            Some(keyboard_ll_proc),
            HINSTANCE::default(),
            0,
        )
    };
    let kb_hook = match kb_hook {
        Ok(h) => {
            session_log("[WIN CAPTURE] WH_KEYBOARD_LL installed — capturing ALL keyboard devices");
            h
        }
        Err(e) => {
            session_log(&format!(
                "[WIN CAPTURE] FATAL: WH_KEYBOARD_LL install failed: {:?}",
                e
            ));
            // SAFETY: Unhooking a previously installed valid hook.
            unsafe { UnhookWindowsHookEx(mouse_hook).ok() };
            return;
        }
    };

    session_log("[WIN CAPTURE] Hooks active. Entering message pump.");

    // ── Sync loop: mirror Arc<AtomicBool> → module-level atomics ──────────────
    // The is_remote_active Arc is shared with the edge-handoff handler in
    // main.rs.  We poll it every few iterations so the hook callback always
    // sees the latest ownership state without taking a lock.
    let mut sync_ticker = 0u32;

    // ── Win32 message pump ─────────────────────────────────────────────────────
    // GetMessageW blocks until a message arrives or WM_QUIT is posted.
    // Hook callbacks are dispatched synchronously inside this call.
    let mut msg = MSG::default();
    loop {
        // Check stop flag (set by the shutdown path or Ctrl+C handler).
        if stop.load(Ordering::Relaxed) || CAPTURE_STOP.load(Ordering::Relaxed) {
            break;
        }

        // Periodically mirror the shared Arc atomics into module-level statics
        // so hook callbacks don't have to touch Arc overhead on the hot path.
        sync_ticker = sync_ticker.wrapping_add(1);
        if sync_ticker.is_multiple_of(64) {
            CAPTURE_IS_REMOTE.store(is_remote_active.load(Ordering::Relaxed), Ordering::Relaxed);
            CAPTURE_KEYBOARD_ENABLED
                .store(keyboard_enabled.load(Ordering::Relaxed), Ordering::Relaxed);
            CAPTURE_DWELL_MS.store(dwell_ms.load(Ordering::Relaxed), Ordering::Relaxed);
            let rw = remote_screen_w.load(Ordering::Relaxed);
            let rh = remote_screen_h.load(Ordering::Relaxed);
            CAPTURE_REMOTE_W.store(rw, Ordering::Relaxed);
            CAPTURE_REMOTE_H.store(rh, Ordering::Relaxed);
        }

        // SAFETY: GetMessageW with a null HWND retrieves messages for this thread.
        // Returns > 0 on normal message, 0 on WM_QUIT, -1 on error.
        let ret = unsafe { GetMessageW(&mut msg, HWND::default(), 0, 0) };
        if ret.0 <= 0 {
            // WM_QUIT or error — exit cleanly.
            break;
        }
        // No TranslateMessage / DispatchMessage needed: we only pump for hook
        // delivery.  The hooks are called synchronously inside GetMessageW.
    }

    // ── Clean-up ───────────────────────────────────────────────────────────────
    // Restore cursor visibility if we left it hidden.
    // SAFETY: ShowCursor is always safe.
    unsafe {
        ShowCursor(true);
    }

    // SAFETY: Unhooking valid hook handles.
    unsafe {
        UnhookWindowsHookEx(mouse_hook).ok();
        UnhookWindowsHookEx(kb_hook).ok();
    }

    session_log("[WIN CAPTURE] Hooks removed. Capture loop stopped.");
}

// ── Non-Windows stub ──────────────────────────────────────────────────────────

#[cfg(not(target_os = "windows"))]
pub fn capture_loop(
    _input_tx: tokio::sync::mpsc::Sender<friday_core::InputEvent>,
    _stop: Arc<AtomicBool>,
    _is_remote_active: Arc<AtomicBool>,
    _keyboard_enabled: Arc<AtomicBool>,
    _edge_threshold_px: i32,
    _screen_width: i32,
    _screen_height: i32,
    _edge_trigger_tx: tokio::sync::mpsc::Sender<crate::control::EdgeTrigger>,
    _remote_screen_w: Arc<AtomicU32>,
    _remote_screen_h: Arc<AtomicU32>,
    _dwell_ms: Arc<AtomicU64>,
) {
    tracing::warn!("Windows capture_loop called on non-Windows platform — no-op.");
}

// ── Display query ─────────────────────────────────────────────────────────────

/// Returns the virtual-desktop dimensions via `GetSystemMetrics`.
pub fn query_display_info() -> AgentResult<DisplayBounds> {
    #[cfg(target_os = "windows")]
    {
        // SAFETY: GetSystemMetrics is always safe with valid SM_ constants.
        let w = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) };
        let h = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) };
        Ok(DisplayBounds::new(0, 0, w as u32, h as u32, 1.0, true))
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err(AgentError::DisplayError("Not on Windows".into()))
    }
}

// ── Injection (SendInput with fallback) ───────────────────────────────────────

/// Inject an `InputEvent` into the Windows input system.
///
/// Uses `SendInput` as the primary path (HAL-level, minimal latency).
/// Falls back to `mouse_event` / `keybd_event` for UIPI-elevated windows.
/// Every synthesized event carries `dwExtraInfo = FRIDAY_INJECTED_MAGIC` to
/// prevent hook-feedback loops.
pub fn inject_event(event: &InputEvent) -> AgentResult<()> {
    #[cfg(target_os = "windows")]
    {
        let inputs = build_inputs(event)?;
        if inputs.is_empty() {
            return Ok(());
        }
        // SAFETY: `inputs` is a valid Vec<INPUT>; nInputs matches len().
        let result = unsafe { SendInput(inputs.as_slice(), std::mem::size_of::<INPUT>() as i32) };
        if result == 0 {
            // UIPI fallback
            if let InputEvent::Mouse(mouse_evt) = event {
                // SAFETY: mouse_event is a documented Win32 API with valid parameters.
                unsafe {
                    match mouse_evt {
                        MouseEvent::Button { button, state, .. } => {
                            let flags = match (button, state) {
                                (MouseButton::Left, ElementState::Pressed) => MOUSEEVENTF_LEFTDOWN,
                                (MouseButton::Left, ElementState::Released) => MOUSEEVENTF_LEFTUP,
                                (MouseButton::Right, ElementState::Pressed) => {
                                    MOUSEEVENTF_RIGHTDOWN
                                }
                                (MouseButton::Right, ElementState::Released) => MOUSEEVENTF_RIGHTUP,
                                (MouseButton::Middle, ElementState::Pressed) => {
                                    MOUSEEVENTF_MIDDLEDOWN
                                }
                                (MouseButton::Middle, ElementState::Released) => {
                                    MOUSEEVENTF_MIDDLEUP
                                }
                                _ => return Ok(()),
                            };
                            windows::Win32::UI::Input::KeyboardAndMouse::mouse_event(
                                flags,
                                0,
                                0,
                                0,
                                FRIDAY_INJECTED_MAGIC,
                            );
                            return Ok(());
                        }
                        MouseEvent::MoveRel { dx, dy, .. } => {
                            windows::Win32::UI::Input::KeyboardAndMouse::mouse_event(
                                MOUSEEVENTF_MOVE,
                                *dx as i32,
                                *dy as i32,
                                0,
                                FRIDAY_INJECTED_MAGIC,
                            );
                            return Ok(());
                        }
                        MouseEvent::Scroll { dx, dy, .. } => {
                            if *dy != 0 {
                                let delta = (-(*dy as i32)) * 120;
                                windows::Win32::UI::Input::KeyboardAndMouse::mouse_event(
                                    MOUSEEVENTF_WHEEL,
                                    0,
                                    0,
                                    delta,
                                    FRIDAY_INJECTED_MAGIC,
                                );
                            }
                            if *dx != 0 {
                                let delta = (*dx as i32) * 120;
                                windows::Win32::UI::Input::KeyboardAndMouse::mouse_event(
                                    MOUSEEVENTF_HWHEEL,
                                    0,
                                    0,
                                    delta,
                                    FRIDAY_INJECTED_MAGIC,
                                );
                            }
                            return Ok(());
                        }
                        _ => {}
                    }
                }
            } else if let InputEvent::Keyboard(kb) = event {
                let (vk, flags) = keycode_to_vk_flags(&kb.key, kb.state);
                // SAFETY: MapVirtualKeyW converts VirtualKey to hardware scancode for full application compatibility
                let scan = unsafe {
                    MapVirtualKeyW(
                        vk.0 as u32,
                        windows::Win32::UI::Input::KeyboardAndMouse::MAP_VIRTUAL_KEY_TYPE(0),
                    ) as u8
                };
                // SAFETY: keybd_event is a documented Win32 fallback API.
                unsafe {
                    keybd_event(vk.0 as u8, scan, flags, FRIDAY_INJECTED_MAGIC);
                }
                return Ok(());
            }

            let err = unsafe { windows::Win32::Foundation::GetLastError() };
            return Err(AgentError::InjectionError(format!(
                "SendInput returned 0 — GetLastError={:?}",
                err
            )));
        }
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = event;
        Err(AgentError::InjectionError("Not on Windows".into()))
    }
}

// ── SendInput builders ────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn build_inputs(event: &InputEvent) -> AgentResult<Vec<INPUT>> {
    let _screen_w = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) } as i32;
    let _screen_h = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) } as i32;

    match event {
        InputEvent::Mouse(m) => match m {
            MouseEvent::MoveRel { dx, dy, .. } => Ok(vec![make_mouse_input(
                *dx as i32,
                *dy as i32,
                0,
                MOUSEEVENTF_MOVE,
            )]),
            MouseEvent::MoveAbs { x_norm, y_norm, .. } => Ok(vec![make_mouse_input(
                *x_norm as i32,
                *y_norm as i32,
                0,
                MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
            )]),
            MouseEvent::Button { button, state, .. } => {
                let flags = match (button, state) {
                    (MouseButton::Left, ElementState::Pressed) => MOUSEEVENTF_LEFTDOWN,
                    (MouseButton::Left, ElementState::Released) => MOUSEEVENTF_LEFTUP,
                    (MouseButton::Right, ElementState::Pressed) => MOUSEEVENTF_RIGHTDOWN,
                    (MouseButton::Right, ElementState::Released) => MOUSEEVENTF_RIGHTUP,
                    (MouseButton::Middle, ElementState::Pressed) => MOUSEEVENTF_MIDDLEDOWN,
                    (MouseButton::Middle, ElementState::Released) => MOUSEEVENTF_MIDDLEUP,
                    _ => return Ok(vec![]),
                };
                Ok(vec![make_mouse_input(0, 0, 0, flags)])
            }
            MouseEvent::Scroll { dx, dy, .. } => {
                let mut inputs = Vec::new();
                if *dy != 0 {
                    let delta = (-(*dy as i32)) * 120;
                    inputs.push(make_mouse_input(0, 0, delta, MOUSEEVENTF_WHEEL));
                }
                if *dx != 0 {
                    let delta = (*dx as i32) * 120;
                    inputs.push(make_mouse_input(0, 0, delta, MOUSEEVENTF_HWHEEL));
                }
                Ok(inputs)
            }
        },
        InputEvent::Keyboard(kb) => {
            let (vk, flags) = keycode_to_vk_flags(&kb.key, kb.state);
            // SAFETY: MapVirtualKeyW converts VirtualKey to hardware scancode for full application compatibility
            let scan = unsafe {
                MapVirtualKeyW(
                    vk.0 as u32,
                    windows::Win32::UI::Input::KeyboardAndMouse::MAP_VIRTUAL_KEY_TYPE(0),
                ) as u16
            };
            Ok(vec![make_keyboard_input(vk, scan, flags)])
        }
    }
}

#[cfg(target_os = "windows")]
pub fn keycode_to_vk_flags(key: &KeyCode, state: ElementState) -> (VIRTUAL_KEY, KEYBD_EVENT_FLAGS) {
    let (vk_code, is_extended) = match key {
        KeyCode::Key0 => (0x30u16, false),
        KeyCode::Key1 => (0x31, false),
        KeyCode::Key2 => (0x32, false),
        KeyCode::Key3 => (0x33, false),
        KeyCode::Key4 => (0x34, false),
        KeyCode::Key5 => (0x35, false),
        KeyCode::Key6 => (0x36, false),
        KeyCode::Key7 => (0x37, false),
        KeyCode::Key8 => (0x38, false),
        KeyCode::Key9 => (0x39, false),
        KeyCode::A => (0x41, false),
        KeyCode::B => (0x42, false),
        KeyCode::C => (0x43, false),
        KeyCode::D => (0x44, false),
        KeyCode::E => (0x45, false),
        KeyCode::F => (0x46, false),
        KeyCode::G => (0x47, false),
        KeyCode::H => (0x48, false),
        KeyCode::I => (0x49, false),
        KeyCode::J => (0x4A, false),
        KeyCode::K => (0x4B, false),
        KeyCode::L => (0x4C, false),
        KeyCode::M => (0x4D, false),
        KeyCode::N => (0x4E, false),
        KeyCode::O => (0x4F, false),
        KeyCode::P => (0x50, false),
        KeyCode::Q => (0x51, false),
        KeyCode::R => (0x52, false),
        KeyCode::S => (0x53, false),
        KeyCode::T => (0x54, false),
        KeyCode::U => (0x55, false),
        KeyCode::V => (0x56, false),
        KeyCode::W => (0x57, false),
        KeyCode::X => (0x58, false),
        KeyCode::Y => (0x59, false),
        KeyCode::Z => (0x5A, false),
        KeyCode::F1 => (0x70, false),
        KeyCode::F2 => (0x71, false),
        KeyCode::F3 => (0x72, false),
        KeyCode::F4 => (0x73, false),
        KeyCode::F5 => (0x74, false),
        KeyCode::F6 => (0x75, false),
        KeyCode::F7 => (0x76, false),
        KeyCode::F8 => (0x77, false),
        KeyCode::F9 => (0x78, false),
        KeyCode::F10 => (0x79, false),
        KeyCode::F11 => (0x7A, false),
        KeyCode::F12 => (0x7B, false),
        KeyCode::Backspace => (0x08, false),
        KeyCode::Tab => (0x09, false),
        KeyCode::Enter => (0x0D, false),
        KeyCode::CapsLock => (0x14, false),
        KeyCode::Escape => (0x1B, false),
        KeyCode::Space => (0x20, false),
        KeyCode::Left => (0x25, true),
        KeyCode::Up => (0x26, true),
        KeyCode::Right => (0x27, true),
        KeyCode::Down => (0x28, true),
        KeyCode::LeftShift => (0xA0, false),
        KeyCode::RightShift => (0xA1, false),
        KeyCode::LeftControl => (0xA2, false),
        KeyCode::RightControl => (0xA3, true),
        KeyCode::LeftAlt => (0xA4, false),
        KeyCode::RightAlt => (0xA5, true),
        KeyCode::LeftSuper => (0x5B, false),
        KeyCode::RightSuper => (0x5C, false),
        KeyCode::Char(c) => match c {
            'a'..='z' => (0x41 + (*c as u16 - b'a' as u16), false),
            'A'..='Z' => (0x41 + (*c as u16 - b'A' as u16), false),
            '0'..='9' => (0x30 + (*c as u16 - b'0' as u16), false),
            ' ' => (0x20, false),
            '\n' | '\r' => (0x0D, false),
            '\t' => (0x09, false),
            ';' | ':' => (0xBA, false),
            '=' | '+' => (0xBB, false),
            ',' | '<' => (0xBC, false),
            '-' | '_' => (0xBD, false),
            '.' | '>' => (0xBE, false),
            '/' | '?' => (0xBF, false),
            '`' | '~' => (0xC0, false),
            '[' | '{' => (0xDB, false),
            '\\' | '|' => (0xDC, false),
            ']' | '}' => (0xDD, false),
            '\'' | '"' => (0xDE, false),
            _ => (*c as u16, false),
        },
        KeyCode::Other(code) => (*code as u16, false),
    };

    let mut flags = KEYBD_EVENT_FLAGS(0);
    if is_extended {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    if state == ElementState::Released {
        flags |= KEYEVENTF_KEYUP;
    }
    (VIRTUAL_KEY(vk_code), flags)
}

#[cfg(target_os = "windows")]
fn make_keyboard_input(vk: VIRTUAL_KEY, scan: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: FRIDAY_INJECTED_MAGIC,
            },
        },
    }
}

#[cfg(target_os = "windows")]
fn make_mouse_input(
    dx: i32,
    dy: i32,
    mouse_data: i32,
    flags: windows::Win32::UI::Input::KeyboardAndMouse::MOUSE_EVENT_FLAGS,
) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx,
                dy,
                mouseData: mouse_data as u32,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: FRIDAY_INJECTED_MAGIC,
            },
        },
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use friday_core::KeyboardEvent;

    // ── Injection tests (can run on Windows CI without hardware) ──────────────

    #[test]
    fn test_build_mouse_move_input() {
        let evt = InputEvent::Mouse(MouseEvent::MoveRel {
            dx: 10,
            dy: -5,
            timestamp: 0,
        });
        let inputs = build_inputs(&evt).expect("build_inputs failed");
        assert_eq!(inputs.len(), 1);
    }

    #[test]
    fn test_build_mouse_button_input() {
        let evt = InputEvent::Mouse(MouseEvent::Button {
            button: MouseButton::Left,
            state: ElementState::Pressed,
            timestamp: 0,
        });
        let inputs = build_inputs(&evt).expect("build_inputs failed");
        assert_eq!(inputs.len(), 1);
    }

    #[test]
    fn test_build_keyboard_input() {
        let evt = InputEvent::Keyboard(KeyboardEvent {
            key: KeyCode::A,
            state: ElementState::Pressed,
            timestamp: 0,
        });
        let inputs = build_inputs(&evt).expect("build_inputs failed");
        assert_eq!(inputs.len(), 1);
    }

    #[test]
    fn test_build_escape_input() {
        // Escape routes like any other key — not intercepted, not suppressed.
        let evt = InputEvent::Keyboard(KeyboardEvent {
            key: KeyCode::Escape,
            state: ElementState::Released,
            timestamp: 0,
        });
        let inputs = build_inputs(&evt).expect("build_inputs failed");
        assert_eq!(inputs.len(), 1);
    }

    #[test]
    fn test_inject_zero_move_is_safe() {
        let evt = InputEvent::Mouse(MouseEvent::MoveRel {
            dx: 0,
            dy: 0,
            timestamp: 0,
        });
        assert!(inject_event(&evt).is_ok());
    }

    #[test]
    fn test_inject_button_release_is_safe() {
        let evt = InputEvent::Mouse(MouseEvent::Button {
            button: MouseButton::Left,
            state: ElementState::Released,
            timestamp: 0,
        });
        assert!(inject_event(&evt).is_ok());
    }

    #[test]
    fn test_inject_keyboard_release_is_safe() {
        let evt = InputEvent::Keyboard(KeyboardEvent {
            key: KeyCode::A,
            state: ElementState::Released,
            timestamp: 0,
        });
        assert!(inject_event(&evt).is_ok());
    }

    // ── VK → KeyCode mapping tests ────────────────────────────────────────────

    #[test]
    fn test_vk_to_keycode_letter_range() {
        assert_eq!(vk_to_keycode(0x41), Some(KeyCode::A));
        assert_eq!(vk_to_keycode(0x5A), Some(KeyCode::Z));
    }

    #[test]
    fn test_vk_to_keycode_digit_range() {
        assert_eq!(vk_to_keycode(0x30), Some(KeyCode::Key0));
        assert_eq!(vk_to_keycode(0x39), Some(KeyCode::Key9));
    }

    #[test]
    fn test_vk_to_keycode_fn_keys() {
        assert_eq!(vk_to_keycode(0x70), Some(KeyCode::F1));
        assert_eq!(vk_to_keycode(0x7B), Some(KeyCode::F12));
    }

    #[test]
    fn test_vk_to_keycode_modifiers() {
        assert_eq!(vk_to_keycode(0xA0), Some(KeyCode::LeftShift));
        assert_eq!(vk_to_keycode(0xA1), Some(KeyCode::RightShift));
        assert_eq!(vk_to_keycode(0xA2), Some(KeyCode::LeftControl));
        assert_eq!(vk_to_keycode(0xA3), Some(KeyCode::RightControl));
        assert_eq!(vk_to_keycode(0xA4), Some(KeyCode::LeftAlt));
        assert_eq!(vk_to_keycode(0xA5), Some(KeyCode::RightAlt));
    }

    #[test]
    fn test_vk_to_keycode_escape_not_suppressed() {
        // Escape must map to a real KeyCode — it is NOT filtered out.
        // Emergency escape is an application-level mechanism, not hook interception.
        assert_eq!(vk_to_keycode(0x1B), Some(KeyCode::Escape));
    }

    #[test]
    fn test_vk_to_keycode_special_keys() {
        assert_eq!(vk_to_keycode(0x0D), Some(KeyCode::Enter));
        assert_eq!(vk_to_keycode(0x20), Some(KeyCode::Space));
        assert_eq!(vk_to_keycode(0x08), Some(KeyCode::Backspace));
        assert_eq!(vk_to_keycode(0x09), Some(KeyCode::Tab));
    }

    #[test]
    fn test_vk_to_keycode_arrow_keys() {
        assert_eq!(vk_to_keycode(0x25), Some(KeyCode::Left));
        assert_eq!(vk_to_keycode(0x26), Some(KeyCode::Up));
        assert_eq!(vk_to_keycode(0x27), Some(KeyCode::Right));
        assert_eq!(vk_to_keycode(0x28), Some(KeyCode::Down));
    }

    // ── Edge detection unit tests ─────────────────────────────────────────────

    #[test]
    fn test_detect_edge_left() {
        assert_eq!(
            detect_edge_win(2, 400, 1920, 1080, 3),
            Some(ScreenEdge::Left)
        );
    }

    #[test]
    fn test_detect_edge_right() {
        assert_eq!(
            detect_edge_win(1918, 400, 1920, 1080, 3),
            Some(ScreenEdge::Right)
        );
    }

    #[test]
    fn test_detect_edge_top() {
        assert_eq!(
            detect_edge_win(960, 1, 1920, 1080, 3),
            Some(ScreenEdge::Top)
        );
    }

    #[test]
    fn test_detect_edge_bottom() {
        assert_eq!(
            detect_edge_win(960, 1078, 1920, 1080, 3),
            Some(ScreenEdge::Bottom)
        );
    }

    #[test]
    fn test_detect_edge_centre_is_none() {
        assert_eq!(detect_edge_win(960, 540, 1920, 1080, 3), None);
    }

    // ── FRIDAY_INJECTED_MAGIC sentinel ────────────────────────────────────────

    #[test]
    fn test_injected_magic_value_stable() {
        // The magic value must never change — it is the loop-prevention contract.
        assert_eq!(FRIDAY_INJECTED_MAGIC, 0x46524944); // "FRID" in ASCII
    }
}
