//! Spike 1 (throwaway): show a clock in the empty taskbar gap.
//!   taskbar-spike overlay [--opaque]   topmost popup over the gap
//!   taskbar-spike child   [--opaque]   SetParent child of Shell_TrayWnd
//!   taskbar-spike dump                 print the taskbar's UIA tree
//! Double-click the clock to quit.

#[cfg_attr(not(windows), allow(dead_code))]
mod layout;
#[cfg(windows)]
mod win;

slint::slint! {
    #[edition = "1.19"]
    export component Clock inherits Window {
        in property <string> time: "--:--:--";
        in property <bool> opaque: false;
        in property <string> mode-label;
        callback quit();
        no-frame: true;
        background: opaque ? #202020 : transparent;
        HorizontalLayout {
            padding-left: 8px;
            spacing: 8px;
            alignment: start;
            Rectangle {
                border-radius: 6px;
                background: #ffffff18;
                TouchArea { double-clicked => { root.quit(); } }
                HorizontalLayout {
                    padding-left: 10px;
                    padding-right: 10px;
                    spacing: 8px;
                    Text { text: root.time; color: white; font-size: 14px; vertical-alignment: center; }
                    Text { text: root.mode-label; color: #ffffff90; font-size: 11px; vertical-alignment: center; }
                }
            }
        }
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("Windows 11 only. `cargo test` runs the layout tests on any OS.");
}

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use std::sync::atomic::{AtomicIsize, Ordering};
    use std::time::Duration;
    use win::Mode;

    static OUR_HWND: AtomicIsize = AtomicIsize::new(0);
    const MARGIN: i32 = 8;
    const MIN_WIDTH: i32 = 120;
    const SCAN_EVERY: Duration = Duration::from_millis(1000);

    let args: Vec<String> = std::env::args().skip(1).collect();
    let opaque = args.iter().any(|a| a == "--opaque");
    let mode = match args.first().map(String::as_str) {
        Some("overlay") => Mode::Overlay,
        Some("child") => Mode::Child,
        Some("dump") => {
            win::Uia::new()?.dump()?;
            return Ok(());
        }
        _ => {
            eprintln!("usage: taskbar-spike overlay|child|dump [--opaque]");
            std::process::exit(2);
        }
    };

    let ui = Clock::new()?;
    ui.set_opaque(opaque);
    ui.set_mode_label(format!("{mode:?}").to_lowercase().into());
    ui.on_quit(|| {
        let _ = slint::quit_event_loop();
    });

    let tick = slint::Timer::default();
    let weak = ui.as_weak();
    tick.start(slint::TimerMode::Repeated, Duration::from_millis(250), move || {
        if let Some(ui) = weak.upgrade() {
            ui.set_time(chrono::Local::now().format("%H:%M:%S").to_string().into());
        }
    });

    ui.show()?;

    // winit creates the native window once the event loop runs: attach shortly after.
    let weak = ui.as_weak();
    slint::Timer::single_shot(Duration::from_millis(200), move || {
        let Some(ui) = weak.upgrade() else { return };
        let handle = ui.window().window_handle();
        let raw = match handle.window_handle().map(|h| h.as_raw()) {
            Ok(RawWindowHandle::Win32(h)) => h.hwnd.get(),
            other => {
                eprintln!("no Win32 handle: {other:?}");
                let _ = slint::quit_event_loop();
                return;
            }
        };
        if let Err(e) = win::attach(win::hwnd_from_raw(raw), mode) {
            eprintln!("attach failed: {e}");
            let _ = slint::quit_event_loop();
            return;
        }
        OUR_HWND.store(raw, Ordering::SeqCst);
        println!("attached {raw:#x} as {mode:?}");
    });

    // UIA scanning off the UI thread; results are applied on the UI thread.
    std::thread::spawn(move || {
        let uia = match win::Uia::new() {
            Ok(u) => u,
            Err(e) => return eprintln!("UIA init failed: {e}"),
        };
        let mut last = None;
        loop {
            let scan = uia.scan();
            let gap = scan.and_then(|(s, _)| layout::gap_rect(&s, MARGIN, MIN_WIDTH));
            if scan != last {
                println!("scan: {scan:?} -> gap {gap:?}");
                last = scan;
            }
            let taskbar = scan.map(|(s, _)| s.taskbar);
            let _ = slint::invoke_from_event_loop(move || {
                let raw = OUR_HWND.load(Ordering::SeqCst);
                if raw == 0 {
                    return;
                }
                let hwnd = win::hwnd_from_raw(raw);
                match (gap, taskbar) {
                    (Some(g), Some(tb)) => win::place(hwnd, mode, g, tb),
                    _ => win::hide(hwnd),
                }
            });
            std::thread::sleep(SCAN_EVERY);
        }
    });

    slint::run_event_loop_until_quit()?;
    Ok(())
}
