//! Scene geometry in canvas pixels; stored transforms remain percentages of the 16:9 frame.
use rinva_core::Sprite;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}
impl Rect {
    pub fn frame(w: f32, h: f32) -> Self {
        let width = w.min(h * 16. / 9.);
        let height = width * 9. / 16.;
        Self {
            x: (w - width) / 2.,
            y: (h - height) / 2.,
            w: width,
            h: height,
        }
    }
    pub fn contains(self, x: f32, y: f32) -> bool {
        x >= self.x && x <= self.x + self.w && y >= self.y && y <= self.y + self.h
    }
}
pub fn bounds(frame: Rect, sprite: &Sprite, aspect: f32) -> Rect {
    let h = frame.h * sprite.scale as f32 / 100.;
    let w = h * aspect;
    Rect {
        x: frame.x + frame.w * sprite.x as f32 / 100. - w / 2.,
        y: frame.y + frame.h * sprite.y as f32 / 100. - h,
        w,
        h,
    }
}
pub fn translate(origin: &Sprite, frame: Rect, dx: f32, dy: f32, snap: bool) -> Sprite {
    let mut next = origin.clone();
    if frame.w <= 0. || frame.h <= 0. {
        return next;
    }
    let quantize = |value: f32| {
        if snap {
            (value / 5.).round() * 5.
        } else {
            value.round()
        }
    };
    next.x = quantize(origin.x as f32 + dx / frame.w * 100.).clamp(-50., 150.) as i32;
    next.y = quantize(origin.y as f32 + dy / frame.h * 100.).clamp(-50., 150.) as i32;
    next
}
pub fn resize(origin: &Sprite, anchor: (f32, f32), start: (f32, f32), at: (f32, f32)) -> Sprite {
    let mut next = origin.clone();
    let v = (start.0 - anchor.0, start.1 - anchor.1);
    let denominator = v.0 * v.0 + v.1 * v.1;
    if denominator > 1. {
        let ratio = ((at.0 - anchor.0) * v.0 + (at.1 - anchor.1) * v.1) / denominator;
        next.scale = (origin.scale as f32 * ratio).round().clamp(10., 300.) as u32;
    }
    next
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn letterbox_mapping_and_dpi_are_consistent() {
        let s = Sprite::new("c".into(), "e".into());
        for dpi in [1., 1.25, 1.5, 2.] {
            let f = Rect::frame(800. * dpi, 600. * dpi);
            let b = bounds(f, &s, 0.5);
            assert!((b.y - f.y).abs() < 0.01);
            let n = translate(&s, f, f.w * 0.1, -f.h * 0.2, false);
            assert_eq!((n.x, n.y), (60, 80));
        }
    }
    #[test]
    fn drag_uses_origin_and_scaling_cannot_flip_or_collapse() {
        let s = Sprite::new("c".into(), "e".into());
        let f = Rect::frame(1600., 900.);
        assert_eq!(translate(&s, f, 47., 0., true).x, 55);
        assert_eq!(translate(&s, f, 16000., -16000., false).x, 150);
        let bigger = resize(&s, (800., 900.), (1000., 500.), (1100., 300.));
        assert_eq!(bigger.scale, 150);
        assert_eq!((bigger.x, bigger.y), (s.x, s.y));
        assert_eq!(resize(&s, (0., 0.), (1., 2.), (-10., -20.)).scale, 10);
    }
}
