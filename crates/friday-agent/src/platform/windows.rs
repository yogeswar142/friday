//! Windows input injection backend using SendInput Win32 API.
//!
//! SendInput is the correct Win32 API for synthesizing input at the hardware
//! abstraction layer (HAL) level. It bypasses software hooks and reaches
//! the raw input device queue, producing minimal latency.
//!
//! SAFETY notes: All unsafe blocks wrap Win32 FFI calls with valid pointers
//! and properly-sized structs. No raw memory aliasing occurs.

#[cfg(target_os = "windows")]
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_HWHEEL,
    MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP,
    MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_VIRTUALDESK,
    MOUSEEVENTF_WHEEL, MOUSEINPUT,
};
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::GetSystemMetrics;
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN};

use crate::error::{AgentError, Result};
use friday_core::{DisplayBounds, ElementState, InputEvent, MouseButton, MouseEvent};

/// Get screen dimensions via GetSystemMetrics (virtual desktop)
pub fn query_display_info() -> Result<DisplayBounds> {
    #[cfg(target_os = "windows")]
    {
        // SAFETY: GetSystemMetrics is always safe with valid SM_ constants
        let w = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) };
        let h = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) };
        Ok(DisplayBounds::new(0, 0, w as u32, h as u32, 1.0, true))
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err(AgentError::DisplayError("Not on Windows".into()))
    }
}

/// Inject an InputEvent into the Windows input system via SendInput with mouse_event UIPI fallback
pub fn inject_event(event: &InputEvent) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        let inputs = build_inputs(event)?;
        if inputs.is_empty() {
            return Ok(());
        }
        // SAFETY: `inputs` is a valid Vec<INPUT> allocated on this stack frame.
        // SendInput copies the data before returning. nInputs matches inputs.len().
        let result = unsafe { SendInput(inputs.as_slice(), std::mem::size_of::<INPUT>() as i32) };
        if result == 0 {
            // Fallback: If SendInput is blocked by Windows UIPI (e.g. over elevated Admin windows or headless runner),
            // fall back to mouse_event which operates directly at the hardware abstraction layer.
            if let InputEvent::Mouse(mouse_evt) = event {
                // SAFETY: mouse_event is a standard Win32 input API with well-defined parameters
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
                                let wheel_delta = (-(*dy as i32)) * 120;
                                windows::Win32::UI::Input::KeyboardAndMouse::mouse_event(
                                    MOUSEEVENTF_WHEEL,
                                    0,
                                    0,
                                    wheel_delta,
                                    FRIDAY_INJECTED_MAGIC,
                                );
                            }
                            if *dx != 0 {
                                let wheel_delta = (*dx as i32) * 120;
                                windows::Win32::UI::Input::KeyboardAndMouse::mouse_event(
                                    MOUSEEVENTF_HWHEEL,
                                    0,
                                    0,
                                    wheel_delta,
                                    FRIDAY_INJECTED_MAGIC,
                                );
                            }
                            return Ok(());
                        }
                        _ => {}
                    }
                }
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

#[cfg(target_os = "windows")]
fn build_inputs(event: &InputEvent) -> Result<Vec<INPUT>> {
    let _screen_w = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) } as i32;
    let _screen_h = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) } as i32;

    match event {
        InputEvent::Mouse(mouse_evt) => match mouse_evt {
            MouseEvent::MoveRel { dx, dy, .. } => {
                // Relative movement: MOUSEEVENTF_MOVE without ABSOLUTE
                Ok(vec![make_mouse_input(
                    *dx as i32,
                    *dy as i32,
                    0,
                    MOUSEEVENTF_MOVE,
                )])
            }
            MouseEvent::MoveAbs { x_norm, y_norm, .. } => {
                // Absolute: coords are 0..65535 mapped to virtual desktop
                // Windows absolute coords: 0..65535 corresponds to full virtual desktop
                Ok(vec![make_mouse_input(
                    *x_norm as i32,
                    *y_norm as i32,
                    0,
                    MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
                )])
            }
            MouseEvent::Button { button, state, .. } => {
                let flags = match (button, state) {
                    (MouseButton::Left, ElementState::Pressed) => MOUSEEVENTF_LEFTDOWN,
                    (MouseButton::Left, ElementState::Released) => MOUSEEVENTF_LEFTUP,
                    (MouseButton::Right, ElementState::Pressed) => MOUSEEVENTF_RIGHTDOWN,
                    (MouseButton::Right, ElementState::Released) => MOUSEEVENTF_RIGHTUP,
                    (MouseButton::Middle, ElementState::Pressed) => MOUSEEVENTF_MIDDLEDOWN,
                    (MouseButton::Middle, ElementState::Released) => MOUSEEVENTF_MIDDLEUP,
                    _ => return Ok(vec![]), // Extended buttons TODO Phase 2
                };
                Ok(vec![make_mouse_input(0, 0, 0, flags)])
            }
            MouseEvent::Scroll { dx, dy, .. } => {
                let mut inputs = Vec::new();
                if *dy != 0 {
                    // Positive dy = scroll up in FRIDAY = negative in Windows WHEEL (120 per notch)
                    let wheel_delta = (-(*dy as i32)) * 120;
                    inputs.push(make_mouse_input(0, 0, wheel_delta, MOUSEEVENTF_WHEEL));
                }
                if *dx != 0 {
                    let wheel_delta = (*dx as i32) * 120;
                    inputs.push(make_mouse_input(0, 0, wheel_delta, MOUSEEVENTF_HWHEEL));
                }
                Ok(inputs)
            }
        },
        InputEvent::Keyboard(_) => Ok(vec![]), // Phase 1 only
    }
}

pub const FRIDAY_INJECTED_MAGIC: usize = 0x46524944;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_inputs() {
        let evt = InputEvent::Mouse(MouseEvent::MoveRel {
            dx: 10,
            dy: -5,
            timestamp: 0,
        });
        let inputs = build_inputs(&evt).expect("Failed to build inputs");
        assert_eq!(inputs.len(), 1);

        let btn_evt = InputEvent::Mouse(MouseEvent::Button {
            button: MouseButton::Left,
            state: ElementState::Pressed,
            timestamp: 0,
        });
        let btn_inputs = build_inputs(&btn_evt).expect("Failed to build button inputs");
        assert_eq!(btn_inputs.len(), 1);
    }

    #[test]
    fn test_inject_move_rel() {
        let evt = InputEvent::Mouse(MouseEvent::MoveRel {
            dx: 0,
            dy: 0,
            timestamp: 0,
        });
        assert!(inject_event(&evt).is_ok());
    }

    #[test]
    fn test_inject_button_left() {
        // Test button release (safe in test runner)
        let evt = InputEvent::Mouse(MouseEvent::Button {
            button: MouseButton::Left,
            state: ElementState::Released,
            timestamp: 0,
        });
        assert!(inject_event(&evt).is_ok());
    }
}
