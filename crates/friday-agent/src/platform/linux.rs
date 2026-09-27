/// Linux X11 real input capture using XQueryPointer polling + XTest injection
/// XTest extension: injects synthetic input events into the X server event queue.
///
/// Capture strategy:
///   We poll XQueryPointer at high frequency (~500 Hz) to track mouse position
///   and use XRecord or /dev/input event reading for button events.
///   This avoids the need for grab and works without `input` group membership.
///
/// Injection strategy:
///   XTestFakeMotionEvent for absolute cursor placement
///   XTestFakeButtonEvent for mouse button press/release
///   XTestFakeRelativeMotionEvent for relative move injection

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;

use friday_core::{DisplayBounds, ElementState, InputEvent, MouseButton, MouseEvent};

use crate::control::{EdgeTrigger, ScreenEdge};
use crate::error::{AgentError, Result as AgentResult};

fn timestamp_ms() -> u32 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u32
}

/// Query real display resolution via xrandr-style Xlib calls
pub fn query_display_info() -> AgentResult<DisplayBounds> {
    // SAFETY: XOpenDisplay is safe to call; returns null on failure
    let display = unsafe { x11::xlib::XOpenDisplay(std::ptr::null()) };
    if display.is_null() {
        return Err(AgentError::DisplayError(
            "Failed to open X display. Is DISPLAY set?".into(),
        ));
    }
    let screen = unsafe { x11::xlib::XDefaultScreen(display) };
    let width = unsafe { x11::xlib::XDisplayWidth(display, screen) } as u32;
    let height = unsafe { x11::xlib::XDisplayHeight(display, screen) } as u32;
    let width_mm = unsafe { x11::xlib::XDisplayWidthMM(display, screen) } as f32;

    // Approximate DPI from physical display width
    let dpi = if width_mm > 0.0 {
        (width as f32 / width_mm) * 25.4
    } else {
        96.0
    };
    let scale_factor = (dpi / 96.0).max(1.0);

    unsafe { x11::xlib::XCloseDisplay(display) };

    Ok(DisplayBounds::new(0, 0, width, height, scale_factor, true))
}

/// Get current cursor position from X server
pub fn get_cursor_position() -> AgentResult<(i32, i32)> {
    // SAFETY: safe Xlib call sequence
    let display = unsafe { x11::xlib::XOpenDisplay(std::ptr::null()) };
    if display.is_null() {
        return Err(AgentError::DisplayError("Cannot open X display".into()));
    }

    let screen = unsafe { x11::xlib::XDefaultScreen(display) };
    let root = unsafe { x11::xlib::XRootWindow(display, screen) };

    let mut root_ret = 0u64;
    let mut child_ret = 0u64;
    let mut root_x = 0i32;
    let mut root_y = 0i32;
    let mut win_x = 0i32;
    let mut win_y = 0i32;
    let mut mask = 0u32;

    // SAFETY: XQueryPointer is standard Xlib; all pointers are valid stack variables
    unsafe {
        x11::xlib::XQueryPointer(
            display,
            root,
            &mut root_ret,
            &mut child_ret,
            &mut root_x,
            &mut root_y,
            &mut win_x,
            &mut win_y,
            &mut mask,
        );
        x11::xlib::XCloseDisplay(display);
    }

    Ok((root_x, root_y))
}

/// Inject a mouse movement (absolute position) via XTest
pub fn inject_move_abs(x: i32, y: i32) -> AgentResult<()> {
    // SAFETY: XTest is standard extension; display pointer valid within scope
    let display = unsafe { x11::xlib::XOpenDisplay(std::ptr::null()) };
    if display.is_null() {
        return Err(AgentError::InjectionError("Cannot open X display".into()));
    }
    let screen = unsafe { x11::xlib::XDefaultScreen(display) };
    unsafe {
        x11::xtest::XTestFakeMotionEvent(display, screen, x, y, 0);
        x11::xlib::XFlush(display);
        x11::xlib::XCloseDisplay(display);
    }
    Ok(())
}

/// Inject a relative mouse movement via XTest
pub fn inject_move_rel(dx: i32, dy: i32) -> AgentResult<()> {
    // SAFETY: same as inject_move_abs
    let display = unsafe { x11::xlib::XOpenDisplay(std::ptr::null()) };
    if display.is_null() {
        return Err(AgentError::InjectionError("Cannot open X display".into()));
    }
    unsafe {
        x11::xtest::XTestFakeRelativeMotionEvent(display, dx, dy, 0, 0);
        x11::xlib::XFlush(display);
        x11::xlib::XCloseDisplay(display);
    }
    Ok(())
}

fn mouse_button_to_x11(button: MouseButton) -> u32 {
    match button {
        MouseButton::Left => 1,
        MouseButton::Middle => 2,
        MouseButton::Right => 3,
        MouseButton::Back => 8,
        MouseButton::Forward => 9,
        MouseButton::Other(n) => n as u32,
    }
}

/// Inject a mouse button event via XTest
pub fn inject_button(button: MouseButton, pressed: bool) -> AgentResult<()> {
    // SAFETY: XTestFakeButtonEvent is standard; display pointer valid within scope
    let display = unsafe { x11::xlib::XOpenDisplay(std::ptr::null()) };
    if display.is_null() {
        return Err(AgentError::InjectionError("Cannot open X display".into()));
    }
    let x11_btn = mouse_button_to_x11(button);
    unsafe {
        x11::xtest::XTestFakeButtonEvent(display, x11_btn, pressed as i32, 0);
        x11::xlib::XFlush(display);
        x11::xlib::XCloseDisplay(display);
    }
    Ok(())
}

/// Inject scroll wheel events (button 4=up, 5=down, 6=left, 7=right)
pub fn inject_scroll(dx: i16, dy: i16) -> AgentResult<()> {
    let display = unsafe { x11::xlib::XOpenDisplay(std::ptr::null()) };
    if display.is_null() {
        return Err(AgentError::InjectionError("Cannot open X display".into()));
    }
    unsafe {
        // Vertical scroll: button 4 (up) or 5 (down)
        if dy != 0 {
            let btn = if dy < 0 { 4u32 } else { 5u32 };
            let count = dy.unsigned_abs() as u32;
            for _ in 0..count {
                x11::xtest::XTestFakeButtonEvent(display, btn, 1, 0);
                x11::xtest::XTestFakeButtonEvent(display, btn, 0, 0);
            }
        }
        // Horizontal scroll: button 6 (left) or 7 (right)
        if dx != 0 {
            let btn = if dx < 0 { 6u32 } else { 7u32 };
            let count = dx.unsigned_abs() as u32;
            for _ in 0..count {
                x11::xtest::XTestFakeButtonEvent(display, btn, 1, 0);
                x11::xtest::XTestFakeButtonEvent(display, btn, 0, 0);
            }
        }
        x11::xlib::XFlush(display);
        x11::xlib::XCloseDisplay(display);
    }
    Ok(())
}

/// Inject any InputEvent received from network into the local X display
pub fn inject_event(event: &InputEvent) -> AgentResult<()> {
    match event {
        InputEvent::Mouse(mouse_evt) => match mouse_evt {
            MouseEvent::MoveRel { dx, dy, .. } => inject_move_rel(*dx as i32, *dy as i32),
            MouseEvent::MoveAbs { x_norm, y_norm, .. } => {
                // Convert normalized to pixels using actual display size
                let info = query_display_info()?;
                let px = (*x_norm as f32 / 65535.0 * (info.width as f32 - 1.0)).round() as i32;
                let py = (*y_norm as f32 / 65535.0 * (info.height as f32 - 1.0)).round() as i32;
                inject_move_abs(px, py)
            }
            MouseEvent::Button { button, state, .. } => {
                inject_button(*button, *state == ElementState::Pressed)
            }
            MouseEvent::Scroll { dx, dy, .. } => inject_scroll(*dx, *dy),
        },
        InputEvent::Keyboard(_) => {
            // Phase 1 only: keyboard not yet injected
            Ok(())
        }
    }
}

/// High-frequency cursor capture task using XQueryPointer polling.
///
/// Captures at ~500 Hz interval (2ms). Emits:
///   - MoveRel when cursor position changes
///   - Button events by tracking mask changes
///
/// Returns when `stop` is set to true.
pub async fn capture_loop(
    tx: mpsc::Sender<InputEvent>,
    stop: Arc<AtomicBool>,
    edge_threshold_px: i32,
    screen_width: i32,
    screen_height: i32,
    edge_trigger_tx: mpsc::Sender<EdgeTrigger>,
) {
    let mut last_x = 0i32;
    let mut last_y = 0i32;
    let mut last_mask = 0u32;
    let mut first = true;

    // Initialize position
    if let Ok((x, y)) = get_cursor_position() {
        last_x = x;
        last_y = y;
    }

    let poll_interval = Duration::from_millis(2); // 500 Hz

    while !stop.load(Ordering::Relaxed) {
        let loop_start = Instant::now();

        // SAFETY: open/close display per iteration is safe; XQueryPointer state returned by value
        let display = unsafe { x11::xlib::XOpenDisplay(std::ptr::null()) };
        if display.is_null() {
            tokio::time::sleep(Duration::from_millis(100)).await;
            continue;
        }

        let screen = unsafe { x11::xlib::XDefaultScreen(display) };
        let root = unsafe { x11::xlib::XRootWindow(display, screen) };

        let mut root_ret = 0u64;
        let mut child_ret = 0u64;
        let mut root_x = 0i32;
        let mut root_y = 0i32;
        let mut win_x = 0i32;
        let mut win_y = 0i32;
        let mut mask = 0u32;

        unsafe {
            x11::xlib::XQueryPointer(
                display,
                root,
                &mut root_ret,
                &mut child_ret,
                &mut root_x,
                &mut root_y,
                &mut win_x,
                &mut win_y,
                &mut mask,
            );
            x11::xlib::XCloseDisplay(display);
        }

        let ts = timestamp_ms();

        if first {
            last_x = root_x;
            last_y = root_y;
            last_mask = mask;
            first = false;
        } else {
            // Movement delta
            let dx = root_x - last_x;
            let dy = root_y - last_y;
            if dx != 0 || dy != 0 {
                // Check edge trigger BEFORE sending movement
                let trigger = detect_edge(root_x, root_y, screen_width, screen_height, edge_threshold_px);
                if let Some(edge) = trigger {
                    // Send edge trigger signal
                    let _ = edge_trigger_tx.try_send(EdgeTrigger {
                        edge,
                        norm_x: root_x as f32 / screen_width as f32,
                        norm_y: root_y as f32 / screen_height as f32,
                    });
                    // Clamp cursor back from edge to prevent OS edge effects
                } else {
                    // Normal movement — send relative event
                    let evt = InputEvent::Mouse(MouseEvent::MoveRel {
                        dx: dx.clamp(i16::MIN as i32, i16::MAX as i32) as i16,
                        dy: dy.clamp(i16::MIN as i32, i16::MAX as i32) as i16,
                        timestamp: ts,
                    });
                    let _ = tx.try_send(evt);
                }

                last_x = root_x;
                last_y = root_y;
            }

            // Detect button state changes via bitmask comparison
            // X11 button masks: Button1=256, Button2=512, Button3=1024, Button4=2048(scroll), Button5=4096(scroll)
            for (xbtn, friday_btn) in [
                (1u32 << 8, MouseButton::Left),
                (1u32 << 9, MouseButton::Middle),
                (1u32 << 10, MouseButton::Right),
            ] {
                let was_pressed = (last_mask & xbtn) != 0;
                let is_pressed = (mask & xbtn) != 0;
                if was_pressed != is_pressed {
                    let state = if is_pressed {
                        ElementState::Pressed
                    } else {
                        ElementState::Released
                    };
                    let evt = InputEvent::Mouse(MouseEvent::Button {
                        button: friday_btn,
                        state,
                        timestamp: ts,
                    });
                    let _ = tx.try_send(evt);
                }
            }

            last_mask = mask;
        }

        // Maintain target poll frequency
        let elapsed = loop_start.elapsed();
        if elapsed < poll_interval {
            tokio::time::sleep(poll_interval - elapsed).await;
        }
    }
}

fn detect_edge(
    x: i32,
    y: i32,
    screen_w: i32,
    screen_h: i32,
    threshold: i32,
) -> Option<ScreenEdge> {
    if x <= threshold {
        Some(ScreenEdge::Left)
    } else if x >= screen_w - 1 - threshold {
        Some(ScreenEdge::Right)
    } else if y <= threshold {
        Some(ScreenEdge::Top)
    } else if y >= screen_h - 1 - threshold {
        Some(ScreenEdge::Bottom)
    } else {
        None
    }
}
