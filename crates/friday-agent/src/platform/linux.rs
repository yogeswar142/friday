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
pub fn capture_loop(
    tx: mpsc::Sender<InputEvent>,
    stop: Arc<AtomicBool>,
    is_remote_active: Arc<AtomicBool>,
    edge_threshold_px: i32,
    screen_width: i32,
    screen_height: i32,
    edge_trigger_tx: mpsc::Sender<EdgeTrigger>,
) {
    let display = unsafe { x11::xlib::XOpenDisplay(std::ptr::null()) };
    if display.is_null() {
        tracing::error!("capture_loop: failed to open X display");
        return;
    }

    let screen = unsafe { x11::xlib::XDefaultScreen(display) };
    let root = unsafe { x11::xlib::XRootWindow(display, screen) };

    let center_x = screen_width / 2;
    let center_y = screen_height / 2;

    let mut last_g50_x = center_x;
    let mut last_g50_y = center_y;
    let mut was_remote = false;

    // Track remote (Yoga) virtual cursor position
    let remote_w = 1280.0f32;
    let remote_h = 800.0f32;
    let mut yoga_x = remote_w / 2.0;
    let mut yoga_y = remote_h / 2.0;

    let poll_interval = Duration::from_millis(2); // 500 Hz

    while !stop.load(Ordering::Relaxed) {
        let loop_start = Instant::now();
        let remote_active = is_remote_active.load(Ordering::Relaxed);
        let ts = timestamp_ms();

        // ── State Transition: Local -> Remote ────────────────────────────────
        if remote_active && !was_remote {
            unsafe {
                // Hide cursor on G50 while controlling Yoga
                x11::xfixes::XFixesHideCursor(display, root);
                // Grab pointer so no clicks/scroll leak to G50 windows
                x11::xlib::XGrabPointer(
                    display,
                    root,
                    0,
                    (x11::xlib::PointerMotionMask
                        | x11::xlib::ButtonPressMask
                        | x11::xlib::ButtonReleaseMask) as u32,
                    x11::xlib::GrabModeAsync,
                    x11::xlib::GrabModeAsync,
                    0,
                    0,
                    x11::xlib::CurrentTime,
                );
                // Warp pointer to center to allow infinite relative travel
                x11::xlib::XWarpPointer(display, 0, root, 0, 0, 0, 0, center_x, center_y);
                x11::xlib::XFlush(display);
            }
            was_remote = true;
            tracing::info!(">>> Active on Yoga: G50 cursor hidden, pointer grabbed");
        }
        // ── State Transition: Remote -> Local ────────────────────────────────
        else if !remote_active && was_remote {
            unsafe {
                // Ungrab pointer and restore G50 cursor
                x11::xlib::XUngrabPointer(display, x11::xlib::CurrentTime);
                x11::xfixes::XFixesShowCursor(display, root);
                // Restore G50 cursor to where it left
                x11::xlib::XWarpPointer(display, 0, root, 0, 0, 0, 0, last_g50_x, last_g50_y);
                x11::xlib::XFlush(display);
            }
            was_remote = false;
            tracing::info!("<<< Active on G50: cursor restored at ({}, {})", last_g50_x, last_g50_y);
        }

        // ── Processing while REMOTE ACTIVE ───────────────────────────────────
        if remote_active {
            // Drain X events for clicks, scroll, and motion while grabbed
            unsafe {
                while x11::xlib::XPending(display) > 0 {
                    let mut event: x11::xlib::XEvent = std::mem::zeroed();
                    x11::xlib::XNextEvent(display, &mut event);

                    match event.get_type() {
                        x11::xlib::ButtonPress => {
                            let btn_event = event.button;
                            let btn_num = btn_event.button;

                            // Buttons 4 & 5 are vertical scroll, 6 & 7 horizontal scroll
                            if btn_num == 4 {
                                let _ = tx.try_send(InputEvent::Mouse(MouseEvent::Scroll {
                                    dx: 0,
                                    dy: -1, // scroll up
                                    timestamp: ts,
                                }));
                            } else if btn_num == 5 {
                                let _ = tx.try_send(InputEvent::Mouse(MouseEvent::Scroll {
                                    dx: 0,
                                    dy: 1, // scroll down
                                    timestamp: ts,
                                }));
                            } else if btn_num == 6 {
                                let _ = tx.try_send(InputEvent::Mouse(MouseEvent::Scroll {
                                    dx: -1,
                                    dy: 0,
                                    timestamp: ts,
                                }));
                            } else if btn_num == 7 {
                                let _ = tx.try_send(InputEvent::Mouse(MouseEvent::Scroll {
                                    dx: 1,
                                    dy: 0,
                                    timestamp: ts,
                                }));
                            } else {
                                let friday_btn = match btn_num {
                                    1 => MouseButton::Left,
                                    2 => MouseButton::Middle,
                                    3 => MouseButton::Right,
                                    other => MouseButton::Other(other as u8),
                                };
                                let _ = tx.try_send(InputEvent::Mouse(MouseEvent::Button {
                                    button: friday_btn,
                                    state: ElementState::Pressed,
                                    timestamp: ts,
                                }));
                            }
                        }

                        x11::xlib::ButtonRelease => {
                            let btn_event = event.button;
                            let btn_num = btn_event.button;
                            if btn_num <= 3 || btn_num > 7 {
                                let friday_btn = match btn_num {
                                    1 => MouseButton::Left,
                                    2 => MouseButton::Middle,
                                    3 => MouseButton::Right,
                                    other => MouseButton::Other(other as u8),
                                };
                                let _ = tx.try_send(InputEvent::Mouse(MouseEvent::Button {
                                    button: friday_btn,
                                    state: ElementState::Released,
                                    timestamp: ts,
                                }));
                            }
                        }

                        x11::xlib::MotionNotify => {
                            let motion = event.motion;
                            let dx = motion.x_root - center_x;
                            let dy = motion.y_root - center_y;

                            if dx != 0 || dy != 0 {
                                // Update virtual Yoga position
                                yoga_x += dx as f32;
                                yoga_y += dy as f32;

                                // Send relative movement to Yoga
                                let _ = tx.try_send(InputEvent::Mouse(MouseEvent::MoveRel {
                                    dx: dx.clamp(i16::MIN as i32, i16::MAX as i32) as i16,
                                    dy: dy.clamp(i16::MIN as i32, i16::MAX as i32) as i16,
                                    timestamp: ts,
                                }));

                                // Check if Yoga virtual cursor reached ANY edge -> return to G50!
                                if yoga_x <= 0.0 || yoga_x >= remote_w - 1.0 || yoga_y <= 0.0 || yoga_y >= remote_h - 1.0 {
                                    tracing::info!("Yoga edge reached at ({:.0}, {:.0}) -> Wrapping back to G50!", yoga_x, yoga_y);
                                    yoga_x = yoga_x.clamp(5.0, remote_w - 6.0);
                                    yoga_y = yoga_y.clamp(5.0, remote_h - 6.0);
                                    is_remote_active.store(false, Ordering::SeqCst);
                                }

                                // Warp pointer back to center to prepare for next delta
                                x11::xlib::XWarpPointer(display, 0, root, 0, 0, 0, 0, center_x, center_y);
                                x11::xlib::XFlush(display);
                            }
                        }

                        _ => {}
                    }
                }
            }
        }
        // ── Processing while LOCAL ACTIVE ────────────────────────────────────
        else {
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
            }

            // Check if cursor reached any edge of G50 screen
            if let Some(edge) = detect_edge(root_x, root_y, screen_width, screen_height, edge_threshold_px) {
                last_g50_x = root_x.clamp(10, screen_width - 11);
                last_g50_y = root_y.clamp(10, screen_height - 11);

                // Set Yoga entry position based on edge of exit
                match edge {
                    ScreenEdge::Right => {
                        yoga_x = 5.0;
                        yoga_y = (root_y as f32 / screen_height as f32) * remote_h;
                    }
                    ScreenEdge::Left => {
                        yoga_x = remote_w - 6.0;
                        yoga_y = (root_y as f32 / screen_height as f32) * remote_h;
                    }
                    ScreenEdge::Top => {
                        yoga_x = (root_x as f32 / screen_width as f32) * remote_w;
                        yoga_y = remote_h - 6.0;
                    }
                    ScreenEdge::Bottom => {
                        yoga_x = (root_x as f32 / screen_width as f32) * remote_w;
                        yoga_y = 5.0;
                    }
                }

                let _ = edge_trigger_tx.try_send(EdgeTrigger {
                    edge,
                    norm_x: root_x as f32 / screen_width as f32,
                    norm_y: root_y as f32 / screen_height as f32,
                });

                tracing::info!("G50 edge {:?} reached at ({}, {}) -> Transferring to Yoga at ({:.0}, {:.0})!", edge, root_x, root_y, yoga_x, yoga_y);
                is_remote_active.store(true, Ordering::SeqCst);
            }
        }

        let elapsed = loop_start.elapsed();
        if elapsed < poll_interval {
            std::thread::sleep(poll_interval - elapsed);
        }
    }

    // Clean exit: restore cursor and ungrab
    unsafe {
        if was_remote {
            x11::xlib::XUngrabPointer(display, x11::xlib::CurrentTime);
            x11::xfixes::XFixesShowCursor(display, root);
            x11::xlib::XFlush(display);
        }
        x11::xlib::XCloseDisplay(display);
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
