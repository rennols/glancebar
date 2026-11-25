# Spike 1: taskbar attachment (throwaway)

Shows a clock in the empty gap of the Windows 11 taskbar, between the
Widgets/weather button and the centered Start button, in two variants:

- `overlay`: our own top-level popup, `WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE`,
  positioned over the gap. Z-order is re-asserted every second.
- `child`: our window restyled to `WS_CHILD` and `SetParent`-ed into `Shell_TrayWnd`,
  positioned in taskbar client coordinates.

The gap is found with UI Automation (out-of-process client, no injection):
`AutomationId=WidgetsButton` gives the left edge, `AutomationId=StartButton` the
right edge. Rescanned every second on a worker thread. Nothing in explorer.exe
is modified except, in `child` mode, the parent of our own window.

```
src/main.rs    Slint UI, args, timers, scan thread -> UI thread
src/layout.rs  gap maths (pure, unit-tested)
src/win.rs     Win32 + UIA wrappers
```

## Build and run (Windows 11)

One-time toolchain:

```powershell
winget install Rustlang.Rustup
winget install Microsoft.VisualStudio.2022.BuildTools --override "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
# new terminal
rustup default stable-x86_64-pc-windows-msvc
```

Build:

```powershell
git clone https://github.com/rennols/glancebar
cd glancebar
git checkout claude/project-thread-koy22j
cd spikes\01-taskbar-attach
cargo build --release
```

Run (from a terminal, it logs to stdout):

```powershell
.\target\release\taskbar-spike.exe dump      # list taskbar UIA elements (send me this if the clock doesn't appear)
.\target\release\taskbar-spike.exe overlay   # variant (a)
.\target\release\taskbar-spike.exe child     # variant (b)
```

- Double-click the clock to quit. `Ctrl+C` in the terminal also works.
- Add `--opaque` if the background renders black/garbage instead of transparent
  (likely in `child` mode); it paints a dark solid background so the rest of the
  checklist can still be tested. Note it in the results.
- Renderer: default is FemtoVG (OpenGL). To compare, set
  `$env:SLINT_BACKEND="winit-software"` before running.
- `cargo test` runs the layout tests (works on any OS).

## Checklist

**Provisional.** The design doc's section 8 checklist replaces this table once I
have the doc. Fill in `pass` / `fail` / notes per variant.

| # | Check | How to test | (a) overlay | (b) child |
|---|-------|-------------|-------------|-----------|
| 1 | Clock appears in the gap, vertically centered, right of Widgets, left of Start | Run, look | | |
| 2 | Never overlaps Widgets button, Start, Search, app icons or tray, even partly | Zoom with Win+Plus on both edges | | |
| 3 | Background is transparent (taskbar Mica/acrylic shows through), no black box or flicker | Look, also with light and dark mode | | |
| 4 | Text is crisp at your DPI | Look; also try 100/125/150% scale if you can | | |
| 5 | Follows the gap when icons are added/removed | Open/close 3-4 apps that aren't pinned | | |
| 6 | Stays visible after clicking the taskbar, Start, Search, tray, a taskbar icon | Click each, watch the clock | | |
| 7 | Stays visible with Start menu, Search, Widgets panel, Action Center open (and doesn't draw over them) | Open each | | |
| 8 | Hidden (or at least not drawn over) a fullscreen app/video and a fullscreen game | F11 in browser, fullscreen video | | |
| 9 | Behaves with taskbar auto-hide (hides with taskbar, returns with it) | Settings > Taskbar > Automatically hide | | |
| 10 | Clicking the clock doesn't steal focus from the active app | Type in Notepad, click clock once, keep typing | | |
| 11 | No taskbar button / Alt+Tab entry for the spike itself | Look at taskbar, Alt+Tab | | |
| 12 | Survives explorer restart (comes back or exits cleanly) | Task Manager > Windows Explorer > Restart | | |
| 13 | Taskbar exactly as before after normal quit | Double-click clock, inspect taskbar | | |
| 14 | Taskbar exactly as before after a hard kill | `taskkill /F /IM taskbar-spike.exe` | | |
| 15 | Explorer stays responsive if the spike hangs | Run, then `Debug > Break All` in a debugger or suspend the process in Resource Monitor (Analyze/Suspend), click around the taskbar for 10 s, resume | | |
| 16 | Display changes: resolution, scale, sleep/resume | Change scale in Settings, sleep and wake | | |
| 17 | Resource use | Task Manager: memory (private working set) and CPU after 5 min | | |

Known expectations going in:
- (a) overlay will probably fail #8 (topmost floats over fullscreen apps) and #9 until we add
  logic for it; that's fixable in app code.
- (b) child shares an input queue with explorer (`SetParent` across processes attaches thread
  input), so #15 is the critical one. It may also fail #3 (no DWM transparency for child windows)
  and #12 (our window dies with the taskbar).
