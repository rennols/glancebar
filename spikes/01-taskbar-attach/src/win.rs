//! Thin Win32/UIA wrappers. Reads the taskbar via UI Automation (out of
//! process, no injection) and restyles/positions our own window only.

use crate::layout::{Rect, TaskbarScan};
use windows::core::{w, BSTR};
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationElement, TreeScope_Descendants,
    UIA_AutomationIdPropertyId,
};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, GetWindowLongPtrW, GetWindowRect, SetParent, SetWindowLongPtrW, SetWindowPos,
    ShowWindow, GWL_EXSTYLE, GWL_STYLE, HWND_TOP, HWND_TOPMOST, SWP_NOACTIVATE, SWP_SHOWWINDOW,
    SW_HIDE, SW_SHOWNOACTIVATE, WS_CAPTION, WS_CHILD, WS_CLIPSIBLINGS, WS_EX_APPWINDOW,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP, WS_THICKFRAME,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Overlay,
    Child,
}

pub fn hwnd_from_raw(raw: isize) -> HWND {
    HWND(raw as *mut _)
}

fn rect(r: RECT) -> Rect {
    Rect { left: r.left, top: r.top, right: r.right, bottom: r.bottom }
}

pub fn find_taskbar() -> Option<HWND> {
    unsafe { FindWindowW(w!("Shell_TrayWnd"), None) }.ok()
}

pub fn taskbar_rect(tb: HWND) -> Option<Rect> {
    let mut r = RECT::default();
    unsafe { GetWindowRect(tb, &mut r) }.ok()?;
    Some(rect(r))
}

/// UI Automation client. Create and use it on a non-UI thread: in child mode
/// our own window is inside the tree being walked.
pub struct Uia(IUIAutomation);

impl Uia {
    pub fn new() -> windows::core::Result<Self> {
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
            Ok(Self(CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)?))
        }
    }

    fn find_by_id(&self, root: &IUIAutomationElement, id: &str) -> Option<Rect> {
        unsafe {
            let cond = self
                .0
                .CreatePropertyCondition(UIA_AutomationIdPropertyId, &VARIANT::from(BSTR::from(id)))
                .ok()?;
            let el = root.FindFirst(TreeScope_Descendants, &cond).ok()?;
            Some(rect(el.CurrentBoundingRectangle().ok()?))
        }
    }

    pub fn scan(&self) -> Option<(TaskbarScan, isize)> {
        let tb = find_taskbar()?;
        let taskbar = taskbar_rect(tb)?;
        let root = unsafe { self.0.ElementFromHandle(tb) }.ok()?;
        let widgets_right = self.find_by_id(&root, "WidgetsButton").map(|r| r.right);
        let icons_left = self.find_by_id(&root, "StartButton").map(|r| r.left);
        Some((TaskbarScan { taskbar, widgets_right, icons_left }, tb.0 as isize))
    }

    /// `--dump`: print every UIA element under the taskbar, so wrong ids are easy to fix.
    pub fn dump(&self) -> windows::core::Result<()> {
        let tb = find_taskbar().ok_or_else(windows::core::Error::empty)?;
        println!("Shell_TrayWnd {:?} rect {:?}", tb.0, taskbar_rect(tb));
        unsafe {
            let root = self.0.ElementFromHandle(tb)?;
            let all = root.FindAll(TreeScope_Descendants, &self.0.CreateTrueCondition()?)?;
            for i in 0..all.Length()? {
                let e = all.GetElement(i)?;
                println!(
                    "id={:<28} class={:<40} name={:<30} rect={:?}",
                    e.CurrentAutomationId().unwrap_or_default().to_string(),
                    e.CurrentClassName().unwrap_or_default().to_string(),
                    e.CurrentName().unwrap_or_default().to_string(),
                    e.CurrentBoundingRectangle().map(rect).ok(),
                );
            }
        }
        Ok(())
    }
}

/// Restyle our freshly created Slint/winit window for the chosen mode.
pub fn attach(hwnd: HWND, mode: Mode) -> windows::core::Result<()> {
    unsafe {
        let _ = ShowWindow(hwnd, SW_HIDE);
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        let ex = (ex | WS_EX_TOOLWINDOW.0 | WS_EX_NOACTIVATE.0) & !WS_EX_APPWINDOW.0;
        match mode {
            Mode::Overlay => {
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, (ex | WS_EX_TOPMOST.0) as isize);
            }
            Mode::Child => {
                let tb = find_taskbar().ok_or_else(windows::core::Error::empty)?;
                let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
                let style = (style & !(WS_POPUP.0 | WS_CAPTION.0 | WS_THICKFRAME.0))
                    | WS_CHILD.0
                    | WS_CLIPSIBLINGS.0;
                SetWindowLongPtrW(hwnd, GWL_STYLE, style as isize);
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, (ex & !WS_EX_TOPMOST.0) as isize);
                SetParent(hwnd, Some(tb))?;
            }
        }
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
    }
    Ok(())
}

/// Move/resize our window onto `gap` (screen coords) and re-assert z-order.
/// Called every scan tick, which also re-raises the overlay after the
/// taskbar has been clicked.
pub fn place(hwnd: HWND, mode: Mode, gap: Rect, taskbar: Rect) {
    let (after, r) = match mode {
        Mode::Overlay => (HWND_TOPMOST, gap),
        Mode::Child => (HWND_TOP, gap.relative_to(taskbar)),
    };
    let flags = SWP_NOACTIVATE | SWP_SHOWWINDOW;
    if let Err(e) =
        unsafe { SetWindowPos(hwnd, Some(after), r.left, r.top, r.width(), r.height(), flags) }
    {
        eprintln!("SetWindowPos failed: {e}");
    }
}

pub fn hide(hwnd: HWND) {
    unsafe {
        let _ = ShowWindow(hwnd, SW_HIDE);
    }
}
