//! Shapes of a frame, by layer and clip rectangle, for epaint to tessellate.

use ecolor::Color32;
use emath::{Pos2, Rect, Vec2};
use epaint::{ClippedShape, CornerRadius, FontId, Galley, Shape, Stroke, StrokeKind, TextureId};
use std::sync::Arc;

/// Collects the frame's shapes. Each node paints into the layer it lies in, clipped to what
/// of it can be seen (a scrolled list's rows end at the list).
pub struct Painter {
    layers: Vec<Vec<ClippedShape>>,
    layer: usize,
    clip: Rect,
}

impl Painter {
    pub(crate) fn new(layers: usize) -> Painter {
        Painter { layers: (0..layers).map(|_| Vec::new()).collect(), layer: 0, clip: Rect::EVERYTHING }
    }

    pub(crate) fn set(&mut self, layer: usize, clip: Rect) {
        self.layer = layer.min(self.layers.len() - 1);
        self.clip = clip;
    }

    pub(crate) fn finish(self) -> Vec<ClippedShape> {
        self.layers.into_iter().flatten().collect()
    }

    /// The clip of the node being painted.
    pub fn clip(&self) -> Rect {
        self.clip
    }

    /// Paint within `clip` too (intersected with the node's own).
    pub fn with_clip(&mut self, clip: Rect, f: impl FnOnce(&mut Painter)) {
        let old = self.clip;
        self.clip = old.intersect(clip);
        f(self);
        self.clip = old;
    }

    pub fn add(&mut self, shape: impl Into<Shape>) {
        let shape = shape.into();
        if self.clip.is_positive() {
            self.layers[self.layer].push(ClippedShape { clip_rect: self.clip, shape });
        }
    }

    pub fn rect_filled(&mut self, rect: Rect, radius: f32, color: Color32) {
        if color.a() > 0 {
            self.add(Shape::rect_filled(rect, CornerRadius::same(radius.round().clamp(0.0, 255.0) as u8), color));
        }
    }

    pub fn rect_stroke(&mut self, rect: Rect, radius: f32, stroke: Stroke) {
        if stroke.width > 0.0 && stroke.color.a() > 0 {
            self.add(Shape::rect_stroke(rect, CornerRadius::same(radius.round().clamp(0.0, 255.0) as u8), stroke, StrokeKind::Inside));
        }
    }

    pub fn circle_filled(&mut self, center: Pos2, radius: f32, color: Color32) {
        self.add(Shape::circle_filled(center, radius, color));
    }

    pub fn circle_stroke(&mut self, center: Pos2, radius: f32, stroke: Stroke) {
        self.add(Shape::circle_stroke(center, radius, stroke));
    }

    pub fn line(&mut self, a: Pos2, b: Pos2, stroke: Stroke) {
        self.add(Shape::line_segment([a, b], stroke));
    }

    /// Text laid out already (see [`layout_text`]) with its top left at `pos`.
    pub fn galley(&mut self, pos: Pos2, galley: Arc<Galley>, fallback: Color32) {
        self.add(Shape::galley(pos, galley, fallback));
    }

    /// `texture` (an application's, see [`crate::Ui::texture_id`]) across `rect`.
    pub fn image(&mut self, texture: TextureId, rect: Rect, uv: Rect, tint: Color32, radius: f32) {
        if radius <= 0.0 {
            self.add(Shape::image(texture, rect, uv, tint));
        } else {
            let brush = epaint::Brush { fill_texture_id: texture, uv };
            let mut s = epaint::RectShape::filled(rect, CornerRadius::same(radius.round().clamp(0.0, 255.0) as u8), tint);
            s.brush = Some(Arc::new(brush));
            self.add(Shape::Rect(s));
        }
    }

    /// An arc of a circle (a spinner), from `a0` to `a1` radians.
    pub fn arc(&mut self, center: Pos2, radius: f32, a0: f32, a1: f32, stroke: Stroke) {
        let n = ((a1 - a0).abs() * radius / 3.0).clamp(6.0, 64.0) as usize;
        let pts: Vec<Pos2> = (0..=n).map(|i| {
            let a = a0 + (a1 - a0) * i as f32 / n as f32;
            center + Vec2::new(a.cos(), a.sin()) * radius
        }).collect();
        self.add(Shape::line(pts, stroke));
    }
}

/// Text in `font` and `color`, wrapped at `wrap` points (None: one line).
/// (`ppp`: physical pixels per point, which the glyphs are rasterised for.)
pub fn layout_text(fonts: &mut epaint::Fonts, ppp: f32, text: &str, font: FontId, color: Color32, wrap: Option<f32>) -> Arc<Galley> {
    match wrap {
        Some(w) => fonts.with_pixels_per_point(ppp).layout(text.to_owned(), font, color, w.max(1.0)),
        None => fonts.with_pixels_per_point(ppp).layout_no_wrap(text.to_owned(), font, color),
    }
}

/// Text on one line no wider than `width`: what does not fit ends in "…".
pub fn layout_text_elided(fonts: &mut epaint::Fonts, ppp: f32, text: &str, font: FontId, color: Color32, width: f32) -> Arc<Galley> {
    let mut job = epaint::text::LayoutJob::simple_singleline(text.to_owned(), font, color);
    job.wrap = epaint::text::TextWrapping { max_width: width.max(1.0), max_rows: 1, break_anywhere: true, overflow_character: Some('…') };
    fonts.with_pixels_per_point(ppp).layout_job(job)
}
