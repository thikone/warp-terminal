use super::*;

#[test]
fn test_scroll_delta_for_axis_fully_into_view() {
    let mode = ScrollToPositionMode::FullyIntoView;
    assert_eq!(
        scroll_delta_for_axis(
            Axis::Horizontal,
            RectF::new(vec2f(100., 0.), vec2f(250., 250.)),
            RectF::new(vec2f(400., 50.), vec2f(50., 50.)),
            mode,
        ),
        100.
    );
    assert_eq!(
        scroll_delta_for_axis(
            Axis::Horizontal,
            RectF::new(vec2f(200., 0.), vec2f(250., 250.)),
            RectF::new(vec2f(100., 50.), vec2f(50., 50.)),
            mode,
        ),
        -100.
    );
    assert_eq!(
        scroll_delta_for_axis(
            Axis::Horizontal,
            RectF::new(vec2f(100., 0.), vec2f(250., 250.)),
            RectF::new(vec2f(325., 50.), vec2f(50., 50.)),
            mode,
        ),
        25.
    );
    assert_eq!(
        scroll_delta_for_axis(
            Axis::Horizontal,
            RectF::new(vec2f(100., 0.), vec2f(250., 250.)),
            RectF::new(vec2f(150., 50.), vec2f(50., 50.)),
            mode,
        ),
        0.
    );
    assert_eq!(
        scroll_delta_for_axis(
            Axis::Horizontal,
            RectF::new(vec2f(100., 0.), vec2f(250., 250.)),
            RectF::new(vec2f(50., 50.), vec2f(350., 50.)),
            mode,
        ),
        0.
    );
}

#[test]
fn test_scroll_delta_for_axis_top_into_view() {
    let mode = ScrollToPositionMode::TopIntoView;

    // --- Element LARGER than the viewport ---

    // Element taller than viewport, below viewport: align top with
    // viewport top.
    assert_eq!(
        scroll_delta_for_axis(
            Axis::Vertical,
            RectF::new(vec2f(0., 100.), vec2f(250., 250.)),
            RectF::new(vec2f(50., 400.), vec2f(50., 300.)),
            mode,
        ),
        300.
    );

    // Element taller than viewport, above viewport: align top with
    // viewport top.
    assert_eq!(
        scroll_delta_for_axis(
            Axis::Vertical,
            RectF::new(vec2f(0., 200.), vec2f(250., 250.)),
            RectF::new(vec2f(50., 100.), vec2f(50., 300.)),
            mode,
        ),
        -100.
    );

    // Element taller than viewport, top at viewport top: align top
    // (delta = 0).
    assert_eq!(
        scroll_delta_for_axis(
            Axis::Vertical,
            RectF::new(vec2f(0., 100.), vec2f(250., 250.)),
            RectF::new(vec2f(50., 100.), vec2f(50., 300.)),
            mode,
        ),
        0.
    );

    // Element taller than viewport, top visible but bottom extends
    // past: align top with viewport top (shows max content from top).
    assert_eq!(
        scroll_delta_for_axis(
            Axis::Vertical,
            RectF::new(vec2f(0., 100.), vec2f(250., 250.)),
            RectF::new(vec2f(50., 200.), vec2f(50., 300.)),
            mode,
        ),
        100.
    );

    // Element taller than viewport, spans entire viewport (top above,
    // bottom below): align top with viewport top.
    assert_eq!(
        scroll_delta_for_axis(
            Axis::Vertical,
            RectF::new(vec2f(0., 100.), vec2f(250., 250.)),
            RectF::new(vec2f(50., 50.), vec2f(50., 400.)),
            mode,
        ),
        -50.
    );

    // --- Element FITS in the viewport (delegates to FullyIntoView) ---

    // Small element below viewport: scroll down (bottom to viewport
    // bottom).
    assert_eq!(
        scroll_delta_for_axis(
            Axis::Vertical,
            RectF::new(vec2f(0., 100.), vec2f(250., 250.)),
            RectF::new(vec2f(50., 400.), vec2f(50., 50.)),
            mode,
        ),
        100.
    );

    // Small element above viewport: scroll up (top to viewport top).
    assert_eq!(
        scroll_delta_for_axis(
            Axis::Vertical,
            RectF::new(vec2f(0., 200.), vec2f(250., 250.)),
            RectF::new(vec2f(50., 100.), vec2f(50., 50.)),
            mode,
        ),
        -100.
    );

    // Small element fully visible: no scroll.
    assert_eq!(
        scroll_delta_for_axis(
            Axis::Vertical,
            RectF::new(vec2f(0., 100.), vec2f(250., 250.)),
            RectF::new(vec2f(50., 150.), vec2f(50., 50.)),
            mode,
        ),
        0.
    );
}

/// The numbers are taken from a real trace of the Session history panel, where
/// the content's bottom edge was 1166.6 and its own last row ended at 1166.6001.
/// An exact containment check rejected the row and the list stopped following new
/// commands for the rest of the session.
#[test]
fn position_flush_with_the_content_edge_is_still_within_it() {
    let child = RectF::from_points(vec2f(5., -56.6), vec2f(306., 1166.6));

    let flush = RectF::from_points(vec2f(5., 1111.0), vec2f(306., 1166.6));
    assert!(position_within_child(child, flush));

    // A hair past the edge, which is what float accumulation produces.
    let overshooting = RectF::from_points(vec2f(5., 1111.0001), vec2f(306., 1166.6001));
    assert!(position_within_child(child, overshooting));
}

#[test]
fn position_outside_the_content_is_rejected() {
    let child = RectF::from_points(vec2f(0., 0.), vec2f(300., 1000.));

    // Belonging to an unrelated element tree, which is what the check is for.
    assert!(!position_within_child(
        child,
        RectF::from_points(vec2f(0., 1200.), vec2f(300., 1260.))
    ));
    assert!(!position_within_child(
        child,
        RectF::from_points(vec2f(500., 10.), vec2f(560., 70.))
    ));
    // Tolerance is a hair, not a licence: half a row past the edge still fails.
    assert!(!position_within_child(
        child,
        RectF::from_points(vec2f(0., 980.), vec2f(300., 1030.))
    ));
}
