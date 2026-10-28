//! The background behind the 3D view, and a glass theme's start page: a gradient, a solid colour,
//! a built-in pattern, or the user's own picture (Preferences › Interface › Colour and glass).
//! Patterns are drawn in code from the background's two colours, so they follow the tab colour and
//! the strength settings like the gradient does; a new pattern is one row of [`PATTERNS`] and a
//! painter. The settings belong to the UI thread, set each frame by `prefs::apply`.
//!
//! In the 3D view a pattern or picture sits under a soft veil of the background's colour,
//! strongest in the middle where the model is worked on, so it never competes with the model.

use std::cell::RefCell;

use egui::{Color32, Mesh, Painter, Pos2, Rect, Stroke, TextureHandle, pos2, vec2};
use serde::{Deserialize, Serialize};

/// What the background is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// The background's two colours, top to bottom.
    #[default]
    Gradient,
    /// The top colour alone.
    Solid,
    /// A built-in pattern ([`PATTERNS`]) in the background's colours.
    Pattern,
    /// The user's own picture, filling the view.
    Image,
}

impl Kind {
    pub const ALL: [Kind; 4] = [Kind::Gradient, Kind::Solid, Kind::Pattern, Kind::Image];

    /// Its name in the Preferences (a message id).
    pub fn label_id(self) -> &'static str {
        match self {
            Kind::Gradient => "prefs.background_gradient",
            Kind::Solid => "prefs.background_solid",
            Kind::Pattern => "prefs.background_pattern",
            Kind::Image => "prefs.background_image",
        }
    }
}

/// The built-in patterns: their id (the `background_pattern` preference) and name (a message id).
pub const PATTERNS: [(&str, &str); 1] = [("aurora", "prefs.pattern_aurora")];

/// Hostile-input caps for the user's picture: file bytes, and the texture's longest side.
const MAX_IMAGE_BYTES: usize = 64 * 1024 * 1024;
const MAX_SIDE: u32 = 2048;
/// How much of the background's colour veils a pattern or picture in the middle of the 3D view.
const VIEW_VEIL: f32 = 0.55;

/// The background settings in effect.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Settings {
    pub kind: Kind,
    pub pattern: String,
    pub image: String,
}

thread_local! {
    static SETTINGS: RefCell<Settings> = RefCell::new(Settings::default());
    /// The user's picture as a texture, by the path it came from (`None`: it didn't read).
    static PICTURE: RefCell<Option<(String, Option<TextureHandle>)>> = const { RefCell::new(None) };
}

/// Use these background settings (`prefs::apply`, every frame).
pub fn set_current(settings: Settings) {
    SETTINGS.with(|s| {
        if let Ok(mut s) = s.try_borrow_mut()
            && *s != settings
        {
            *s = settings;
        }
    });
}

fn current() -> Settings {
    SETTINGS.with(|s| s.try_borrow().map(|s| s.clone()).unwrap_or_default())
}

/// Where the background is drawn, for how calm it keeps the middle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    /// The 3D view: a pattern or picture is veiled where the model is worked on.
    View,
    /// A full page (the start page), which washes it itself.
    Page,
}

/// Paint the background into `rect`, from its `top` and `bottom` colours.
pub fn paint(painter: &Painter, rect: Rect, top: Color32, bottom: Color32, place: Place) {
    if !rect.is_finite() || rect.width() < 1.0 || rect.height() < 1.0 {
        return;
    }
    let settings = current();
    let drawn = match settings.kind {
        Kind::Gradient => false,
        Kind::Solid => {
            painter.rect_filled(rect, 0.0, top);
            return;
        }
        Kind::Pattern => {
            gradient(painter, rect, top, bottom);
            if let Some(pattern) = pattern(&settings.pattern) {
                pattern(painter, rect, top, bottom);
            }
            true
        }
        Kind::Image => picture(painter, rect, &settings.image),
    };
    if !drawn {
        gradient(painter, rect, top, bottom);
        return;
    }
    if place == Place::View {
        veil(painter, rect, top.lerp_to_gamma(bottom, 0.5));
    }
}

/// The painter of a built-in pattern ([`PATTERNS`]), by its id.
fn pattern(id: &str) -> Option<fn(&Painter, Rect, Color32, Color32)> {
    match id {
        "aurora" => Some(aurora),
        _ => None,
    }
}

fn gradient(painter: &Painter, rect: Rect, top: Color32, bottom: Color32) {
    let mut mesh = Mesh::default();
    for (corner, color) in [(rect.left_top(), top), (rect.right_top(), top), (rect.right_bottom(), bottom), (rect.left_bottom(), bottom)] {
        mesh.colored_vertex(corner, color);
    }
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(mesh);
}

/// A soft wash of `color`, strongest in the middle of `rect` and gone at its edges.
fn veil(painter: &Painter, rect: Rect, color: Color32) {
    let center = rect.center();
    let radii = rect.size() * 0.62;
    fan(painter, center, color.gamma_multiply(VIEW_VEIL), &ellipse(center, radii, 48), Color32::TRANSPARENT);
}

/// Aurora's layers and their colours, made from the background's two: a deep wash across the
/// top, light rays and fine strands in near-white tints of the background, a soft glow along the
/// ribbons, a white glow where they gather, and a warm, spring-green glow low on the left.
struct Layers {
    wash: Color32,
    ray: Color32,
    strand: Color32,
    ribbon: Color32,
    glow: Color32,
    warm: Color32,
}

impl Layers {
    fn of(top: Color32, bottom: Color32) -> Self {
        Layers {
            wash: top.lerp_to_gamma(Color32::BLACK, 0.22),
            ray: top.lerp_to_gamma(Color32::WHITE, 0.85),
            strand: bottom.lerp_to_gamma(Color32::WHITE, 0.75),
            ribbon: bottom.lerp_to_gamma(Color32::WHITE, 0.45),
            glow: Color32::WHITE,
            warm: bottom.lerp_to_gamma(Color32::from_rgb(0xe8, 0xff, 0x8c), 0.45),
        }
    }
}

/// "Aurora": soft rays falling from the top, and three ribbons of fine strands sweeping up from
/// the lower left across the view in long curves, glowing where they gather, all faint, in the
/// background's own colours ([`Layers`]).
fn aurora(painter: &Painter, rect: Rect, top: Color32, bottom: Color32) {
    let (w, h) = (rect.width(), rect.height());
    // The layers drift with the light (`theme::light`), the nearer ones more: rays, then the
    // glow, then the ribbons.
    let light = crate::theme::light();
    let shift = |depth: f32| vec2(light.x * w * 0.03 * depth, light.y * h * 0.02 * depth);
    let at = |x: f32, y: f32| pos2(rect.left() + w * x, rect.top() + h * y);
    let layers = Layers::of(top, bottom);
    // A deeper wash across the upper right, so the light reads against it.
    fan(painter, at(0.8, 0.0), layers.wash.gamma_multiply(0.35), &ellipse(at(0.8, 0.0), vec2(w * 0.85, h * 0.7), 48), Color32::TRANSPARENT);
    // Rays: (where across the top, how wide, how far they lean, how bright)
    for (x, width, lean, strength) in
        [(0.05, 0.08, 0.1, 0.1), (0.15, 0.03, 0.08, 0.12), (0.28, 0.1, 0.06, 0.07), (0.4, 0.04, 0.05, 0.09), (0.58, 0.08, 0.02, 0.06)]
    {
        soft_band(painter, rect, at(x, 0.0).x + shift(1.0).x, w * width, w * lean, h * 0.8, layers.ray.gamma_multiply(strength));
    }
    // Glows: warm low on the left, white where the ribbons gather, and a halo round it.
    let warm = at(0.12, 0.92);
    fan(painter, warm, layers.warm.gamma_multiply(0.28), &ellipse(warm, vec2(w * 0.42, h * 0.26), 48), Color32::TRANSPARENT);
    // Where the ribbons gather leans towards the light.
    let gather = at(0.6, 0.86) + shift(1.6);
    fan(painter, gather, layers.ribbon.gamma_multiply(0.25), &ellipse(gather, vec2(w * 0.55, h * 0.2), 48), Color32::TRANSPARENT);
    fan(painter, gather, layers.glow.gamma_multiply(0.3), &ellipse(gather, vec2(w * 0.3, h * 0.07), 48), Color32::TRANSPARENT);
    // Ribbons: (the curve's four points, how far its strands spread, how many strands)
    let ribbons: [([f32; 8], f32, usize); 3] = [
        ([-0.1, 0.98, 0.25, 0.62, 0.6, 0.95, 1.1, 0.52], 0.1, 14),
        ([-0.1, 0.72, 0.35, 0.55, 0.65, 0.88, 1.1, 0.7], 0.08, 12),
        ([-0.1, 0.88, 0.4, 0.98, 0.75, 0.6, 1.1, 0.85], 0.06, 10),
    ];
    for (points, spread, strands) in ribbons {
        let curve = |offset: f32| {
            let p = |i: usize| {
                let (x, y) = (points.get(i).copied().unwrap_or(0.0), points.get(i + 1).copied().unwrap_or(0.0));
                // The strands fan out as the ribbon sweeps right.
                at(x, y + offset * (0.3 + 0.7 * x.clamp(0.0, 1.0))) + shift(0.6)
            };
            [p(0), p(2), p(4), p(6)]
        };
        let line = |offset: f32, width: f32, color: Color32| {
            painter.add(egui::epaint::CubicBezierShape::from_points_stroke(curve(offset), false, Color32::TRANSPARENT, Stroke::new(width, color)));
        };
        line(0.0, h * 0.12, layers.ribbon.gamma_multiply(0.04));
        line(0.0, h * 0.04, layers.ribbon.gamma_multiply(0.06));
        let last = strands.saturating_sub(1).max(1) as f32;
        for i in 0..strands {
            let along = i as f32 / last - 0.5;
            line(spread * along, 1.2, layers.strand.gamma_multiply(0.05 + 0.09 * (1.0 - along.abs() * 2.0)));
        }
    }
}

/// A soft, slanted band of light from the top of `rect`, fading in across its first third, out
/// across its last, and out towards its `length` down.
fn soft_band(painter: &Painter, rect: Rect, x: f32, width: f32, lean: f32, length: f32, color: Color32) {
    let top = rect.top();
    let bottom = top + length;
    let edge = 0.35;
    for (from, to, a, b) in [(0.0, edge, 0.0, 1.0), (edge, 1.0 - edge, 1.0, 1.0), (1.0 - edge, 1.0, 1.0, 0.0)] {
        let mut mesh = Mesh::default();
        let at = |across: f32, down: f32| pos2(x + width * across + lean * down, top + (bottom - top) * down);
        mesh.colored_vertex(at(from, 0.0), color.gamma_multiply(a));
        mesh.colored_vertex(at(to, 0.0), color.gamma_multiply(b));
        mesh.colored_vertex(at(to, 1.0), Color32::TRANSPARENT);
        mesh.colored_vertex(at(from, 1.0), Color32::TRANSPARENT);
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(0, 2, 3);
        painter.add(mesh);
    }
}

fn ellipse(center: Pos2, radii: egui::Vec2, steps: usize) -> Vec<Pos2> {
    (0..steps)
        .map(|i| {
            let a = std::f32::consts::TAU * i as f32 / steps as f32;
            pos2(center.x + radii.x * a.cos(), center.y + radii.y * a.sin())
        })
        .collect()
}

/// A convex outline filled from `hub` (in `inner`) to its edge (in `outer`).
fn fan(painter: &Painter, hub: Pos2, inner: Color32, outline: &[Pos2], outer: Color32) {
    let Ok(count) = u32::try_from(outline.len()) else { return };
    if count < 3 {
        return;
    }
    let mut mesh = Mesh::default();
    mesh.colored_vertex(hub, inner);
    for p in outline {
        mesh.colored_vertex(*p, outer);
    }
    for i in 0..count {
        mesh.add_triangle(0, i + 1, (i + 1) % count + 1);
    }
    painter.add(mesh);
}

/// The user's picture, scaled to fill `rect` and cropped to it. Returns whether it drew; a picture
/// that doesn't read (missing, too big, not PNG or JPEG) draws nothing, and the gradient shows.
fn picture(painter: &Painter, rect: Rect, path: &str) -> bool {
    if path.trim().is_empty() {
        return false;
    }
    let texture = PICTURE.with(|held| {
        let Ok(mut held) = held.try_borrow_mut() else { return None };
        if held.as_ref().is_none_or(|(p, _)| p != path) {
            *held = Some((path.to_string(), load(painter.ctx(), path)));
        }
        held.as_ref().and_then(|(_, texture)| texture.clone())
    });
    let Some(texture) = texture else { return false };
    let size = texture.size_vec2();
    if size.x <= 0.0 || size.y <= 0.0 {
        return false;
    }
    // Cover: the picture fills the rectangle, its middle kept, the rest cropped.
    let scale = (rect.width() / size.x).max(rect.height() / size.y);
    let shown = rect.size() / (size * scale);
    let uv = Rect::from_center_size(pos2(0.5, 0.5), shown);
    painter.image(texture.id(), rect, uv, Color32::WHITE);
    true
}

/// The user's picture as a texture: PNG or JPEG, at most [`MAX_IMAGE_BYTES`], scaled to at most
/// [`MAX_SIDE`] a side. `None`: it doesn't read.
fn load(ctx: &egui::Context, path: &str) -> Option<TextureHandle> {
    let bytes = solidcraft_engine::io::vfs::read(path, MAX_IMAGE_BYTES).map_err(|e| log::warn!("background picture {path}: {e}")).ok()?;
    let picture = match image::guess_format(&bytes).ok()? {
        image::ImageFormat::Png | image::ImageFormat::Jpeg => {
            image::load_from_memory(&bytes).map_err(|e| log::warn!("background picture {path}: {e}")).ok()?
        }
        _ => {
            log::warn!("background picture {path}: not a PNG or JPEG picture");
            return None;
        }
    };
    let picture = if picture.width() > MAX_SIDE || picture.height() > MAX_SIDE { picture.thumbnail(MAX_SIDE, MAX_SIDE) } else { picture };
    let rgba = picture.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let image = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
    Some(ctx.load_texture("sc_background_picture", image, egui::TextureOptions::LINEAR))
}
