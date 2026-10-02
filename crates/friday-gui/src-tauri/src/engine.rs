/// FRIDAY Mouse Topology & Input Sharing Engine
///
/// Handles real-time UDP input reception, OS input injection,
/// cursor edge detection against circular topology, and handoff between machines.
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

use friday_agent::control::ControlMessage;
use friday_core::{
    CircularTopology, Edge, ElementState, InputEvent, MouseButton, MouseEvent, NormalizedPoint,
};
use friday_network::{NetworkPacket, PacketPayload};

use crate::state::SharedAppState;

static IS_CONTROLLING_REMOTE: AtomicBool = AtomicBool::new(false);
static HOOK_EVENT_TX: OnceLock<std::sync::mpsc::Sender<InputEvent>> = OnceLock::new();
static LAST_HOOK_X: AtomicI32 = AtomicI32::new(0);
static LAST_HOOK_Y: AtomicI32 = AtomicI32::new(0);
static HAS_LAST_HOOK_PT: AtomicBool = AtomicBool::new(false);
static FREEZE_CURSOR_X: AtomicI32 = AtomicI32::new(-1);
static FREEZE_CURSOR_Y: AtomicI32 = AtomicI32::new(-1);
static IS_SELF_SETTING_CURSOR: AtomicBool = AtomicBool::new(false);

pub const FRIDAY_INJECTED_MAGIC: usize = 0x46524944; // "FRID"

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, POINT, WPARAM};
#[cfg(target_os = "windows")]
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_ESCAPE};
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetCursorPos, GetSystemMetrics, PeekMessageW, SetCursorPos,
    SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, MSG, MSLLHOOKSTRUCT, PM_REMOVE,
    SM_CXSCREEN, SM_CYSCREEN, WH_MOUSE_LL, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN,
    WM_MBUTTONUP, WM_MOUSEHWHEEL, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_RBUTTONDOWN, WM_RBUTTONUP,
};

// ── Native platform cursor & screen helpers ──────────────────────────────────

pub fn get_screen_dimensions() -> (i32, i32) {
    #[cfg(target_os = "windows")]
    {
        // SAFETY: Querying standard system metrics for virtual or primary desktop width & height
        let w = unsafe { GetSystemMetrics(SM_CXSCREEN) };
        let h = unsafe { GetSystemMetrics(SM_CYSCREEN) };
        if w > 0 && h > 0 {
            return (w, h);
        }
        (1920, 1080)
    }
    #[cfg(target_os = "linux")]
    {
        friday_agent::platform::linux::query_display_info()
            .map(|d| (d.width as i32, d.height as i32))
            .unwrap_or((1920, 1080))
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        (1920, 1080)
    }
}

pub fn get_local_cursor_pos() -> Option<(i32, i32)> {
    #[cfg(target_os = "windows")]
    {
        let mut pt = POINT { x: 0, y: 0 };
        // SAFETY: pt is stack-allocated and valid for GetCursorPos
        if unsafe { GetCursorPos(&mut pt) }.is_ok() {
            Some((pt.x, pt.y))
        } else {
            None
        }
    }
    #[cfg(target_os = "linux")]
    {
        friday_agent::platform::linux::get_cursor_position().ok()
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        None
    }
}

pub fn set_local_cursor_pos(x: i32, y: i32) {
    #[cfg(target_os = "windows")]
    {
        IS_SELF_SETTING_CURSOR.store(true, Ordering::SeqCst);
        // SAFETY: SetCursorPos is safe with integer screen coordinates
        unsafe {
            let _ = SetCursorPos(x, y);
        }
        IS_SELF_SETTING_CURSOR.store(false, Ordering::SeqCst);
    }
    #[cfg(target_os = "linux")]
    {
        let _ = friday_agent::platform::linux::inject_move_abs(x, y);
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        let _ = (x, y);
    }
}

pub fn inject_os_event(event: &InputEvent) {
    match event {
        InputEvent::Mouse(MouseEvent::MoveRel { dx, dy, .. }) => {
            if let Some((cur_x, cur_y)) = get_local_cursor_pos() {
                let (screen_w, screen_h) = get_screen_dimensions();
                let new_x = (cur_x + *dx as i32).clamp(0, screen_w - 1);
                let new_y = (cur_y + *dy as i32).clamp(0, screen_h - 1);
                set_local_cursor_pos(new_x, new_y);
            }
        }
        InputEvent::Mouse(MouseEvent::MoveAbs { x_norm, y_norm, .. }) => {
            let (screen_w, screen_h) = get_screen_dimensions();
            let new_x = (((*x_norm as f32) / 65535.0) * (screen_w - 1) as f32).round() as i32;
            let new_y = (((*y_norm as f32) / 65535.0) * (screen_h - 1) as f32).round() as i32;
            set_local_cursor_pos(new_x.clamp(0, screen_w - 1), new_y.clamp(0, screen_h - 1));
        }
        other => {
            #[cfg(target_os = "windows")]
            {
                let _ = friday_agent::platform::windows::inject_event(other);
            }
            #[cfg(target_os = "linux")]
            {
                let _ = friday_agent::platform::linux::inject_event(other);
            }
            #[cfg(not(any(target_os = "windows", target_os = "linux")))]
            {
                let _ = other;
            }
        }
    }
}

// ── Windows Low-Level Mouse Hook ──────────────────────────────────────────────

#[cfg(target_os = "windows")]
unsafe extern "system" fn low_level_mouse_proc(
    n_code: i32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if n_code >= 0 {
        // Emergency escape: check if Escape key is pressed while controlling remote
        if GetAsyncKeyState(VK_ESCAPE.0 as i32) < 0 {
            FREEZE_CURSOR_X.store(-1, Ordering::Relaxed);
            FREEZE_CURSOR_Y.store(-1, Ordering::Relaxed);
            IS_CONTROLLING_REMOTE.store(false, Ordering::SeqCst);
            return CallNextHookEx(None, n_code, w_param, l_param);
        }

        if IS_CONTROLLING_REMOTE.load(Ordering::Relaxed) {
            let info = *(l_param.0 as *const MSLLHOOKSTRUCT);

            // Filter out FRIDAY's own programmatic events and self-setting cursor actions.
            // NOTE: We MUST NOT filter by (info.flags & 1) != 0 because laptop trackpads /
            // touchpads (Synaptics, ELAN, Precision Touchpad) frequently report their input
            // via driver-injected events (bit 0 = 1). Checking dwExtraInfo and IS_SELF_SETTING_CURSOR
            // ensures physical mouse AND laptop touchpads work flawlessly!
            if IS_SELF_SETTING_CURSOR.load(Ordering::SeqCst)
                || info.dwExtraInfo == FRIDAY_INJECTED_MAGIC
            {
                return CallNextHookEx(None, n_code, w_param, l_param);
            }

            let msg = w_param.0 as u32;

            match msg {
                WM_MOUSEMOVE => {
                    let fx = FREEZE_CURSOR_X.load(Ordering::Relaxed);
                    let fy = FREEZE_CURSOR_Y.load(Ordering::Relaxed);

                    // If this event was triggered by our own freeze reposition, consume without sending delta
                    if fx >= 0 && fy >= 0 && info.pt.x == fx && info.pt.y == fy {
                        return LRESULT(1);
                    }

                    let dx;
                    let dy;

                    if fx >= 0 && fy >= 0 {
                        // Delta is movement relative to the frozen host anchor position (works for both physical mouse AND laptop touchpad!)
                        dx = info.pt.x - fx;
                        dy = info.pt.y - fy;

                        // Lock cursor back at the freeze anchor
                        set_local_cursor_pos(fx, fy);
                    } else if !HAS_LAST_HOOK_PT.swap(true, Ordering::SeqCst) {
                        LAST_HOOK_X.store(info.pt.x, Ordering::Relaxed);
                        LAST_HOOK_Y.store(info.pt.y, Ordering::Relaxed);
                        dx = 0;
                        dy = 0;
                    } else {
                        let prev_x = LAST_HOOK_X.swap(info.pt.x, Ordering::Relaxed);
                        let prev_y = LAST_HOOK_Y.swap(info.pt.y, Ordering::Relaxed);
                        dx = info.pt.x - prev_x;
                        dy = info.pt.y - prev_y;
                    }

                    if dx != 0 || dy != 0 {
                        if let Some(tx) = HOOK_EVENT_TX.get() {
                            let _ = tx.send(InputEvent::Mouse(MouseEvent::MoveRel {
                                dx: dx.clamp(i16::MIN as i32, i16::MAX as i32) as i16,
                                dy: dy.clamp(i16::MIN as i32, i16::MAX as i32) as i16,
                                timestamp: 0,
                            }));
                        }
                    }

                    // Consume the movement so the local host cursor stays completely stationary
                    return LRESULT(1);
                }
                WM_LBUTTONDOWN => {
                    if let Some(tx) = HOOK_EVENT_TX.get() {
                        let _ = tx.send(InputEvent::Mouse(MouseEvent::Button {
                            button: MouseButton::Left,
                            state: ElementState::Pressed,
                            timestamp: 0,
                        }));
                    }
                    return LRESULT(1);
                }
                WM_LBUTTONUP => {
                    if let Some(tx) = HOOK_EVENT_TX.get() {
                        let _ = tx.send(InputEvent::Mouse(MouseEvent::Button {
                            button: MouseButton::Left,
                            state: ElementState::Released,
                            timestamp: 0,
                        }));
                    }
                    return LRESULT(1);
                }
                WM_RBUTTONDOWN => {
                    if let Some(tx) = HOOK_EVENT_TX.get() {
                        let _ = tx.send(InputEvent::Mouse(MouseEvent::Button {
                            button: MouseButton::Right,
                            state: ElementState::Pressed,
                            timestamp: 0,
                        }));
                    }
                    return LRESULT(1);
                }
                WM_RBUTTONUP => {
                    if let Some(tx) = HOOK_EVENT_TX.get() {
                        let _ = tx.send(InputEvent::Mouse(MouseEvent::Button {
                            button: MouseButton::Right,
                            state: ElementState::Released,
                            timestamp: 0,
                        }));
                    }
                    return LRESULT(1);
                }
                WM_MBUTTONDOWN => {
                    if let Some(tx) = HOOK_EVENT_TX.get() {
                        let _ = tx.send(InputEvent::Mouse(MouseEvent::Button {
                            button: MouseButton::Middle,
                            state: ElementState::Pressed,
                            timestamp: 0,
                        }));
                    }
                    return LRESULT(1);
                }
                WM_MBUTTONUP => {
                    if let Some(tx) = HOOK_EVENT_TX.get() {
                        let _ = tx.send(InputEvent::Mouse(MouseEvent::Button {
                            button: MouseButton::Middle,
                            state: ElementState::Released,
                            timestamp: 0,
                        }));
                    }
                    return LRESULT(1);
                }
                WM_MOUSEWHEEL => {
                    let delta = ((info.mouseData >> 16) as i16) / 120;
                    if let Some(tx) = HOOK_EVENT_TX.get() {
                        let _ = tx.send(InputEvent::Mouse(MouseEvent::Scroll {
                            dx: 0,
                            dy: delta,
                            timestamp: 0,
                        }));
                    }
                    return LRESULT(1);
                }
                WM_MOUSEHWHEEL => {
                    let delta = ((info.mouseData >> 16) as i16) / 120;
                    if let Some(tx) = HOOK_EVENT_TX.get() {
                        let _ = tx.send(InputEvent::Mouse(MouseEvent::Scroll {
                            dx: delta,
                            dy: 0,
                            timestamp: 0,
                        }));
                    }
                    return LRESULT(1);
                }
                _ => {}
            }
        }
    }
    CallNextHookEx(None, n_code, w_param, l_param)
}

#[cfg(target_os = "windows")]
fn run_windows_hook_thread(stop_flag: Arc<AtomicBool>) {
    // SAFETY: Low-level mouse hook on dedicated worker thread
    let hook = unsafe {
        SetWindowsHookExW(
            WH_MOUSE_LL,
            Some(low_level_mouse_proc),
            HINSTANCE(std::ptr::null_mut()),
            0,
        )
    };

    if let Ok(h) = hook {
        info!("Windows WH_MOUSE_LL hook successfully installed");
        let mut msg = MSG::default();
        while !stop_flag.load(Ordering::Relaxed) {
            // SAFETY: Pumping messages to keep Windows hook dispatching
            unsafe {
                if PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                } else {
                    std::thread::sleep(Duration::from_millis(2));
                }
            }
        }
        // SAFETY: Cleanup hook upon thread exit
        unsafe {
            let _ = UnhookWindowsHookEx(h);
        }
        info!("Windows WH_MOUSE_LL hook uninstalled");
    } else {
        warn!("Failed to install Windows WH_MOUSE_LL hook — will use cursor trapping fallback");
    }
}

#[cfg(target_os = "linux")]
fn run_linux_hook_thread(stop_flag: Arc<AtomicBool>) {
    info!("Starting Linux X11 mouse capture thread");
    // SAFETY: Open dedicated X display connection for this background capture worker
    let display = unsafe { x11::xlib::XOpenDisplay(std::ptr::null()) };
    if display.is_null() {
        warn!("run_linux_hook_thread: Failed to open X display — mouse capture disabled");
        return;
    }

    let screen = unsafe { x11::xlib::XDefaultScreen(display) };
    let root = unsafe { x11::xlib::XRootWindow(display, screen) };

    let (screen_w, screen_h) = get_screen_dimensions();
    let center_x = screen_w / 2;
    let center_y = screen_h / 2;

    // Create a 1x1 transparent invisible cursor so that no cursor is rendered on screen during grab
    let mut dummy_color: x11::xlib::XColor = unsafe { std::mem::zeroed() };
    let data = [0u8; 1];
    let blank_pixmap = unsafe {
        x11::xlib::XCreateBitmapFromData(
            display,
            root,
            data.as_ptr() as *const std::os::raw::c_char,
            1,
            1,
        )
    };
    let blank_cursor = unsafe {
        x11::xlib::XCreatePixmapCursor(
            display,
            blank_pixmap,
            blank_pixmap,
            &mut dummy_color,
            &mut dummy_color,
            0,
            0,
        )
    };
    unsafe {
        x11::xlib::XFreePixmap(display, blank_pixmap);
    }

    let mut was_controlling = false;
    let mut last_x = center_x;
    let mut last_y = center_y;

    while !stop_flag.load(Ordering::Relaxed) {
        let is_controlling = IS_CONTROLLING_REMOTE.load(Ordering::Relaxed);

        if is_controlling && !was_controlling {
            // ── State Transition: Local Host -> Controlling Remote ──
            unsafe {
                // 1. Hide the local cursor via XFixes
                x11::xfixes::XFixesHideCursor(display, root);

                // 2. Warp pointer to center to give ample room for relative movement
                x11::xlib::XWarpPointer(display, 0, root, 0, 0, 0, 0, center_x, center_y);
                x11::xlib::XFlush(display);

                // 3. Grab pointer with blank_cursor so clicks/moves never leak to local apps
                let mut grabbed = false;
                for _ in 0..10 {
                    let status = x11::xlib::XGrabPointer(
                        display,
                        root,
                        0, // owner_events = False
                        (x11::xlib::PointerMotionMask
                            | x11::xlib::ButtonPressMask
                            | x11::xlib::ButtonReleaseMask) as u32,
                        x11::xlib::GrabModeAsync,
                        x11::xlib::GrabModeAsync,
                        0, // confine_to = None
                        blank_cursor,
                        x11::xlib::CurrentTime,
                    );
                    if status == x11::xlib::GrabSuccess {
                        grabbed = true;
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
                if grabbed {
                    debug!("Linux X11 pointer successfully grabbed with invisible cursor");
                } else {
                    warn!("Linux X11 XGrabPointer failed after 10 attempts");
                }

                // 4. Drain all pending events generated during the transition (including warp to center)
                x11::xlib::XFlush(display);
                while x11::xlib::XPending(display) > 0 {
                    let mut discard: x11::xlib::XEvent = std::mem::zeroed();
                    x11::xlib::XNextEvent(display, &mut discard);
                }
            }

            last_x = center_x;
            last_y = center_y;
            was_controlling = true;
        } else if !is_controlling && was_controlling {
            // ── State Transition: Remote -> Returned to Local Host ──
            unsafe {
                // 1. Release pointer grab
                x11::xlib::XUngrabPointer(display, x11::xlib::CurrentTime);

                // 2. Restore cursor to the freeze / return position
                let fx = FREEZE_CURSOR_X.load(Ordering::Relaxed);
                let fy = FREEZE_CURSOR_Y.load(Ordering::Relaxed);
                let restore_x = if fx >= 0 { fx } else { center_x };
                let restore_y = if fy >= 0 { fy } else { center_y };

                x11::xlib::XWarpPointer(display, 0, root, 0, 0, 0, 0, restore_x, restore_y);
                x11::xfixes::XFixesShowCursor(display, root);
                x11::xlib::XFlush(display);
            }
            was_controlling = false;
        }

        if is_controlling {
            // Emergency Escape key check via XQueryKeymap
            let mut keymap = [0u8; 32];
            unsafe {
                x11::xlib::XQueryKeymap(display, keymap.as_mut_ptr() as *mut i8);
            }
            let escape_keycode = unsafe { x11::xlib::XKeysymToKeycode(display, 0xff1b) }; // XK_Escape = 0xff1b
            if escape_keycode > 0
                && (keymap[(escape_keycode / 8) as usize] & (1 << (escape_keycode % 8))) != 0
            {
                info!("Emergency Escape pressed on Linux Host: releasing remote control");
                FREEZE_CURSOR_X.store(-1, Ordering::Relaxed);
                FREEZE_CURSOR_Y.store(-1, Ordering::Relaxed);
                IS_CONTROLLING_REMOTE.store(false, Ordering::SeqCst);
                continue;
            }

            // Process captured input events
            unsafe {
                while x11::xlib::XPending(display) > 0 {
                    let mut event: x11::xlib::XEvent = std::mem::zeroed();
                    x11::xlib::XNextEvent(display, &mut event);

                    match event.get_type() {
                        x11::xlib::ButtonPress => {
                            let btn_num = event.button.button;
                            if btn_num == 4 {
                                if let Some(tx) = HOOK_EVENT_TX.get() {
                                    let _ = tx.send(InputEvent::Mouse(MouseEvent::Scroll {
                                        dx: 0,
                                        dy: -1,
                                        timestamp: 0,
                                    }));
                                }
                            } else if btn_num == 5 {
                                if let Some(tx) = HOOK_EVENT_TX.get() {
                                    let _ = tx.send(InputEvent::Mouse(MouseEvent::Scroll {
                                        dx: 0,
                                        dy: 1,
                                        timestamp: 0,
                                    }));
                                }
                            } else if btn_num == 6 {
                                if let Some(tx) = HOOK_EVENT_TX.get() {
                                    let _ = tx.send(InputEvent::Mouse(MouseEvent::Scroll {
                                        dx: -1,
                                        dy: 0,
                                        timestamp: 0,
                                    }));
                                }
                            } else if btn_num == 7 {
                                if let Some(tx) = HOOK_EVENT_TX.get() {
                                    let _ = tx.send(InputEvent::Mouse(MouseEvent::Scroll {
                                        dx: 1,
                                        dy: 0,
                                        timestamp: 0,
                                    }));
                                }
                            } else {
                                let friday_btn = match btn_num {
                                    1 => MouseButton::Left,
                                    2 => MouseButton::Middle,
                                    3 => MouseButton::Right,
                                    other => MouseButton::Other(other as u8),
                                };
                                if let Some(tx) = HOOK_EVENT_TX.get() {
                                    let _ = tx.send(InputEvent::Mouse(MouseEvent::Button {
                                        button: friday_btn,
                                        state: ElementState::Pressed,
                                        timestamp: 0,
                                    }));
                                }
                            }
                        }
                        x11::xlib::ButtonRelease => {
                            let btn_num = event.button.button;
                            if btn_num <= 3 || btn_num > 7 {
                                let friday_btn = match btn_num {
                                    1 => MouseButton::Left,
                                    2 => MouseButton::Middle,
                                    3 => MouseButton::Right,
                                    other => MouseButton::Other(other as u8),
                                };
                                if let Some(tx) = HOOK_EVENT_TX.get() {
                                    let _ = tx.send(InputEvent::Mouse(MouseEvent::Button {
                                        button: friday_btn,
                                        state: ElementState::Released,
                                        timestamp: 0,
                                    }));
                                }
                            }
                        }
                        x11::xlib::MotionNotify => {
                            let cur_x = event.motion.x_root;
                            let cur_y = event.motion.y_root;

                            let dx = cur_x - last_x;
                            let dy = cur_y - last_y;
                            last_x = cur_x;
                            last_y = cur_y;

                            if dx != 0 || dy != 0 {
                                if let Some(tx) = HOOK_EVENT_TX.get() {
                                    let _ = tx.send(InputEvent::Mouse(MouseEvent::MoveRel {
                                        dx: dx.clamp(i16::MIN as i32, i16::MAX as i32) as i16,
                                        dy: dy.clamp(i16::MIN as i32, i16::MAX as i32) as i16,
                                        timestamp: 0,
                                    }));
                                }
                            }

                            // Boundary Guard: Re-center ONLY when approaching screen borders
                            // This ensures relative motion can continue indefinitely without clamping
                            let margin = 100;
                            if cur_x < margin
                                || cur_x > screen_w - margin
                                || cur_y < margin
                                || cur_y > screen_h - margin
                            {
                                x11::xlib::XWarpPointer(
                                    display, 0, root, 0, 0, 0, 0, center_x, center_y,
                                );
                                x11::xlib::XFlush(display);
                                last_x = center_x;
                                last_y = center_y;

                                // Drain the synthetic MotionNotify generated by the warp
                                while x11::xlib::XPending(display) > 0 {
                                    let mut discard: x11::xlib::XEvent = std::mem::zeroed();
                                    x11::xlib::XNextEvent(display, &mut discard);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        std::thread::sleep(Duration::from_millis(1));
    }

    // SAFETY: Clean ungrab, restore cursor, and free resources on thread exit
    unsafe {
        if was_controlling {
            x11::xlib::XUngrabPointer(display, x11::xlib::CurrentTime);
            x11::xfixes::XFixesShowCursor(display, root);
            x11::xlib::XFlush(display);
        }
        x11::xlib::XFreeCursor(display, blank_cursor);
        x11::xlib::XCloseDisplay(display);
    }
    info!("Linux X11 mouse capture thread stopped");
}

// ── Background Engine Service ──────────────────────────────────────────────────

pub fn start_engine_service(shared_state: SharedAppState, stop_flag: Arc<AtomicBool>) {
    let (event_tx, event_rx) = std::sync::mpsc::channel::<InputEvent>();
    let _ = HOOK_EVENT_TX.set(event_tx);

    // 1. Windows low-level hook thread
    #[cfg(target_os = "windows")]
    {
        let stop_clone = stop_flag.clone();
        std::thread::spawn(move || {
            run_windows_hook_thread(stop_clone);
        });
    }

    // 1b. Linux X11 mouse capture hook thread
    #[cfg(target_os = "linux")]
    {
        let stop_clone = stop_flag.clone();
        std::thread::spawn(move || {
            run_linux_hook_thread(stop_clone);
        });
    }

    // 2. UDP Input & Control Receiver Thread
    let state_recv = shared_state.clone();
    let stop_recv = stop_flag.clone();
    std::thread::spawn(move || {
        run_input_receiver(state_recv, stop_recv);
    });

    // 3. Mouse Tracking & Edge Routing Loop
    let state_track = shared_state.clone();
    let stop_track = stop_flag.clone();
    std::thread::spawn(move || {
        run_mouse_router(state_track, stop_track, event_rx);
    });
}

// ── UDP Input & Control Receiver ───────────────────────────────────────────────

fn run_input_receiver(shared_state: SharedAppState, stop_flag: Arc<AtomicBool>) {
    let port = {
        let app = shared_state.lock().unwrap();
        app.settings.peer_port
    };

    let socket = match UdpSocket::bind(format!("0.0.0.0:{}", port)) {
        Ok(s) => s,
        Err(e) => {
            warn!("Failed to bind input receiver on port {}: {}", port, e);
            return;
        }
    };
    socket
        .set_read_timeout(Some(Duration::from_millis(100)))
        .ok();

    info!("FRIDAY Input Receiver listening on UDP port {}", port);

    let mut buf = [0u8; 1024];
    let mut last_second = Instant::now();
    let mut packets_this_sec = 0u64;
    let mut bytes_this_sec = 0u64;
    let mut client_last_move_log = Instant::now();
    let mut client_move_count = 0u32;

    while !stop_flag.load(Ordering::Relaxed) {
        match socket.recv_from(&mut buf) {
            Ok((len, src)) => {
                packets_this_sec += 1;
                bytes_this_sec += len as u64;

                if let Ok(packet) = NetworkPacket::decode(&buf[..len]) {
                    match packet.payload {
                        PacketPayload::Input(event) => {
                            if let Ok(mut app) = shared_state.lock() {
                                if app.is_host {
                                    app.is_host = false;
                                    app.persist_config();
                                }
                            }
                            // Only inject into OS if we are NOT currently controlling remote
                            if !IS_CONTROLLING_REMOTE.load(Ordering::Relaxed) {
                                match &event {
                                    InputEvent::Mouse(MouseEvent::Button {
                                        button, state, ..
                                    }) => {
                                        if let Ok(mut app) = shared_state.lock() {
                                            app.add_log(
                                                "INFO",
                                                "friday_core::mouse",
                                                &format!("Client injected mouse button {:?} {:?} from Host ({})", button, state, src),
                                            );
                                        }
                                    }
                                    InputEvent::Mouse(MouseEvent::Scroll { dx, dy, .. }) => {
                                        if let Ok(mut app) = shared_state.lock() {
                                            app.add_log(
                                                "INFO",
                                                "friday_core::mouse",
                                                &format!("Client injected scroll (dx: {}, dy: {}) from Host ({})", dx, dy, src),
                                            );
                                        }
                                    }
                                    InputEvent::Mouse(MouseEvent::MoveRel { .. }) => {
                                        client_move_count += 1;
                                        if client_last_move_log.elapsed()
                                            >= Duration::from_millis(1000)
                                        {
                                            if let Some((cx, cy)) = get_local_cursor_pos() {
                                                if let Ok(mut app) = shared_state.lock() {
                                                    app.add_log(
                                                        "INFO",
                                                        "friday_core::mouse",
                                                        &format!("Client active: injected {} mouse moves from Host ({}) | cursor at ({}, {})", client_move_count, src, cx, cy),
                                                    );
                                                }
                                            }
                                            client_move_count = 0;
                                            client_last_move_log = Instant::now();
                                        }
                                    }
                                    _ => {}
                                }
                                inject_os_event(&event);
                            }
                        }
                        PacketPayload::Control(bytes) => {
                            handle_incoming_control(bytes, src, &shared_state, &socket);
                        }
                        _ => {}
                    }
                }
            }
            Err(_) => {
                // Timeout — normal for non-blocking recv
            }
        }

        // Update telemetry rates every second
        if last_second.elapsed() >= Duration::from_secs(1) {
            if let Ok(mut app) = shared_state.lock() {
                app.telemetry.packets_per_sec = packets_this_sec as u32;
                app.telemetry.bytes_per_sec = bytes_this_sec as u32;
            }
            packets_this_sec = 0;
            bytes_this_sec = 0;
            last_second = Instant::now();
        }
    }
}

fn handle_incoming_control(
    bytes: Vec<u8>,
    src: SocketAddr,
    shared_state: &SharedAppState,
    socket: &UdpSocket,
) {
    if let Ok(msg) = ControlMessage::decode(&bytes) {
        match msg {
            ControlMessage::HandoffControl {
                entry_x_norm,
                entry_y_norm,
            } => {
                // Remote transferred control to this machine!
                info!(
                    "Control handed off to this machine at ({:.3}, {:.3}) from {}",
                    entry_x_norm, entry_y_norm, src
                );
                IS_CONTROLLING_REMOTE.store(false, Ordering::SeqCst);

                let (screen_w, screen_h) = get_screen_dimensions();
                let target_x =
                    ((entry_x_norm * (screen_w - 1) as f32).round() as i32).clamp(0, screen_w - 1);
                let target_y =
                    ((entry_y_norm * (screen_h - 1) as f32).round() as i32).clamp(0, screen_h - 1);

                set_local_cursor_pos(target_x, target_y);

                if let Ok(mut app) = shared_state.lock() {
                    let local_id = app.local_device_id.clone();
                    app.active_device_id = local_id.clone();
                    app.topology.set_active_device(&local_id);
                    if app.is_host {
                        app.is_host = false;
                        app.persist_config();
                    }
                    for d in &mut app.devices {
                        d.is_active = d.id == local_id;
                    }
                    app.add_log(
                        "INFO",
                        "friday_core::ownership",
                        &format!(
                            "Host mouse entered screen — placed at ({}, {}) (Client Screen Mode)",
                            target_x, target_y
                        ),
                    );
                }
            }
            ControlMessage::TakeControl => {
                // Remote host is claiming control
                info!("Remote {} claimed TakeControl", src);
                if let Ok(mut app) = shared_state.lock() {
                    let remote_dev = app
                        .devices
                        .iter()
                        .find(|d| d.ip_address == src.ip().to_string())
                        .cloned();
                    if let Some(r) = remote_dev {
                        app.active_device_id = r.id.clone();
                        app.topology.set_active_device(&r.id);
                        for d in &mut app.devices {
                            d.is_active = d.id == r.id;
                        }
                    }
                }
            }
            ControlMessage::ReleaseAll => {
                debug!("Received ReleaseAll from {}", src);
                // Release held mouse buttons on client
                inject_os_event(&InputEvent::Mouse(MouseEvent::Button {
                    button: MouseButton::Left,
                    state: ElementState::Released,
                    timestamp: 0,
                }));
                inject_os_event(&InputEvent::Mouse(MouseEvent::Button {
                    button: MouseButton::Right,
                    state: ElementState::Released,
                    timestamp: 0,
                }));
                if let Ok(mut app) = shared_state.lock() {
                    if !app.is_host {
                        // On Client, cursor is no longer on this display; mark host as active owner
                        let remote_id = app
                            .devices
                            .iter()
                            .find(|d| !d.is_local)
                            .map(|d| d.id.clone())
                            .unwrap_or_default();
                        if !remote_id.is_empty() {
                            app.active_device_id = remote_id.clone();
                            app.topology.set_active_device(&remote_id);
                            for d in &mut app.devices {
                                d.is_active = d.id == remote_id;
                            }
                        }
                    }
                }
            }
            ControlMessage::Ping { seq } => {
                let pong = ControlMessage::Pong { seq };
                if let Ok(pong_bytes) = pong.encode() {
                    let packet = NetworkPacket::new_control(pong_bytes);
                    if let Ok(enc) = packet.encode() {
                        let _ = socket.send_to(&enc, src);
                    }
                }
            }
            ControlMessage::Pong { seq: _ } => {
                // Heartbeat reply received
            }
            ControlMessage::Hello {
                device_name,
                screen: _,
            } => {
                info!("Hello received from {} ({})", device_name, src);
                if let Ok(mut app) = shared_state.lock() {
                    if let Some(dev) = app
                        .devices
                        .iter_mut()
                        .find(|d| d.ip_address == src.ip().to_string())
                    {
                        dev.is_connected = true;
                    }
                }
                let (w, h) = get_screen_dimensions();
                let welcome = ControlMessage::Welcome {
                    device_name: crate::state::detect_local_hostname(),
                    screen: friday_agent::control::ScreenInfo {
                        width: w as u32,
                        height: h as u32,
                        scale_factor: 1.0,
                        device_name: crate::state::detect_local_hostname(),
                    },
                };
                if let Ok(bytes) = welcome.encode() {
                    let packet = NetworkPacket::new_control(bytes);
                    if let Ok(enc) = packet.encode() {
                        let _ = socket.send_to(&enc, src);
                    }
                }
            }
            ControlMessage::Welcome {
                device_name,
                screen: _,
            } => {
                info!(
                    "Welcome received from {} ({}) — link active",
                    device_name, src
                );
                if let Ok(mut app) = shared_state.lock() {
                    if let Some(dev) = app
                        .devices
                        .iter_mut()
                        .find(|d| d.ip_address == src.ip().to_string())
                    {
                        dev.is_connected = true;
                    }
                }
            }
            ControlMessage::Goodbye => {
                info!("Goodbye received from {}", src);
                if let Ok(mut app) = shared_state.lock() {
                    if let Some(dev) = app
                        .devices
                        .iter_mut()
                        .find(|d| d.ip_address == src.ip().to_string())
                    {
                        dev.is_connected = false;
                    }
                }
            }
            _ => {}
        }
    }
}

// ── Mouse Tracking & Circular Edge Routing ─────────────────────────────────────

fn run_mouse_router(
    shared_state: SharedAppState,
    stop_flag: Arc<AtomicBool>,
    event_rx: std::sync::mpsc::Receiver<InputEvent>,
) {
    let send_socket = match UdpSocket::bind("0.0.0.0:0") {
        Ok(s) => s,
        Err(e) => {
            warn!("Failed to bind mouse router outgoing UDP socket: {}", e);
            return;
        }
    };

    let mut dwell_start: Option<(Edge, Instant)> = None;
    let mut return_dwell_start: Option<(Edge, Instant)> = None;

    // Remote virtual cursor position and screen dimensions
    let mut remote_x = 960.0f32;
    let mut remote_y = 540.0f32;
    let remote_w = 1920.0f32;
    let remote_h = 1080.0f32;
    let mut target_ip = String::new();
    let mut target_port = 48700u16;
    let mut current_target_device_id = String::new();

    let mut seq = 0u32;
    let threshold = 4i32;

    let mut last_move_log_time = Instant::now();
    let mut move_log_count = 0u32;
    let mut move_log_dx = 0i32;
    let mut move_log_dy = 0i32;

    while !stop_flag.load(Ordering::Relaxed) {
        let (is_running, is_paused, is_host, local_id, active_id, connected_peers, dwell_ms) = {
            let app = shared_state.lock().unwrap();
            let peers: Vec<crate::types::DeviceInfo> = app
                .devices
                .iter()
                .filter(|d| !d.is_local && d.is_connected)
                .cloned()
                .collect();
            (
                app.engine_running,
                app.engine_paused,
                app.is_host,
                app.local_device_id.clone(),
                app.active_device_id.clone(),
                peers,
                app.settings.edge_dwell_ms.max(100),
            )
        };

        // If engine is not running, paused, this machine is a CLIENT (not host), or no peers:
        // Client machines NEVER run mouse edge routing or capture local mouse!
        if !is_running || is_paused || !is_host || connected_peers.is_empty() {
            if IS_CONTROLLING_REMOTE.load(Ordering::Relaxed) {
                IS_CONTROLLING_REMOTE.store(false, Ordering::SeqCst);
            }
            std::thread::sleep(Duration::from_millis(50));
            continue;
        }

        // On Host: detect if active_id was changed via GUI (e.g. user clicked "Take Control" on Host)
        if active_id != local_id {
            if current_target_device_id != active_id
                || !IS_CONTROLLING_REMOTE.load(Ordering::Relaxed)
            {
                if let Some(target_dev) = connected_peers.iter().find(|d| d.id == active_id) {
                    target_ip = target_dev.ip_address.clone();
                    target_port = target_dev.port;
                    current_target_device_id = target_dev.id.clone();
                    remote_x = remote_w / 2.0;
                    remote_y = remote_h / 2.0;
                    IS_CONTROLLING_REMOTE.store(true, Ordering::SeqCst);
                    HAS_LAST_HOOK_PT.store(false, Ordering::SeqCst);
                    return_dwell_start = None;
                }
            }
        } else if IS_CONTROLLING_REMOTE.load(Ordering::Relaxed) && active_id == local_id {
            IS_CONTROLLING_REMOTE.store(false, Ordering::SeqCst);
            return_dwell_start = None;
            current_target_device_id.clear();
        }

        let is_controlling = IS_CONTROLLING_REMOTE.load(Ordering::Relaxed);
        let (screen_w, screen_h) = get_screen_dimensions();

        // ── Case 1: Physical mouse is active locally on this machine ──
        if !is_controlling && active_id == local_id {
            if let Some((cur_x, cur_y)) = get_local_cursor_pos() {
                // Detect which screen edge cursor is dwelling on
                let edge = if cur_x >= screen_w - 1 - threshold {
                    Some(Edge::Right)
                } else if cur_x <= threshold {
                    Some(Edge::Left)
                } else if cur_y <= threshold {
                    Some(Edge::Top)
                } else if cur_y >= screen_h - 1 - threshold {
                    Some(Edge::Bottom)
                } else {
                    None
                };

                if let Some(hit_edge) = edge {
                    // Check topology: which device is connected at this edge?
                    let next_target = {
                        let app = shared_state.lock().unwrap();
                        app.topology
                            .neighbor_at_edge(&local_id, hit_edge)
                            .map(|s| s.to_string())
                    };

                    if let Some(target_id) = next_target {
                        if let Some(target_dev) = connected_peers.iter().find(|d| d.id == target_id)
                        {
                            let now = Instant::now();
                            let is_same = match dwell_start {
                                Some((e, _)) => e == hit_edge,
                                None => false,
                            };

                            if !is_same {
                                dwell_start = Some((hit_edge, now));
                                if let Ok(mut app) = shared_state.lock() {
                                    app.add_log(
                                        "INFO",
                                        "friday_core::mouse",
                                        &format!(
                                            "Mouse dwelling on {:?} edge at ({}, {}) → handoff target: {} ({}ms dwell)",
                                            hit_edge, cur_x, cur_y, target_dev.name, dwell_ms
                                        ),
                                    );
                                }
                            } else if let Some((_, start)) = dwell_start {
                                if start.elapsed() >= Duration::from_millis(dwell_ms) {
                                    // ── TRIGGER HANDOFF TO TARGET DEVICE ──
                                    let norm_x = (cur_x as f32 / screen_w as f32).clamp(0.0, 1.0);
                                    let norm_y = (cur_y as f32 / screen_h as f32).clamp(0.0, 1.0);
                                    let entry_point = CircularTopology::calculate_entry_point(
                                        hit_edge,
                                        NormalizedPoint {
                                            x: norm_x,
                                            y: norm_y,
                                        },
                                    );

                                    info!(
                                        "Edge {:?} handoff triggered! Transferring active ownership to {} ({})",
                                        hit_edge, target_dev.name, target_dev.ip_address
                                    );

                                    target_ip = target_dev.ip_address.clone();
                                    target_port = target_dev.port;
                                    current_target_device_id = target_dev.id.clone();
                                    remote_x = entry_point.x * remote_w;
                                    remote_y = entry_point.y * remote_h;

                                    // Send HandoffControl over UDP
                                    let handoff = ControlMessage::HandoffControl {
                                        entry_x_norm: entry_point.x,
                                        entry_y_norm: entry_point.y,
                                    };
                                    if let Ok(bytes) = handoff.encode() {
                                        let packet = NetworkPacket::new_control(bytes);
                                        if let Ok(enc) = packet.encode() {
                                            let dest = format!("{}:{}", target_ip, target_port);
                                            let _ = send_socket.send_to(&enc, &dest);
                                        }
                                    }

                                    // Update state: active owner is now the remote device
                                    if let Ok(mut app) = shared_state.lock() {
                                        app.active_device_id = current_target_device_id.clone();
                                        app.topology.set_active_device(&current_target_device_id);
                                        for d in &mut app.devices {
                                            d.is_active = d.id == current_target_device_id;
                                        }
                                        app.add_log(
                                            "INFO",
                                            "friday_core::mouse",
                                            &format!(
                                                "Mouse crossed {:?} edge ({}, {})! Active ownership transferred to {} ({}) at entry ({:.3}, {:.3})",
                                                hit_edge, cur_x, cur_y, target_dev.name, target_dev.ip_address, entry_point.x, entry_point.y
                                            ),
                                        );
                                        app.persist_config();
                                    }

                                    FREEZE_CURSOR_X.store(cur_x, Ordering::Relaxed);
                                    FREEZE_CURSOR_Y.store(cur_y, Ordering::Relaxed);
                                    IS_CONTROLLING_REMOTE.store(true, Ordering::SeqCst);
                                    HAS_LAST_HOOK_PT.store(false, Ordering::SeqCst);
                                    dwell_start = None;
                                }
                            }
                        } else {
                            dwell_start = None;
                        }
                    } else {
                        dwell_start = None;
                    }
                } else {
                    dwell_start = None;
                }
            }
            std::thread::sleep(Duration::from_millis(3));
        }
        // ── Case 2: Physical mouse is captured locally, driving active remote device ──
        else if is_controlling {
            // Drain captured mouse events from the hook and transmit to active remote peer
            let mut drained = false;
            while let Ok(evt) = event_rx.try_recv() {
                drained = true;
                seq = seq.wrapping_add(1);

                // Update virtual cursor coordinates on active remote device and log key inputs
                match &evt {
                    InputEvent::Mouse(MouseEvent::MoveRel { dx, dy, .. }) => {
                        remote_x = (remote_x + *dx as f32).clamp(0.0, remote_w - 1.0);
                        remote_y = (remote_y + *dy as f32).clamp(0.0, remote_h - 1.0);
                        move_log_count += 1;
                        move_log_dx += *dx as i32;
                        move_log_dy += *dy as i32;
                    }
                    InputEvent::Mouse(MouseEvent::Button { button, state, .. }) => {
                        if let Ok(mut app) = shared_state.lock() {
                            app.add_log(
                                "INFO",
                                "friday_core::mouse",
                                &format!(
                                    "Host forwarded mouse button {:?} {:?} to {}",
                                    button, state, current_target_device_id
                                ),
                            );
                        }
                    }
                    InputEvent::Mouse(MouseEvent::Scroll { dx, dy, .. }) => {
                        if let Ok(mut app) = shared_state.lock() {
                            app.add_log(
                                "INFO",
                                "friday_core::mouse",
                                &format!(
                                    "Host forwarded mouse scroll (dx: {}, dy: {}) to {}",
                                    dx, dy, current_target_device_id
                                ),
                            );
                        }
                    }
                    _ => {}
                }

                // Check which edge of the active remote device the virtual cursor reached
                let virtual_edge = if remote_x >= remote_w - 1.0 - threshold as f32 {
                    Some(Edge::Right)
                } else if remote_x <= threshold as f32 {
                    Some(Edge::Left)
                } else if remote_y <= threshold as f32 {
                    Some(Edge::Top)
                } else if remote_y >= remote_h - 1.0 - threshold as f32 {
                    Some(Edge::Bottom)
                } else {
                    None
                };

                if let Some(hit_edge) = virtual_edge {
                    // Query CircularTopology: which device connects at this edge of the active device?
                    let next_target = {
                        let app = shared_state.lock().unwrap();
                        app.topology
                            .neighbor_at_edge(&current_target_device_id, hit_edge)
                            .map(|s| s.to_string())
                    };

                    if let Some(next_id) = next_target {
                        let now = Instant::now();
                        let is_same = match return_dwell_start {
                            Some((e, _)) => e == hit_edge,
                            None => false,
                        };

                        if !is_same {
                            return_dwell_start = Some((hit_edge, now));
                            if let Ok(mut app) = shared_state.lock() {
                                app.add_log(
                                    "INFO",
                                    "friday_core::mouse",
                                    &format!(
                                        "Virtual cursor on {} hit {:?} edge at ({:.0}, {:.0}) — dwell started for next peer {}",
                                        current_target_device_id, hit_edge, remote_x, remote_y, next_id
                                    ),
                                );
                            }
                        } else if let Some((_, start)) = return_dwell_start {
                            if start.elapsed() >= Duration::from_millis(dwell_ms) {
                                let norm_x = (remote_x / remote_w).clamp(0.0, 1.0);
                                let norm_y = (remote_y / remote_h).clamp(0.0, 1.0);
                                let entry_point = CircularTopology::calculate_entry_point(
                                    hit_edge,
                                    NormalizedPoint {
                                        x: norm_x,
                                        y: norm_y,
                                    },
                                );

                                if next_id == local_id {
                                    // ── CIRCULAR RETURN TO PHYSICAL MOUSE SOURCE (LOCAL MACHINE) ──
                                    info!(
                                        "Circular routing: virtual cursor on {} crossed {:?} edge → returning ownership to Physical Mouse machine ({})",
                                        current_target_device_id, hit_edge, local_id
                                    );

                                    // Release any held buttons on the remote device
                                    if let Ok(bytes) = ControlMessage::ReleaseAll.encode() {
                                        let packet = NetworkPacket::new_control(bytes);
                                        if let Ok(enc) = packet.encode() {
                                            let dest = format!("{}:{}", target_ip, target_port);
                                            let _ = send_socket.send_to(&enc, &dest);
                                        }
                                    }

                                    let local_entry_x =
                                        ((entry_point.x * (screen_w - 1) as f32).round() as i32)
                                            .clamp(0, screen_w - 1);
                                    let local_entry_y =
                                        ((entry_point.y * (screen_h - 1) as f32).round() as i32)
                                            .clamp(0, screen_h - 1);

                                    FREEZE_CURSOR_X.store(local_entry_x, Ordering::Relaxed);
                                    FREEZE_CURSOR_Y.store(local_entry_y, Ordering::Relaxed);
                                    IS_CONTROLLING_REMOTE.store(false, Ordering::SeqCst);

                                    set_local_cursor_pos(local_entry_x, local_entry_y);

                                    if let Ok(mut app) = shared_state.lock() {
                                        let local_id_clone = app.local_device_id.clone();
                                        app.active_device_id = local_id_clone.clone();
                                        app.topology.set_active_device(&local_id_clone);
                                        for d in &mut app.devices {
                                            d.is_active = d.id == local_id_clone;
                                        }
                                        app.add_log(
                                            "INFO",
                                            "friday_core::mouse",
                                            &format!(
                                                "Virtual cursor crossed {:?} edge of {} → returned to Host ({}) at ({}, {})",
                                                hit_edge, current_target_device_id, local_id_clone, local_entry_x, local_entry_y
                                            ),
                                        );
                                        app.persist_config();
                                    }

                                    return_dwell_start = None;
                                    break;
                                } else {
                                    // ── CIRCULAR FORWARDING TO NEXT REMOTE DEVICE IN RING (e.g. C in A → B → C) ──
                                    info!(
                                            "Circular routing: virtual cursor on {} crossed {:?} edge → transferring to next peer {}",
                                            current_target_device_id, hit_edge, next_id
                                        );

                                    // Release inputs on previous peer
                                    if let Ok(bytes) = ControlMessage::ReleaseAll.encode() {
                                        let packet = NetworkPacket::new_control(bytes);
                                        if let Ok(enc) = packet.encode() {
                                            let dest = format!("{}:{}", target_ip, target_port);
                                            let _ = send_socket.send_to(&enc, &dest);
                                        }
                                    }

                                    let next_peer = {
                                        let app = shared_state.lock().unwrap();
                                        app.devices
                                            .iter()
                                            .find(|d| d.id == next_id && d.is_connected)
                                            .cloned()
                                    };

                                    if let Some(next_dev) = next_peer {
                                        target_ip = next_dev.ip_address.clone();
                                        target_port = next_dev.port;
                                        current_target_device_id = next_dev.id.clone();
                                        remote_x = entry_point.x * remote_w;
                                        remote_y = entry_point.y * remote_h;

                                        // Send HandoffControl to new peer
                                        let handoff = ControlMessage::HandoffControl {
                                            entry_x_norm: entry_point.x,
                                            entry_y_norm: entry_point.y,
                                        };
                                        if let Ok(bytes) = handoff.encode() {
                                            let packet = NetworkPacket::new_control(bytes);
                                            if let Ok(enc) = packet.encode() {
                                                let dest = format!("{}:{}", target_ip, target_port);
                                                let _ = send_socket.send_to(&enc, &dest);
                                            }
                                        }

                                        if let Ok(mut app) = shared_state.lock() {
                                            app.active_device_id = current_target_device_id.clone();
                                            app.topology
                                                .set_active_device(&current_target_device_id);
                                            for d in &mut app.devices {
                                                d.is_active = d.id == current_target_device_id;
                                            }
                                            app.add_log(
                                                    "INFO",
                                                    "friday_core::ownership",
                                                    &format!(
                                                        "Circular routing: active ownership transferred to {}",
                                                        next_dev.name
                                                    ),
                                                );
                                            app.persist_config();
                                        }
                                    }

                                    return_dwell_start = None;
                                }
                            }
                        }
                    } else {
                        return_dwell_start = None;
                    }
                } else {
                    return_dwell_start = None;
                }

                // Send input packet over UDP to target peer
                let packet = NetworkPacket::new_input(evt, seq);
                if let Ok(enc) = packet.encode() {
                    let dest = format!("{}:{}", target_ip, target_port);
                    let _ = send_socket.send_to(&enc, &dest);
                }
            }

            if last_move_log_time.elapsed() >= Duration::from_millis(1000) {
                if move_log_count > 0 {
                    if let Ok(mut app) = shared_state.lock() {
                        app.add_log(
                            "INFO",
                            "friday_core::mouse",
                            &format!(
                                "Routing mouse to {}: sent {} moves (dx: {:+}, dy: {:+}) | virtual cursor: ({:.0}, {:.0})",
                                current_target_device_id, move_log_count, move_log_dx, move_log_dy, remote_x, remote_y
                            ),
                        );
                    }
                    move_log_count = 0;
                    move_log_dx = 0;
                    move_log_dy = 0;
                }
                last_move_log_time = Instant::now();
            }

            // Also check if active_id was changed via GUI (e.g. user clicked Take Control on local device)
            if active_id == local_id && is_controlling {
                IS_CONTROLLING_REMOTE.store(false, Ordering::SeqCst);
                return_dwell_start = None;
            }

            if !drained {
                std::thread::sleep(Duration::from_millis(2));
            }
        } else {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

// ── Public notification helpers ────────────────────────────────────────────────

/// Called when user clicks "Take Control" in the GUI (Host only)
pub fn notify_active_device_changed(shared_state: &SharedAppState, new_active_id: &str) {
    let (is_local, target_addr, all_remotes) = {
        let app = shared_state.lock().unwrap();
        let is_loc = new_active_id == app.local_device_id;
        let addr = app
            .devices
            .iter()
            .find(|d| d.id == new_active_id)
            .map(|d| format!("{}:{}", d.ip_address, d.port));
        let remotes: Vec<String> = app
            .devices
            .iter()
            .filter(|d| !d.is_local && d.is_connected)
            .map(|d| format!("{}:{}", d.ip_address, d.port))
            .collect();
        (is_loc, addr, remotes)
    };

    if is_local {
        // Host took control back locally!
        FREEZE_CURSOR_X.store(-1, Ordering::Relaxed);
        FREEZE_CURSOR_Y.store(-1, Ordering::Relaxed);
        IS_CONTROLLING_REMOTE.store(false, Ordering::SeqCst);
        if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
            if let Ok(bytes) = ControlMessage::ReleaseAll.encode() {
                let packet = NetworkPacket::new_control(bytes);
                if let Ok(enc) = packet.encode() {
                    for dest in all_remotes {
                        let _ = socket.send_to(&enc, &dest);
                    }
                }
            }
        }
    } else if let Some(dest) = target_addr {
        // Host jumped control directly to a remote client screen
        if let Some((cur_x, cur_y)) = get_local_cursor_pos() {
            FREEZE_CURSOR_X.store(cur_x, Ordering::Relaxed);
            FREEZE_CURSOR_Y.store(cur_y, Ordering::Relaxed);
        }
        IS_CONTROLLING_REMOTE.store(true, Ordering::SeqCst);
        HAS_LAST_HOOK_PT.store(false, Ordering::SeqCst);

        if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
            // First send ReleaseAll to other client screens
            if let Ok(bytes) = ControlMessage::ReleaseAll.encode() {
                let packet = NetworkPacket::new_control(bytes);
                if let Ok(enc) = packet.encode() {
                    for other_dest in all_remotes.iter().filter(|d| *d != &dest) {
                        let _ = socket.send_to(&enc, other_dest);
                    }
                }
            }
            // Send HandoffControl to target client screen
            let handoff = ControlMessage::HandoffControl {
                entry_x_norm: 0.5,
                entry_y_norm: 0.5,
            };
            if let Ok(bytes) = handoff.encode() {
                let packet = NetworkPacket::new_control(bytes);
                if let Ok(enc) = packet.encode() {
                    let _ = socket.send_to(&enc, &dest);
                }
            }
        }
    }
}

/// Called when a device connects
pub fn notify_device_connected(shared_state: &SharedAppState, device_id: &str) {
    let target = {
        let app = shared_state.lock().unwrap();
        app.devices
            .iter()
            .find(|d| d.id == device_id || d.ip_address == device_id)
            .map(|d| (d.ip_address.clone(), d.port))
    };

    if let Some((ip, port)) = target {
        let (w, h) = get_screen_dimensions();
        let hello = ControlMessage::Hello {
            device_name: crate::state::detect_local_hostname(),
            screen: friday_agent::control::ScreenInfo {
                width: w as u32,
                height: h as u32,
                scale_factor: 1.0,
                device_name: crate::state::detect_local_hostname(),
            },
        };
        if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
            let dest = format!("{}:{}", ip, port);
            if let Ok(bytes) = hello.encode() {
                let packet = NetworkPacket::new_control(bytes);
                if let Ok(enc) = packet.encode() {
                    let _ = socket.send_to(&enc, &dest);
                }
            }
        }
    }
}

/// Called when a device disconnects
pub fn notify_device_disconnected(shared_state: &SharedAppState, device_id: &str) {
    let target = {
        let app = shared_state.lock().unwrap();
        app.devices
            .iter()
            .find(|d| d.id == device_id || d.ip_address == device_id)
            .map(|d| (d.ip_address.clone(), d.port))
    };

    if let Some((ip, port)) = target {
        if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
            let dest = format!("{}:{}", ip, port);
            if let Ok(bytes) = ControlMessage::Goodbye.encode() {
                let packet = NetworkPacket::new_control(bytes);
                if let Ok(enc) = packet.encode() {
                    let _ = socket.send_to(&enc, &dest);
                }
            }
        }
    }
}
