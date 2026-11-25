//! Pure gap maths, unit-tested on any OS.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    pub fn width(&self) -> i32 {
        self.right - self.left
    }
    pub fn height(&self) -> i32 {
        self.bottom - self.top
    }
    /// Same rect expressed relative to `origin` (screen -> parent-client coords).
    pub fn relative_to(&self, origin: Rect) -> Rect {
        Rect {
            left: self.left - origin.left,
            top: self.top - origin.top,
            right: self.right - origin.left,
            bottom: self.bottom - origin.top,
        }
    }
}

/// What the UIA scan found on the taskbar, all in physical screen pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskbarScan {
    pub taskbar: Rect,
    /// Right edge of Windows' Widgets/weather button, if present.
    pub widgets_right: Option<i32>,
    /// Left edge of the centered icon group (the Start button).
    pub icons_left: Option<i32>,
}

/// Screen rect of the empty gap between the Widgets button (or the taskbar's
/// left edge) and the centered icons, shrunk by `margin` on both sides.
/// `None` when there is no usable gap (left-aligned taskbar, missing Start
/// button, or narrower than `min_width`).
pub fn gap_rect(scan: &TaskbarScan, margin: i32, min_width: i32) -> Option<Rect> {
    let tb = scan.taskbar;
    let icons_left = scan.icons_left?;
    let left = scan.widgets_right.unwrap_or(tb.left).max(tb.left) + margin;
    let right = icons_left.min(tb.right) - margin;
    if right - left < min_width {
        return None;
    }
    Some(Rect { left, top: tb.top, right, bottom: tb.bottom })
}

#[cfg(test)]
mod tests {
    use super::*;

    // 2560x1440 at 150%: taskbar 72 px high.
    const TB: Rect = Rect { left: 0, top: 1368, right: 2560, bottom: 1440 };

    fn scan(widgets_right: Option<i32>, icons_left: Option<i32>) -> TaskbarScan {
        TaskbarScan { taskbar: TB, widgets_right, icons_left }
    }

    #[test]
    fn centered_with_widgets_button() {
        let g = gap_rect(&scan(Some(300), Some(1000)), 8, 40).unwrap();
        assert_eq!(g, Rect { left: 308, top: 1368, right: 992, bottom: 1440 });
    }

    #[test]
    fn no_widgets_button_starts_at_taskbar_edge() {
        let g = gap_rect(&scan(None, Some(1000)), 8, 40).unwrap();
        assert_eq!(g.left, 8);
    }

    #[test]
    fn left_aligned_taskbar_has_no_gap() {
        assert_eq!(gap_rect(&scan(Some(300), Some(12)), 8, 40), None);
    }

    #[test]
    fn missing_start_button_means_no_gap() {
        assert_eq!(gap_rect(&scan(Some(300), None), 8, 40), None);
    }

    #[test]
    fn too_narrow_gap_is_rejected() {
        assert_eq!(gap_rect(&scan(Some(300), Some(350)), 8, 40), None);
    }

    #[test]
    fn edges_are_clamped_to_taskbar() {
        let g = gap_rect(&scan(Some(-50), Some(9000)), 0, 40).unwrap();
        assert_eq!((g.left, g.right), (0, 2560));
    }

    #[test]
    fn relative_to_taskbar_on_secondary_origin() {
        let tb = Rect { left: -1920, top: 1032, right: 0, bottom: 1080 };
        let r = Rect { left: -1500, top: 1032, right: -1000, bottom: 1080 };
        assert_eq!(r.relative_to(tb), Rect { left: 420, top: 0, right: 920, bottom: 48 });
    }
}
