//! A dashed border, drawn as a canvas stroke layered over a surface.
//!
//! `iced::Border` is colour, width and radius only — it carries no dash
//! pattern — so a dashed border cannot be part of the quad a `container`
//! draws. `box border-dash=(..)` therefore lowers to this helper: the surface
//! keeps its background, radius and shadow, its *solid* border is dropped, and
//! the dashed stroke is drawn on top of the same rectangle. Layout is
//! untouched: the surface is the base layer of a stack, so the stack takes its
//! size, and the stroke only fills what the surface already measured.
use iced::border::Radius;
use iced::widget::canvas;
use iced::{Color, Element, Length, Point, Rectangle, Renderer, Size, Theme, mouse};
use std::cell::RefCell;

/// Layers a dashed rounded-rectangle stroke over `content`.
///
/// `segments` is the on/off dash pattern in pixels, and `radius` is the
/// surface's own corner radius; the stroke is inset by half of `width` so it
/// lands inside the surface bounds, the way a CSS border does.
pub fn dashed_border<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    color: Color,
    width: f32,
    radius: Radius,
    segments: Vec<f32>,
) -> Element<'a, Message> {
    let stroke = canvas(DashedBorder::new(color, width, radius, segments))
        .width(Length::Fill)
        .height(Length::Fill);
    iced::widget::stack([content.into(), Element::from(stroke)]).into()
}

/// The canvas program [`dashed_border`] layers over the surface.
///
/// Public only so the crate's allocation contract can call `Program::draw`
/// directly; the view builds it through [`dashed_border`].
#[doc(hidden)]
#[derive(Clone, PartialEq)]
pub struct DashedBorder {
    color: Color,
    width: f32,
    radius: Radius,
    segments: Vec<f32>,
}

impl DashedBorder {
    pub fn new(color: Color, width: f32, radius: Radius, segments: Vec<f32>) -> Self {
        Self {
            color,
            width,
            radius,
            segments: normalize_segments(segments),
        }
    }
}

/// The stroke's tessellated geometry, kept across frames.
///
/// `canvas` calls `Program::draw` from its own `draw`, so without this every
/// frame re-walks the rounded rectangle and re-tessellates the dashed stroke
/// for geometry that only changes when its parameters do. `canvas::Cache`
/// keys itself on the layer's bounds, so it already covers a resize; `drawn`
/// covers everything else the closure reads, which is the whole of
/// [`DashedBorder`]. The comparison is the derived `PartialEq`, so a field
/// added later cannot quietly escape it.
#[doc(hidden)]
#[derive(Default)]
pub struct DashedBorderCache {
    geometry: canvas::Cache,
    drawn: RefCell<Option<DashedBorder>>,
}

impl<Message> canvas::Program<Message> for DashedBorder {
    type State = DashedBorderCache;

    fn draw(
        &self,
        state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        if self.width <= 0.0 || self.segments.is_empty() {
            return Vec::new();
        }
        // A spurious mismatch costs one redraw; a missed one draws a stale
        // border. So this compares every parameter, floats included, and
        // never tries to be clever about which of them "matter".
        let stale = state.drawn.borrow().as_ref() != Some(self);
        if stale {
            state.geometry.clear();
            *state.drawn.borrow_mut() = Some(self.clone());
        }
        let inset = self.width / 2.0;
        vec![state.geometry.draw(renderer, bounds.size(), |frame| {
            let path = canvas::Path::rounded_rectangle(
                Point::new(inset, inset),
                Size::new(
                    (bounds.width - self.width).max(0.0),
                    (bounds.height - self.width).max(0.0),
                ),
                inner_radius(self.radius, inset),
            );
            frame.stroke(
                &path,
                canvas::Stroke {
                    style: canvas::Style::Solid(self.color),
                    width: self.width,
                    line_dash: canvas::LineDash {
                        segments: &self.segments,
                        offset: 0,
                    },
                    ..canvas::Stroke::default()
                },
            );
        })]
    }
}

/// Makes the pattern valid for both canvas renderers. WGPU repeats odd-length
/// patterns itself, while tiny-skia rejects them; all-zero patterns are
/// invalid in tiny-skia and can make WGPU's path walker stop advancing.
fn normalize_segments(mut segments: Vec<f32>) -> Vec<f32> {
    for segment in &mut segments {
        if !segment.is_finite() || *segment < 0.0 {
            *segment = 0.0;
        }
    }
    if segments.iter().all(|segment| *segment == 0.0) {
        return Vec::new();
    }
    if segments.len() % 2 == 1 {
        segments.extend_from_within(..);
    }
    segments
}

/// Keeps the stroke concentric with the surface it traces: an inset path needs
/// its corners tightened by the same inset, exactly like a CSS inner border.
fn inner_radius(radius: Radius, inset: f32) -> Radius {
    Radius {
        top_left: (radius.top_left - inset).max(0.0),
        top_right: (radius.top_right - inset).max(0.0),
        bottom_right: (radius.bottom_right - inset).max(0.0),
        bottom_left: (radius.bottom_left - inset).max(0.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tightens_corners_by_the_inset_and_never_goes_negative() {
        let radius = inner_radius(
            Radius {
                top_left: 8.0,
                top_right: 8.0,
                bottom_right: 1.0,
                bottom_left: 0.0,
            },
            2.0,
        );
        assert_eq!(radius.top_left, 6.0);
        assert_eq!(radius.bottom_right, 0.0);
        assert_eq!(radius.bottom_left, 0.0);
    }

    #[test]
    fn normalizes_patterns_for_both_canvas_renderers() {
        assert_eq!(
            normalize_segments(vec![4.0, 3.0, 2.0]),
            vec![4.0, 3.0, 2.0, 4.0, 3.0, 2.0]
        );
        assert_eq!(normalize_segments(vec![0.0, 0.0]), Vec::<f32>::new());
        assert_eq!(
            normalize_segments(vec![f32::NAN, -1.0, 2.0]),
            vec![0.0, 0.0, 2.0, 0.0, 0.0, 2.0]
        );
    }
}
