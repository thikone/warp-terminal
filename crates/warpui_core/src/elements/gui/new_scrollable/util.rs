use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::vector::{Vector2F, vec2f};

use crate::elements::{
    Axis, ClippedScrollStateHandle, RectFExt as _, ScrollToPositionMode,
    project_scroll_delta_by_sensitivity,
};
use crate::units::Pixels;
use crate::{EventContext, SizeConstraint};

/// Calculate the child size constraint for a given axis.
/// For a clipped element, lay it out unbounded on the main axis but apply constraint on the cross axis.
/// For a manual element, lay it out bounded with the incoming size constraint. Note that we need to
/// subtract the total scrollbar offset to take into account the spacing it takes in the viewport.
pub(super) fn child_constraint_for_axis(
    axis: Axis,
    constraint: SizeConstraint,
    is_clipped: bool,
    scrollbar_size_with_padding: Vector2F,
) -> SizeConstraint {
    let incoming_constraint = if is_clipped {
        match axis {
            Axis::Horizontal => SizeConstraint {
                min: vec2f(0.0, constraint.min.y()),
                max: vec2f(f32::INFINITY, constraint.max.y()),
            },
            Axis::Vertical => SizeConstraint {
                min: vec2f(constraint.min.x(), 0.),
                max: vec2f(constraint.max.x(), f32::INFINITY),
            },
        }
    } else {
        constraint
    };

    SizeConstraint {
        min: (incoming_constraint.min - scrollbar_size_with_padding).max(Vector2F::zero()),
        max: (incoming_constraint.max - scrollbar_size_with_padding).max(Vector2F::zero()),
    }
}

/// Update the ClippedScrollStateHandle to match scrolling with the given delta.
pub(super) fn scroll_clipped_scrollable_handle_with_delta(
    handle: &ClippedScrollStateHandle,
    child_size: Pixels,
    viewport_size: Pixels,
    delta: Pixels,
    ctx: &mut EventContext,
) {
    let scroll_start = handle.scroll_start();

    if child_size > viewport_size {
        // The max scroll start here is the total child size - viewport size.
        // ==================
        // |                |
        // |                |
        // | max_scroll_top |
        // |                |
        // |                |
        // ==================
        // |    viewport    |
        // ==================
        let new_scroll_start = (scroll_start - delta)
            .max(Pixels::zero())
            .min(child_size - viewport_size);

        // If the scroll start positions have changed, scroll and re-render.
        if (scroll_start - new_scroll_start).as_f32().abs() > f32::EPSILON {
            handle.scroll_to(new_scroll_start);
            ctx.notify();
        }
    }
}

/// Adjust scroll delta based on the set sensitivity level:
/// - If horizontal delta * sensitivity > vertical delta, set vertical delta to zero.
/// - If vertical delta * sensitivity > horizontal delta, set horizontal delta to zero.
pub(super) fn adjust_scroll_delta_with_sensitivity_config(
    delta: Vector2F,
    sensitivity: f32,
) -> Vector2F {
    project_scroll_delta_by_sensitivity(delta, sensitivity)
}

//           Viewport
//       ┌──────┴───────┐
// ┌─────┲━━━━━━━━━━━━━━┱────────┐ ┐
// │     ┃              ┃        │ │
// │     ┃              ┃        │ │
// │     ┃              ┃        │ │
// │     ┃              ┃  ┌──┐  │ │
// │     ┃              ┃  │**│  │ ├─Viewport
// │     ┃              ┃  └──┘  │ │
// │     ┃              ┃        │ │
// │     ┃              ┃        │ │
// │     ┃              ┃        │ │
// │     ┗━━━━━━━━━━━━━━┛        │ ┘
// │                             │
// │                             │
// │                             │
// └─────────────────────────────┘
//                 Viewport
//             ┌──────┴───────┐
//        delta
//       ┌──┴──┐
// ┌───────────┲━━━━━━━━━━━━━━┱──┐ ┐
// │           ┃              ┃  │ │
// │           ┃              ┃  │ │
// │           ┃              ┃  │ │
// │           ┃           ┌──┨  │ │
// │           ┃           │**┃  │ ├─Viewport
// │           ┃           └──┨  │ │
// │           ┃              ┃  │ │
// │           ┃              ┃  │ │
// │           ┃              ┃  │ │
// │           ┗━━━━━━━━━━━━━━┛  │ ┘
// │                             │
// │                             │
// │                             │
// └─────────────────────────────┘
/// Calculate the scroll delta (in pixels) needed to bring the element delimited by
/// `position_bounds` into view within `viewport_bounds` on the given axis.
///
/// The behaviour depends on `mode`:
/// - [`ScrollToPositionMode::FullyIntoView`]: scrolls the minimum amount to make the
///   entire element visible. When the element is larger than the viewport, no scroll
///   is performed.
/// - [`ScrollToPositionMode::TopIntoView`]: behaves like `FullyIntoView` when the
///   element fits in the viewport. When the element is larger, aligns the element's
///   leading edge with the viewport's leading edge.
/// Whether `position_bounds` belongs to the scrollable's content.
///
/// The check exists to reject a position from an unrelated element tree, so it is
/// deliberately tolerant. A scrollable's last child sits flush against the edge of
/// the content that holds it, and the two rects are accumulated along different
/// paths, so after enough children float rounding leaves the child a fraction
/// larger than its own container -- observed as a content edge of 1166.6 against a
/// last child ending at 1166.6001, roughly 20 rows in.
///
/// [`RectF::contains_rect`] is an exact comparison, and that fraction is the
/// difference between following the newest row and silently never scrolling again:
/// once the error appears it recurs for every later row, because each new one is
/// also flush to that same edge.
pub(crate) fn position_within_child(child_bounds: RectF, position_bounds: RectF) -> bool {
    /// Far below anything that distinguishes a real child from a stray position,
    /// and far above the rounding error being absorbed.
    const TOLERANCE: f32 = 1.0;

    [Axis::Horizontal, Axis::Vertical].into_iter().all(|axis| {
        position_bounds.min_along(axis) >= child_bounds.min_along(axis) - TOLERANCE
            && position_bounds.max_along(axis) <= child_bounds.max_along(axis) + TOLERANCE
    })
}

pub(crate) fn scroll_delta_for_axis(
    axis: Axis,
    viewport_bounds: RectF,
    position_bounds: RectF,
    mode: ScrollToPositionMode,
) -> f32 {
    let viewport_max_along_axis = viewport_bounds.max_along(axis);
    let viewport_min_along_axis = viewport_bounds.min_along(axis);
    let max_position_along_axis = position_bounds.max_along(axis);
    let min_position_along_axis = position_bounds.min_along(axis);

    let viewport_size = viewport_max_along_axis - viewport_min_along_axis;
    let element_size = max_position_along_axis - min_position_along_axis;

    if element_size > viewport_size {
        match mode {
            ScrollToPositionMode::FullyIntoView => 0.0,
            ScrollToPositionMode::TopIntoView => min_position_along_axis - viewport_min_along_axis,
        }
    } else if max_position_along_axis > viewport_max_along_axis {
        max_position_along_axis - viewport_max_along_axis
    } else if min_position_along_axis < viewport_min_along_axis {
        min_position_along_axis - viewport_min_along_axis
    } else {
        0.0
    }
}

#[cfg(test)]
#[path = "util_tests.rs"]
mod tests;
