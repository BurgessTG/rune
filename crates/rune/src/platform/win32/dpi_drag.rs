//! Dragging a window between displays with different scaling. Windows sends `WM_DPICHANGED`
//! with a rect it worked out for the new scale, and expects the window to take it. winit sizes
//! the window itself, keeps the pointer's place along the title bar, then nudges the window
//! until most of it is on the new display. Mid-drag that nudge moves the window out from
//! under the pointer; the move loop puts it back under the pointer, mostly on the old display,
//! which changes the scale back, and the window grows and shrinks at the boundary instead of
//! crossing it (hex#1355). While the user moves or sizes the window, it takes Windows' rect
//! after winit has handled the message, so winit still learns the new scale. A window with a
//! custom frame (`frame`) lands on Windows' rect in winit's own move, so it needs no second.
//!
//! The move or size loop is also when a resize draws its frame at once (`in_size_move`).

use std::cell::RefCell;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    SWP_NOACTIVATE, SWP_NOZORDER, SetWindowPos, WM_DPICHANGED, WM_ENTERSIZEMOVE, WM_EXITSIZEMOVE, WM_NCDESTROY,
};

const SUBCLASS: usize = 0x5255_4e44;

thread_local! {
    /// The windows on this thread inside a move or size loop.
    static MOVING: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
}

/// Makes `window` take the rect Windows suggests when its scale changes during a move or size.
pub(crate) fn follow_suggested_rect(window: &winit::window::Window) {
    let Some(hwnd) = super::hwnd(window) else { return };
    // SAFETY: the subclass reads no data, and goes at WM_NCDESTROY.
    unsafe { SetWindowSubclass(hwnd, Some(subclass), SUBCLASS, 0); }
}

/// Whether the user is moving or sizing `window` by its frame (`WM_ENTERSIZEMOVE` until
/// `WM_EXITSIZEMOVE`), in the system's modal loop.
pub(crate) fn in_size_move(window: &winit::window::Window) -> bool {
    super::hwnd(window).is_some_and(moving)
}

fn moving(hwnd: HWND) -> bool {
    MOVING.with(|m| m.borrow().contains(&(hwnd as usize)))
}

static T0: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
fn ib3_top(hwnd: HWND) -> bool { unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetWindowLongW(hwnd, windows_sys::Win32::UI::WindowsAndMessaging::GWL_EXSTYLE) as u32 & 8 != 0 } }
unsafe extern "system" fn subclass(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM, _: usize, _: usize) -> LRESULT {
    if (message == 0x46 || message == 0x47) && std::env::var_os("IB3_TOPMOST").is_some() {
        let t = T0.get_or_init(std::time::Instant::now).elapsed().as_micros();
        let pos = unsafe { &*(lparam as *const windows_sys::Win32::UI::WindowsAndMessaging::WINDOWPOS) };
        let (after0, flags0) = (pos.hwndInsertAfter as isize, pos.flags);
        let sent = unsafe { windows_sys::Win32::UI::WindowsAndMessaging::InSendMessageEx(std::ptr::null_mut()) };
        let top0 = ib3_top(hwnd);
        let result = unsafe { DefSubclassProc(hwnd, message, wparam, lparam) };
        let pos = unsafe { &*(lparam as *const windows_sys::Win32::UI::WindowsAndMessaging::WINDOWPOS) };
        let bt = if sent == 0 && flags0 & SWP_NOZORDER == 0 { format!("\n{}", std::backtrace::Backtrace::force_capture()) } else { String::new() };
        eprintln!("ib3top {t} {} sent={sent:x} after={after0}->{} flags={flags0:x}->{:x} top {top0}->{}{bt}", if message == 0x46 { "CHANGING" } else { "CHANGED" }, pos.hwndInsertAfter as isize, pos.flags, ib3_top(hwnd));
        return result;
    }
    match message {
        WM_ENTERSIZEMOVE => MOVING.with(|m| m.borrow_mut().push(hwnd as usize)),
        WM_EXITSIZEMOVE => MOVING.with(|m| m.borrow_mut().retain(|&h| h != hwnd as usize)),
        // SAFETY: WM_DPICHANGED's lparam points to the suggested RECT.
        WM_DPICHANGED if moving(hwnd) && !super::frame::installed(hwnd) => unsafe {
            let suggested = *(lparam as *const RECT);
            let result = DefSubclassProc(hwnd, message, wparam, lparam);
            let RECT { left, top, right, bottom } = suggested;
            SetWindowPos(hwnd, std::ptr::null_mut(), left, top, right - left, bottom - top, SWP_NOZORDER | SWP_NOACTIVATE);
            return result;
        },
        WM_NCDESTROY => {
            MOVING.with(|m| m.borrow_mut().retain(|&h| h != hwnd as usize));
            // SAFETY: removes this subclass from the window it's on.
            unsafe { RemoveWindowSubclass(hwnd, Some(subclass), SUBCLASS); }
        }
        _ => {}
    }
    // SAFETY: forwards to the next window procedure.
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}
