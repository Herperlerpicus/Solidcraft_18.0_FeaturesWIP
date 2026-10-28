//! Colours and widget style: a dark theme (the default, a blue-grey in the spirit of desktop CAD
//! tools, viewport #2e3440) and a light one, each with colour vision modes, and theme files that
//! set any of their colours (`theme_files`). Every colour the UI uses comes from [`Tokens`].

use std::cell::Cell;

#[path = "../../../assets/styles/ui/palettes.rs"]
mod palettes;

use egui::{Color32, Stroke};
use serde::{Deserialize, Serialize};

thread_local! {
    /// Theme state belongs to the UI thread drawing the frame. Concurrent app/test renderers
    /// must not change one another's palette while a frame or icon texture is being built.
    static DARK: Cell<bool> = const { Cell::new(true) };
    /// The colour vision mode, per thread. Each app
    /// sets it from its own preferences at the start of every frame (`SolveApp::logic`), on the
    /// thread that draws it. Tests run many apps side by side on their own threads in one process:
    /// with one global, a test in a mode changed the colours under another one
    /// (`hover_stays_off_during_spin_and_animated_views` failed that way). No worker thread reads
    /// it: the preview thread is handed its colours (`preview::Colors`) and thumbnails are drawn by
    /// the engine in the model's colours, so a new thread starts Off and stays Off.
    static VISION: Cell<ColorVision> = const { Cell::new(ColorVision::Off) };
    /// The palette of a theme file (`theme_files`) in place of the built-in one of its base
    /// (dark or light), per thread like the rest. `None`: the built-in palettes.
    static CUSTOM: Cell<Option<Custom>> = const { Cell::new(None) };
    /// The application bar's logo in the theme's accent instead of its own logo colours
    /// (Preferences › Interface › Logo), per thread like the rest.
    static LOGO_ACCENT: Cell<bool> = const { Cell::new(false) };
    /// The 3D view's own dark or light when Preferences give it one ([`ViewportTheme`], #4146),
    /// per thread like the rest. `None`: the view is as dark as the window.
    static VIEWPORT_DARK: Cell<Option<bool>> = const { Cell::new(None) };
}

/// Three structural border levels plus independent content and control outlines.
/// Focus, selection, status, artwork and CAD geometry do not use these roles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BorderRole {
    Primary,
    Secondary,
    Tertiary,
    Container,
    Control,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Borders {
    pub primary: Option<Stroke>,
    pub secondary: Option<Stroke>,
    /// Pattern for secondary section rules; solid preserves existing themes.
    pub secondary_dotted: bool,
    pub tertiary: Option<Stroke>,
    /// Solid outlines for nested previews, information wells and shortcut keycaps.
    pub container: Option<Stroke>,
    pub control: Option<Stroke>,
    pub primary_inset: Option<InsetBorder>,
    /// Optional directional chrome. Absent themes keep their flat strokes.
    pub bevel: Option<Bevel>,
    /// Optional translucent chrome over the 3D view. Absent themes keep opaque surfaces.
    pub glass: Option<Glass>,
}

/// Glass surfaces: what floats over the 3D view (floating panels, windows, popups, menus,
/// toasts, navigation) lets it show through, with a sheen fading down from the top edge.
/// Docked panels and the application bar stay opaque.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glass {
    /// How much of a surface's own fill covers the view behind it (0.3 to 1).
    pub opacity: f32,
    /// The sheen's colour at the top edge; it fades out over `sheen_height` points.
    pub sheen: Color32,
    pub sheen_height: f32,
    /// Command, navigation and title bar buttons sit on glossy bubbles of this shape
    /// (`paint_bubble`) in place of flat hover and pressed fills. `None`: flat fills.
    pub bubbles: Option<BubbleShape>,
    /// How much menus, popups, tooltips and notifications cover what is behind them (0.3 to 1),
    /// usually more than panels so their text stays readable over busy ones. `None`: `opacity`.
    pub menu_opacity: Option<f32>,
    /// How deep a shadow menus, popups, tooltips and notifications cast (0 to 1: softness,
    /// offset and darkness together). `None` or 0: none, as glass surfaces otherwise have.
    pub menu_shadow: Option<f32>,
}

/// The shape of a glass theme's bubbles.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BubbleShape {
    /// Orbs.
    #[default]
    Round,
    /// Tiles with rounded corners.
    Square,
}

impl BubbleShape {
    pub fn id(self) -> &'static str {
        match self {
            BubbleShape::Round => "round",
            BubbleShape::Square => "square",
        }
    }
}

/// What a bubble shows of its button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bubble {
    Rest,
    Hover,
    /// Pressed, or on (a running command, a selected tool).
    On,
    Disabled,
}

/// A bubble painted behind a button's icon (`Tokens::bubble`); `finish` adds its gloss over the
/// icon once that is drawn.
#[derive(Clone, Copy, Debug)]
pub struct Bubbled {
    glass: Glass,
    center: egui::Pos2,
    radius: f32,
    state: Bubble,
}

impl Bubbled {
    pub fn finish(self, painter: &egui::Painter) {
        self.glass.paint_bubble_gloss(painter, self.center, self.radius, self.state);
    }
}

impl Bubble {
    /// A button's bubble state: disabled, on (pressed, or `on`: running, selected, open), under
    /// the pointer or focused, or at rest.
    pub fn of(response: &egui::Response, enabled: bool, on: bool) -> Self {
        if !enabled {
            Bubble::Disabled
        } else if on || response.is_pointer_button_down_on() {
            Bubble::On
        } else if response.hovered() || response.has_focus() {
            Bubble::Hover
        } else {
            Bubble::Rest
        }
    }

    /// The body's tint, the rim's and the white shine of a bubble in this state.
    fn strengths(self) -> (f32, f32, f32) {
        match self {
            Bubble::Rest => (0.16, 0.45, 0.7),
            Bubble::Hover => (0.32, 0.75, 0.9),
            Bubble::On => (0.6, 1.0, 0.8),
            Bubble::Disabled => (0.08, 0.25, 0.35),
        }
    }
}

impl Glass {
    /// A glossy bubble of `radius` behind an icon at `center`, tinted with `tint` (the tab's
    /// accent): a translucent body, a glow along its bottom and a rim. `paint_bubble_gloss`
    /// then covers the icon with the bubble's highlight. Paint only: no layout or input target.
    /// Too small or non-finite: nothing.
    pub fn paint_bubble(self, painter: &egui::Painter, center: egui::Pos2, radius: f32, tint: Color32, state: Bubble) {
        if !center.is_finite() || !radius.is_finite() || radius < 4.0 {
            return;
        }
        let (body, rim, shine) = state.strengths();
        // The glow: brightest at the bottom edge, gone above the middle.
        let glow = |p: egui::Pos2| Color32::WHITE.gamma_multiply(0.45 * shine * ((p.y - center.y) / radius).clamp(0.0, 1.0));
        let rim = Stroke::new(1.0, tint.gamma_multiply(rim));
        let inner = Stroke::new(1.0, Color32::WHITE.gamma_multiply(0.5 * shine));
        match self.bubbles.unwrap_or_default() {
            BubbleShape::Round => {
                painter.circle_filled(center, radius, tint.gamma_multiply(body));
                fan(painter, center, Color32::TRANSPARENT, ellipse(center, egui::vec2(radius, radius)), glow);
                painter.circle_stroke(center, radius - 0.5, rim);
                painter.circle_stroke(center, radius - 1.5, inner);
            }
            BubbleShape::Square => {
                let (tile, corner) = tile(center, radius);
                self.paint_tile(painter, tile, egui::CornerRadius::same(corner), tint, state);
            }
        }
    }

    /// A glass tile filling `rect` with its own `corner`s (a button's whole face, or one segment
    /// of a joined group): a translucent body in `tint`, a glow along its bottom, a gloss over
    /// its top half that fades out at the middle, and a rim. All of it goes behind the icon and
    /// label the caller draws next, so the gloss never covers them. Too small: nothing.
    pub fn paint_tile(self, painter: &egui::Painter, rect: egui::Rect, corner: egui::CornerRadius, tint: Color32, state: Bubble) {
        if !rect.is_finite() || rect.width() < 6.0 || rect.height() < 6.0 {
            return;
        }
        let (body, rim, shine) = state.strengths();
        let center = rect.center();
        painter.rect_filled(rect, corner, tint.gamma_multiply(body));
        let half = rect.height() * 0.5;
        let glow = |p: egui::Pos2| Color32::WHITE.gamma_multiply(0.45 * shine * ((p.y - center.y) / half).clamp(0.0, 1.0));
        fan(painter, center, Color32::TRANSPARENT, outline(rect, corner), glow);
        // The gloss: the top half inside the rim, brightest at the top and gone at the middle.
        let face = rect.shrink(1.5);
        let inner_corner = shrunk(corner, 1);
        let fade = |y: f32| Color32::WHITE.gamma_multiply(0.6 * shine * (1.0 - (y - face.top()) / (center.y - face.top())).clamp(0.0, 1.0));
        let top = outline(face, inner_corner).into_iter().map(|p| egui::pos2(p.x, p.y.min(center.y))).collect();
        fan(painter, center, fade(center.y), top, |p| fade(p.y));
        self.paint_streaks(painter, rect.shrink(1.0));
        painter.rect_stroke(rect, corner, Stroke::new(1.0, tint.gamma_multiply(rim)), egui::StrokeKind::Inside);
        painter.rect_stroke(rect.shrink(1.0), inner_corner, Stroke::new(1.0, Color32::WHITE.gamma_multiply(0.5 * shine)), egui::StrokeKind::Inside);
    }

    /// An orb's highlight, painted over its icon like a glass dome: an ellipse across the top
    /// half, fading downwards, light enough that the icon reads through it. Tiles have theirs
    /// under the icon already. The icon's own artwork is unchanged.
    pub fn paint_bubble_gloss(self, painter: &egui::Painter, center: egui::Pos2, radius: f32, state: Bubble) {
        if !center.is_finite() || !radius.is_finite() || radius < 4.0 {
            return;
        }
        let shine = 0.6 * state.strengths().2;
        match self.bubbles.unwrap_or_default() {
            BubbleShape::Round => {
                let top = egui::pos2(center.x, center.y - radius * 0.42);
                let size = egui::vec2(radius * 0.72, radius * 0.48);
                let fade = |p: egui::Pos2| Color32::WHITE.gamma_multiply(shine * ((top.y + size.y * 0.7 - p.y) / (size.y * 1.7)).clamp(0.0, 1.0));
                fan(painter, top, fade(top), ellipse(top, size), fade);
            }
            // A tile's gloss went under its icon with the tile (`paint_tile`).
            BubbleShape::Square => {}
        }
    }
    /// A glass surface's fill. Its drop shadow is left out, since it would show through.
    pub fn fill(self, fill: Color32) -> Color32 {
        fill.gamma_multiply(self.opacity.clamp(0.0, 1.0))
    }

    /// The drop shadow of a menu, popup, tooltip or notification (`menu_shadow`).
    pub fn menu_shadow(self) -> egui::Shadow {
        let depth = self.menu_shadow.filter(|v| v.is_finite()).unwrap_or(0.0).clamp(0.0, 1.0);
        if depth <= 0.0 {
            return egui::Shadow::NONE;
        }
        egui::Shadow {
            offset: [0, (2.0 + 6.0 * depth).round() as i8],
            blur: (6.0 + 22.0 * depth).round() as u8,
            spread: 0,
            color: Color32::from_black_alpha((90.0 * depth).round() as u8),
        }
    }

    /// A menu's, popup's, tooltip's or notification's fill (`menu_opacity`).
    pub fn menu_fill(self, fill: Color32) -> Color32 {
        fill.gamma_multiply(self.menu_opacity.unwrap_or(self.opacity).clamp(0.0, 1.0))
    }

    /// Paint the sheen inside `rect` and its rounded corners; no layout or input target.
    pub fn paint_sheen(self, painter: &egui::Painter, rect: egui::Rect, radius: impl Into<egui::CornerRadius>) {
        if let Some(mesh) = self.sheen_mesh(rect, radius) {
            painter.add(mesh);
        }
    }

    /// The sheen of `rect` as a shape, for a host that reserved its place under its contents
    /// (`Container::begin_content`). Too small or non-finite: `None`.
    pub fn sheen_mesh(self, rect: egui::Rect, radius: impl Into<egui::CornerRadius>) -> Option<egui::Mesh> {
        let height = self.sheen_height.min(rect.height() * 0.5);
        if !rect.is_finite() || !height.is_finite() || height < 2.0 || rect.width() < 4.0 || self.sheen.a() == 0 {
            return None;
        }
        let radius: egui::CornerRadius = radius.into();
        let rounding: egui::epaint::CornerRadiusF32 = radius.into();
        let mut path = Vec::new();
        egui::epaint::tessellator::path::rounded_rectangle(&mut path, rect, rounding);
        let count = u32::try_from(path.len()).ok()?;
        // A fan from the point where the sheen has faded out; the outline is convex.
        let mut mesh = egui::Mesh::default();
        mesh.colored_vertex(egui::pos2(rect.center().x, rect.top() + height), Color32::TRANSPARENT);
        for point in &path {
            mesh.colored_vertex(*point, self.sheen.gamma_multiply((1.0 - (point.y - rect.top()) / height).clamp(0.0, 1.0)));
        }
        for i in 0..count {
            mesh.add_triangle(0, i + 1, (i + 1) % count + 1);
        }
        Some(mesh)
    }

    /// Diagonal streaks of light across a glass bar (the title bar), as light catches a pane:
    /// a few slanted bands in the sheen's colour, brightest at the top and fading downwards,
    /// clipped to `rect`. Paint before the bar's contents. The Gloss setting dims them with the
    /// sheen.
    pub fn paint_streaks(self, painter: &egui::Painter, rect: egui::Rect) {
        glass_area(painter, rect);
        if let Some(streaks) = self.streaks_mesh(painter.ctx().content_rect(), rect) {
            painter.add(streaks);
        }
    }

    /// The streaks of light `rect` shows: one set of bands across the whole window (`screen`), as
    /// if one light falls on all its glass, so they line up across every glass surface; each
    /// surface shows the part it covers, cut to its rectangle, brightest along its top. White
    /// light as strong as the sheen (the Gloss setting). `None`: nothing to show.
    pub fn streaks_mesh(self, screen: egui::Rect, rect: egui::Rect) -> Option<egui::Mesh> {
        if !rect.is_finite() || !screen.is_finite() || rect.width() < 4.0 || rect.height() < 4.0 || self.sheen.a() == 0 {
            return None;
        }
        let light = Color32::WHITE.gamma_multiply(f32::from(self.sheen.a()) / 255.0);
        let mut mesh = egui::Mesh::default();
        // (where across the window the band starts at its top, how wide, how bright)
        for (at, width, strength) in [(0.18, 84.0, 0.5), (0.235, 30.0, 0.42), (0.52, 120.0, 0.36), (0.6, 40.0, 0.45)] {
            // The band leans: its left edge moves right as it goes down the window. It is soft:
            // its light rises across its first third, holds, and fades across its last third,
            // each part a strip of its own so the light changes evenly across it.
            // The bands drift with the light, as if it moved.
            let drift = self::light().x * STREAK_DRIFT;
            let left = |y: f32| screen.left() + screen.width() * at + drift + (y - screen.top()) * STREAK_SLANT;
            let profile = |p: egui::Pos2| {
                let across = ((p.x - left(p.y)) / width).clamp(0.0, 1.0);
                (across / STREAK_EDGE).min((1.0 - across) / STREAK_EDGE).min(1.0)
            };
            for (from, to) in [(0.0, STREAK_EDGE), (STREAK_EDGE, 1.0 - STREAK_EDGE), (1.0 - STREAK_EDGE, 1.0)] {
                let strip = [
                    egui::pos2(left(rect.top()) + width * from, rect.top()),
                    egui::pos2(left(rect.top()) + width * to, rect.top()),
                    egui::pos2(left(rect.bottom()) + width * to, rect.bottom()),
                    egui::pos2(left(rect.bottom()) + width * from, rect.bottom()),
                ];
                let piece = clip_x(&clip_x(&strip, rect.left(), true), rect.right(), false);
                let Ok(first) = u32::try_from(mesh.vertices.len()) else { return None };
                let Ok(count) = u32::try_from(piece.len()) else { continue };
                if count < 3 {
                    continue;
                }
                for p in &piece {
                    let down = ((p.y - rect.top()) / rect.height()).clamp(0.0, 1.0);
                    mesh.colored_vertex(*p, light.gamma_multiply(strength * profile(*p) * (1.0 - 0.45 * down)));
                }
                for i in 1..count - 1 {
                    mesh.add_triangle(first, first + i, first + i + 1);
                }
            }
        }
        (!mesh.is_empty()).then_some(mesh)
    }

    /// An embossed line in `color` filling `rect` (a tab's colour line): the colour, a light
    /// highlight along its top and a darker edge along its bottom, as if pressed into the glass.
    pub fn paint_emboss(self, painter: &egui::Painter, rect: egui::Rect, color: Color32) {
        if !rect.is_finite() || rect.width() < 2.0 || rect.height() < 1.0 {
            return;
        }
        let corner = (rect.height() * 0.5).round() as u8;
        painter.rect_filled(rect, corner, color);
        let [r, g, b, a] = color.to_srgba_unmultiplied();
        let deeper = |c: u8| (f32::from(c) * 0.6).round() as u8;
        painter.hline(
            rect.x_range().shrink(1.0),
            rect.bottom() - 0.5,
            Stroke::new(1.0, Color32::from_rgba_unmultiplied(deeper(r), deeper(g), deeper(b), a)),
        );
        painter.hline(rect.x_range().shrink(1.0), rect.top() + 0.5, Stroke::new(1.0, Color32::WHITE.gamma_multiply(0.7)));
    }

    /// A glass theme's highlighted row (`Tokens::paint_highlight`): the fill, lit from the top
    /// by a white gradient that fades out towards the bottom, inside a slightly deeper border.
    pub fn paint_highlight(self, painter: &egui::Painter, rect: egui::Rect, radius: egui::CornerRadius, fill: Color32) {
        painter.rect_filled(rect, radius, fill);
        if !rect.is_finite() || rect.height() < 4.0 || rect.width() < 4.0 {
            return;
        }
        let mut path = Vec::new();
        egui::epaint::tessellator::path::rounded_rectangle(&mut path, rect, radius.into());
        let light = |y: f32| Color32::WHITE.gamma_multiply(0.5 * (1.0 - ((y - rect.top()) / rect.height()).clamp(0.0, 1.0)));
        fan(painter, rect.center(), light(rect.center().y), path, |p| light(p.y));
        let [r, g, b, a] = fill.to_srgba_unmultiplied();
        let deeper = |c: u8| (f32::from(c) * 0.8).round() as u8;
        painter.rect_stroke(
            rect,
            radius,
            Stroke::new(1.0, Color32::from_rgba_unmultiplied(deeper(r), deeper(g), deeper(b), a)),
            egui::StrokeKind::Inside,
        );
    }

    /// The glossy face of a glass theme's buttons (`Tokens::button_bevel`): the top half
    /// brighter, fading towards a soft split across the middle, inside the button's corners.
    /// Dimmer while pressed or on, fainter when disabled. Paint only; too small: nothing.
    pub fn paint_gloss(self, painter: &egui::Painter, rect: egui::Rect, radius: impl Into<egui::CornerRadius>, pressed: bool, enabled: bool) {
        if !rect.is_finite() || rect.height() < 8.0 || rect.width() < 8.0 || self.sheen.a() == 0 {
            return;
        }
        let strength = match (enabled, pressed) {
            (false, _) => 0.25,
            (true, true) => 0.35,
            (true, false) => 0.75,
        };
        let radius: egui::CornerRadius = radius.into();
        let mut path = Vec::new();
        egui::epaint::tessellator::path::rounded_rectangle(&mut path, rect, radius.into());
        let split = rect.center().y;
        // The outline's lower half folds up onto the split: the top half's rounded outline.
        let face = |y: f32| self.sheen.gamma_multiply(strength * (0.3 + 0.7 * ((split - y) / (split - rect.top())).clamp(0.0, 1.0)));
        let outline = path.into_iter().map(|p| egui::pos2(p.x, p.y.min(split))).collect();
        fan(painter, egui::pos2(rect.center().x, split), face(split), outline, |p| face(p.y));
    }
}

/// How far the light streaks lean: across per down.
const STREAK_SLANT: f32 = 0.9;
/// The share of a streak's width over which each edge fades in or out.
const STREAK_EDGE: f32 = 0.35;
/// How far the light streaks drift as the light moves to the window's edge.
const STREAK_DRIFT: f32 = 48.0;

/// A convex polygon cut to one side of a vertical line (`keep_right`: keep `x >= at`).
fn clip_x(points: &[egui::Pos2], at: f32, keep_right: bool) -> Vec<egui::Pos2> {
    let inside = |p: egui::Pos2| if keep_right { p.x >= at } else { p.x <= at };
    let mut out = Vec::with_capacity(points.len() + 2);
    for (i, p) in points.iter().enumerate() {
        let Some(q) = points.get((i + 1) % points.len()) else { continue };
        if inside(*p) {
            out.push(*p);
        }
        if inside(*p) != inside(*q) && (q.x - p.x).abs() > f32::EPSILON {
            let t = (at - p.x) / (q.x - p.x);
            out.push(egui::pos2(at, p.y + (q.y - p.y) * t));
        }
    }
    out
}

/// A square bubble's tile and corner radius for a bubble of `radius` at `center`.
fn tile(center: egui::Pos2, radius: f32) -> (egui::Rect, u8) {
    let corner = (radius * 0.3).round().clamp(2.0, 12.0) as u8;
    (egui::Rect::from_center_size(center, egui::vec2(radius * 2.0, radius * 2.0)), corner)
}

/// Points round a rounded rectangle, for the tiles' gradients.
fn outline(rect: egui::Rect, corner: egui::CornerRadius) -> Vec<egui::Pos2> {
    let mut path = Vec::new();
    egui::epaint::tessellator::path::rounded_rectangle(&mut path, rect, corner.into());
    path
}

/// Corners `by` points tighter, for an outline inside another.
fn shrunk(corner: egui::CornerRadius, by: u8) -> egui::CornerRadius {
    egui::CornerRadius {
        nw: corner.nw.saturating_sub(by),
        ne: corner.ne.saturating_sub(by),
        sw: corner.sw.saturating_sub(by),
        se: corner.se.saturating_sub(by),
    }
}

/// Points round an ellipse, for the bubbles' gradients.
fn ellipse(center: egui::Pos2, radii: egui::Vec2) -> Vec<egui::Pos2> {
    const STEPS: usize = 40;
    (0..STEPS)
        .map(|i| {
            let a = std::f32::consts::TAU * i as f32 / STEPS as f32;
            egui::pos2(center.x + radii.x * a.cos(), center.y + radii.y * a.sin())
        })
        .collect()
}

/// A convex outline filled as a fan from `hub`, each outline point coloured by `color`.
fn fan(painter: &egui::Painter, hub: egui::Pos2, hub_color: Color32, outline: Vec<egui::Pos2>, color: impl Fn(egui::Pos2) -> Color32) {
    let Ok(count) = u32::try_from(outline.len()) else {
        return;
    };
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(hub, hub_color);
    for point in outline {
        mesh.colored_vertex(point, color(point));
    }
    for i in 0..count {
        mesh.add_triangle(0, i + 1, (i + 1) % count + 1);
    }
    painter.add(mesh);
}

/// Two directional edge bands, shared by raised chrome and recessed editors.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bevel {
    pub width: f32,
    pub highlight: Color32,
    pub light: Color32,
    pub shadow: Color32,
    pub dark_shadow: Color32,
}

impl Bevel {
    /// Draw inside existing geometry; no extra input targets or layout allocation.
    pub fn paint(self, painter: &egui::Painter, rect: egui::Rect, radius: impl Into<egui::CornerRadius>, recessed: bool, enabled: bool) {
        if !rect.is_finite() || self.width <= 0.0 || !self.width.is_finite() || rect.width().min(rect.height()) <= self.width * 4.0 {
            return;
        }
        let radius = radius.into();
        let bands = if recessed {
            [(self.shadow, self.highlight), (self.dark_shadow, self.light)]
        } else {
            [(self.highlight, self.dark_shadow), (self.light, self.shadow)]
        };
        for (index, (upper, lower)) in bands.into_iter().enumerate() {
            let inset = (index as f32 + 0.5) * self.width;
            let edge = rect.shrink(inset);
            let mut rounding: egui::epaint::CornerRadiusF32 = radius.into();
            rounding -= inset;
            let mut path = Vec::new();
            egui::epaint::tessellator::path::rounded_rectangle(&mut path, edge, rounding);
            if let Some(first) = path.first().copied() {
                path.push(first);
            }
            for pair in path.windows(2) {
                if let Some((a, b)) = pair.first().zip(pair.get(1)) {
                    let middle = *a + (*b - *a) * 0.5;
                    let relative = (middle - edge.center()) / edge.size();
                    let color = if relative.x + relative.y < 0.0 { upper } else { lower };
                    let color = if enabled { color } else { color.gamma_multiply(0.45) };
                    painter.line_segment([*a, *b], Stroke::new(self.width, color));
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InsetBorder {
    pub stroke: Stroke,
    /// Clear distance from the inner edge of the primary stroke, in logical points.
    pub gap: f32,
}

impl Borders {
    pub const LEGACY: Self = Self {
        primary: None,
        secondary: None,
        secondary_dotted: false,
        tertiary: None,
        container: None,
        control: None,
        primary_inset: None,
        bevel: None,
        glass: None,
    };

    pub fn stroke(self, role: BorderRole, fallback: Stroke) -> Stroke {
        match role {
            BorderRole::Primary => self.primary,
            BorderRole::Secondary => self.secondary,
            BorderRole::Tertiary => self.tertiary,
            BorderRole::Container => self.container,
            BorderRole::Control => self.control,
        }
        .unwrap_or(fallback)
    }
}

impl Tokens {
    /// Bevel themes use the editable surface colour independently of button faces.
    pub fn input_fill(&self) -> Color32 {
        if self.borders.bevel.is_some() { self.field } else { self.control_fill }
    }

    pub fn input_bevel(&self, ui: &egui::Ui, response: &egui::Response) {
        if let Some(bevel) = self.borders.bevel {
            bevel.paint(ui.painter(), response.rect.shrink(1.0), self.control_radius, true, response.enabled());
        }
    }

    /// A button's bubble in a theme with bubbles, behind the icon and label the caller draws
    /// next; then `Bubbled::finish`. A tile fills the whole of `rect` (icon and label), with the
    /// theme's control corners; an orb sits inside it. `None`: the theme has no bubbles, and the
    /// button keeps its own fill.
    pub fn bubble(&self, painter: &egui::Painter, rect: egui::Rect, tint: Color32, state: Bubble) -> Option<Bubbled> {
        let glass = self.borders.glass.filter(|glass| glass.bubbles.is_some())?;
        let bubbled = Bubbled { glass, center: rect.center(), radius: rect.width().min(rect.height()) * 0.5, state };
        match glass.bubbles {
            Some(BubbleShape::Square) => glass.paint_tile(painter, rect, egui::CornerRadius::same(self.control_radius), tint, state),
            _ => glass.paint_bubble(painter, bubbled.center, bubbled.radius, tint, state),
        }
        Some(bubbled)
    }

    /// One segment of a joined group of buttons (the window's caption buttons) in a theme with
    /// square bubbles: a tile with the `corner`s the group gives it, so the segments meet in one
    /// strip. Returns whether it painted; otherwise the caller uses `bubble` or its own fill.
    pub fn tile_segment(&self, painter: &egui::Painter, rect: egui::Rect, corner: egui::CornerRadius, tint: Color32, state: Bubble) -> bool {
        match self.borders.glass.filter(|glass| glass.bubbles == Some(BubbleShape::Square)) {
            Some(glass) => {
                glass.paint_tile(painter, rect, corner, tint, state);
                true
            }
            None => false,
        }
    }

    /// A highlighted row or item (under the pointer, selected, open): a flat `fill`, or in a glass
    /// theme a soft top-lit gradient of it with a slightly deeper border (`Glass::paint_highlight`).
    /// Hover and selection fills paint through this so a theme can restyle all of them at once.
    pub fn paint_highlight(&self, painter: &egui::Painter, rect: egui::Rect, radius: impl Into<egui::CornerRadius>, fill: Color32) {
        let radius = radius.into();
        match self.borders.glass {
            Some(glass) => glass.paint_highlight(painter, rect, radius, fill),
            None => {
                painter.rect_filled(rect, radius, fill);
            }
        }
    }

    pub fn button_bevel(&self, ui: &egui::Ui, response: &egui::Response, selected: bool) {
        self.button_bevel_with_radius(ui, response, selected, self.control_radius);
    }

    /// Compound controls supply the same corner geometry as their selected fill.
    pub fn button_bevel_with_radius(&self, ui: &egui::Ui, response: &egui::Response, selected: bool, radius: impl Into<egui::CornerRadius>) {
        let pressed = || {
            response.enabled()
                && (response.is_pointer_button_down_on()
                    || (response.has_focus() && ui.input(|i| i.key_down(egui::Key::Space) || i.key_down(egui::Key::Enter))))
        };
        let radius: egui::CornerRadius = radius.into();
        if let Some(bevel) = self.borders.bevel {
            bevel.paint(ui.painter(), response.rect, radius, selected || pressed(), response.enabled());
        }
        // A glass theme's buttons are glossy (`Glass::paint_gloss`).
        if let Some(glass) = self.borders.glass {
            glass.paint_gloss(ui.painter(), response.rect.shrink(1.0), radius, selected || pressed(), response.enabled());
        }
    }

    pub fn primary_border(&self, color: Color32) -> Stroke {
        self.borders.stroke(BorderRole::Primary, Stroke::new(1.0, color))
    }
    pub fn secondary_border(&self, color: Color32) -> Stroke {
        self.borders.stroke(BorderRole::Secondary, Stroke::new(1.0, color))
    }
    /// Secondary rules share a pattern as well as a stroke. Dot count is bounded for tiny widths.
    fn secondary_dots(&self, painter: &egui::Painter, path: &[egui::Pos2], stroke: Stroke) {
        if stroke.width <= 0.0 || path.iter().any(|p| !p.is_finite()) {
            return;
        }
        let length: f32 = path.windows(2).map(|pair| pair.first().zip(pair.get(1)).map_or(0.0, |(a, b)| a.distance(*b))).sum();
        if !length.is_finite() {
            return;
        }
        let spacing = (stroke.width * 3.0).max(3.0).max(length / 4096.0);
        painter.extend(egui::Shape::dotted_line(path, stroke.color, spacing, stroke.width * 0.5));
    }

    pub fn secondary_line(&self, painter: &egui::Painter, points: [egui::Pos2; 2], color: Color32) {
        let stroke = self.secondary_border(color);
        if self.borders.secondary_dotted {
            self.secondary_dots(painter, &points, stroke);
        } else {
            painter.line_segment(points, stroke);
        }
    }

    pub fn secondary_hline(&self, painter: &egui::Painter, range: impl Into<egui::emath::Rangef>, y: f32, color: Color32) {
        let range = range.into();
        self.secondary_line(painter, [egui::pos2(range.min, y), egui::pos2(range.max, y)], color);
    }

    pub fn secondary_vline(&self, painter: &egui::Painter, x: f32, range: impl Into<egui::emath::Rangef>, color: Color32) {
        let range = range.into();
        self.secondary_line(painter, [egui::pos2(x, range.min), egui::pos2(x, range.max)], color);
    }

    /// Enclosures stay solid independently of the section-divider pattern.
    pub fn container_rect(&self, painter: &egui::Painter, rect: egui::Rect, radius: impl Into<egui::CornerRadius>, fill: Color32, color: Color32) {
        let radius = radius.into();
        painter.rect(rect, radius, fill, self.container_border(color), egui::StrokeKind::Inside);
        if let Some(bevel) = self.borders.bevel {
            bevel.paint(painter, rect, radius, false, true);
        }
    }

    pub fn container_border(&self, color: Color32) -> Stroke {
        self.borders.stroke(BorderRole::Container, Stroke::new(1.0, color))
    }

    pub fn tertiary_border(&self, color: Color32) -> Stroke {
        self.borders.stroke(BorderRole::Tertiary, Stroke::new(1.0, color))
    }
    pub fn control_border_stroke(&self, color: Color32) -> Stroke {
        self.borders.stroke(BorderRole::Control, Stroke::new(1.0, color))
    }
}

/// A theme file's palette and whether it is dark.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Custom {
    pub dark: bool,
    pub tokens: Tokens,
}

/// Use a theme file's palette for its base, or the built-in palettes (`None`). Returns whether
/// that changed anything (the visuals follow on the next [`apply`]).
pub fn set_custom(custom: Option<Custom>) -> bool {
    CUSTOM.with(|c| c.replace(custom) != custom)
}

/// Draw the logo in the theme's accent (`true`) or its own logo colours. Returns whether that
/// changed anything (the visuals follow on the next [`apply`]).
pub fn set_logo_accent(on: bool) -> bool {
    LOGO_ACCENT.with(|c| c.replace(on) != on)
}

/// The user's own adjustments over a theme (Preferences › Interface › Colour and glass): a base
/// colour and how strongly it tints, and a glass theme's opacity and gloss. `None`: the theme's.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Personal {
    pub color: Option<Color32>,
    pub intensity: f32,
    pub opacity: Option<f32>,
    pub gloss: Option<f32>,
    pub menu_opacity: Option<f32>,
    pub menu_shadow: Option<f32>,
    /// The 3D view's background gradient, top and bottom, in place of the theme's.
    pub sky: Option<[Color32; 2]>,
}

impl Personal {
    /// Nothing over the theme's own.
    pub fn is_empty(&self) -> bool {
        self.color.is_none()
            && self.opacity.is_none()
            && self.gloss.is_none()
            && self.menu_opacity.is_none()
            && self.menu_shadow.is_none()
            && self.sky.is_none()
    }
}

/// The surfaces the base colour tints, with the ink read on each and how much of the tint it
/// takes at full intensity: the title bar most, panels and dialogs gently.
macro_rules! tinted_surfaces {
    ($t:ident, $tint:expr, $k:expr, [$($fill:ident on $ink:ident * $share:expr),* $(,)?]) => {
        $( $t.$fill = readable_mix($t.$fill, $tint, $k * $share, $t.$ink); )*
    };
}

impl Tokens {
    /// These tokens with the user's own base colour and glass over them. Text stays readable:
    /// a surface takes less of the tint where its text would read worse than 4.5:1 (or than it
    /// did, if the theme's own was lower).
    pub fn personalised(mut self, p: Personal) -> Tokens {
        if let Some(tint) = p.color {
            let k = if p.intensity.is_finite() { p.intensity.clamp(0.0, 1.0) } else { 0.0 };
            tinted_surfaces!(self, tint, k, [
                app_bar on app_bar_text * 0.85,
                app_bar_tab_active on app_bar_text * 0.3,
                toolbar on text * 0.3,
                panel on text * 0.3,
                panel_header on text * 0.35,
                timeline on text * 0.3,
                dialog_bg on dialog_label * 0.3,
                dialog_body on dialog_label * 0.25,
                dialog_footer on dialog_label * 0.3,
                overlay on text * 0.3,
            ]);
        }
        if let Some([top, bottom]) = p.sky {
            (self.viewport_top, self.viewport_bottom) = (top, bottom);
            // The window's title bar follows the background's top, its text kept readable: a
            // colour too strong for it is lightened or darkened first, so it still shows.
            self.app_bar = readable_mix(self.app_bar, readable_shade(top, self.app_bar_text), 0.75, self.app_bar_text);
        }
        if let Some(glass) = self.borders.glass.as_mut() {
            if let Some(opacity) = p.opacity.filter(|v| v.is_finite()) {
                glass.opacity = opacity.clamp(0.3, 1.0);
            }
            if let Some(menu) = p.menu_opacity.filter(|v| v.is_finite()) {
                glass.menu_opacity = Some(menu.clamp(0.3, 1.0));
            }
            if let Some(shadow) = p.menu_shadow.filter(|v| v.is_finite()) {
                glass.menu_shadow = Some(shadow.clamp(0.0, 1.0));
            }
            if let Some(gloss) = p.gloss.filter(|v| v.is_finite()) {
                let [r, g, b, _] = glass.sheen.to_srgba_unmultiplied();
                glass.sheen = Color32::from_rgba_unmultiplied(r, g, b, (gloss.clamp(0.0, 1.0) * 255.0).round() as u8);
            }
        }
        self
    }
}

/// `base` mixed towards `tint` by `amount`, keeping `base`'s alpha, and backed off until `ink`
/// reads on it at least as well as 4.5:1 (or as on `base`, if that was lower).
fn readable_mix(base: Color32, tint: Color32, amount: f32, ink: Color32) -> Color32 {
    let [r, g, b, a] = base.to_srgba_unmultiplied();
    let opaque = Color32::from_rgb(r, g, b);
    let floor = contrast_ratio(ink, opaque).min(4.5) - 1e-6;
    let mut amount = amount.clamp(0.0, 1.0);
    for _ in 0..16 {
        let mixed = opaque.lerp_to_gamma(tint, amount);
        if contrast_ratio(ink, mixed) >= floor {
            return Color32::from_rgba_unmultiplied(mixed.r(), mixed.g(), mixed.b(), a);
        }
        amount *= 0.75;
    }
    base
}

/// `color`, its hue kept, lightened (behind dark `ink`) or darkened (behind light `ink`) only as
/// far as `ink` needs to read on it at 4.5:1.
fn readable_shade(color: Color32, ink: Color32) -> Color32 {
    let opaque = Color32::from_rgb(color.r(), color.g(), color.b());
    if contrast_ratio(ink, opaque) >= 4.5 {
        return opaque;
    }
    // The middle of the luminance range in contrast terms: darker ink reads on lighter colours.
    let toward = if luminance(Color32::from_rgb(ink.r(), ink.g(), ink.b())) < 0.18 { Color32::WHITE } else { Color32::BLACK };
    let (mut low, mut high) = (0.0_f32, 1.0_f32);
    for _ in 0..16 {
        let middle = (low + high) * 0.5;
        if contrast_ratio(ink, opaque.lerp_to_gamma(toward, middle)) >= 4.5 {
            high = middle;
        } else {
            low = middle;
        }
    }
    opaque.lerp_to_gamma(toward, high)
}

/// Isolated widget tests restore the thread's theme even when an assertion fails.
#[cfg(test)]
pub(crate) fn scoped_tokens(tokens: Tokens) -> impl Drop {
    struct Restore(Option<Custom>);
    impl Drop for Restore {
        fn drop(&mut self) {
            set_custom(self.0);
        }
    }
    let restore = Restore(CUSTOM.get());
    set_custom(Some(Custom { dark: is_dark(), tokens }));
    restore
}

/// A tab's selection colours. This is deliberately separate from the global widget style:
/// application chrome, checkboxes, status colours and dialog actions retain their own tokens.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TabAccent {
    pub color: Color32,
    pub ink: Color32,
    pub soft: Color32,
}

/// Every ribbon tab that can be given a colour of its own (Preferences), as the ribbon names it:
/// its pill, its line, its soft selections and, with icons following their tab, its icons.
/// The application's own accent (title bar, workspace buttons) never changes with it.
pub const COLOURED_TABS: &[(&str, &str)] = &[
    ("MODEL", "Model"),
    ("SURFACE", "Surface"),
    ("ASSEMBLE", "Assemble"),
    ("INSERT", "Insert"),
    ("INSPECT", "Inspect"),
    ("MANAGE", "Manage"),
    ("VIEW", "View"),
    ("SHEET METAL", "Sheet Metal"),
    ("WELDMENTS", "Weldments"),
    ("SKETCH", "Sketch"),
    ("MESH", "Mesh"),
    ("FORM", "Form"),
    ("RENDER", "Render"),
];

/// The id a tab's own colour is kept under (the Render workspace's tabs share one).
fn tab_key(tab: &str) -> &str {
    if tab.starts_with("RENDER") { "RENDER" } else { tab }
}

thread_local! {
    /// The window's rectangle this frame, which the light streaks span (`Tokens::screen`).
    static SCREEN: Cell<egui::Rect> = const { Cell::new(egui::Rect::ZERO) };
    /// Where the light comes from: the pointer's place from the window's middle, -1 to 1 each
    /// way, smoothed (`set_light`); zero when it doesn't follow the pointer.
    static LIGHT: Cell<egui::Vec2> = const { Cell::new(egui::Vec2::ZERO) };
    /// The user's own tab colours (`prefs::apply`), per thread like the theme. Only tab drawing
    /// reads them (`TabAccent::for_tab`).
    static TAB_COLORS: Cell<Option<std::rc::Rc<std::collections::BTreeMap<String, Color32>>>> = const { Cell::new(None) };
}

impl Tokens {
    /// The window's rectangle this frame (`set_screen`), which the light streaks span.
    pub fn screen() -> egui::Rect {
        SCREEN.get()
    }
}

/// Keep the window's rectangle for the light streaks (`prefs::apply`, every frame).
pub fn set_screen(rect: egui::Rect) {
    SCREEN.set(rect);
}

/// Where the light comes from now (`prefs::apply`, every frame): -1 to 1 each way from the middle.
pub fn set_light(offset: egui::Vec2) {
    LIGHT.set(if offset.is_finite() { offset.clamp(egui::Vec2::splat(-1.0), egui::Vec2::splat(1.0)) } else { egui::Vec2::ZERO });
}

/// Where the light comes from (`set_light`): the light streaks and patterns shift with it.
pub fn light() -> egui::Vec2 {
    LIGHT.get()
}

/// Use the user's own tab colours (empty: the theme's). Returns whether that changed anything.
pub fn set_tab_colors(colors: std::collections::BTreeMap<String, Color32>) -> bool {
    let held = TAB_COLORS.take();
    let same = held.as_deref().map_or(colors.is_empty(), |old| *old == colors);
    TAB_COLORS.set(if same { held } else { (!colors.is_empty()).then(|| std::rc::Rc::new(colors)) });
    !same
}

/// The user's own colour for a tab, if any.
fn user_tab_color(tab: &str) -> Option<Color32> {
    let held = TAB_COLORS.take();
    let color = held.as_ref().and_then(|colors| colors.get(tab_key(tab)).copied());
    TAB_COLORS.set(held);
    color
}

/// Palette ramp for icons on a surface explicitly accented by its active tab.
pub(crate) fn icon_accent_for_tab(tab: &str) -> Option<&'static str> {
    match tab {
        "SKETCH" => Some("sketch"),
        "SURFACE" => Some("surface"),
        "MESH" => Some("mesh"),
        "FORM" => Some("form"),
        _ => None,
    }
}

/// A colour on its way to `target` over `seconds`, channel by channel, from where it was last
/// asked for under `id` (the first time, `target` itself).
pub fn shifting(ctx: &egui::Context, id: &str, target: Color32, seconds: f32) -> Color32 {
    let channel = |i: u8, v: u8| ctx.animate_value_with_time(egui::Id::new((id, i)), f32::from(v), seconds).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgb(channel(0, target.r()), channel(1, target.g()), channel(2, target.b()))
}

thread_local! {
    /// The glass painted this frame, by layer (`glass_area`): the text on it gets a soft shadow
    /// at the end of the frame (`shadow_text_on_glass`).
    static GLASS_AREAS: std::cell::RefCell<Vec<(egui::LayerId, egui::Rect)>> = const { std::cell::RefCell::new(Vec::new()) };
    /// How strong the halo and shadow behind text on glass are (`set_text_shadow`), 0 to 1.
    static TEXT_SHADOW_STRENGTH: Cell<f32> = const { Cell::new(0.5) };
}

/// How strong the halo and shadow behind text on glass are (`prefs::apply`, every frame): 0 for
/// none, 0.5 a soft glow, 1 a strong one.
pub fn set_text_shadow(strength: f32) {
    TEXT_SHADOW_STRENGTH.set(if strength.is_finite() { strength.clamp(0.0, 1.0) } else { 0.5 });
}

/// At most this many glass areas are kept in a frame; more are drawn without text shadows.
const MAX_GLASS_AREAS: usize = 4096;

/// Note a glass surface painted this frame, as far as its painter shows it, so the text drawn on
/// it gets a soft shadow (`shadow_text_on_glass`).
pub fn glass_area(painter: &egui::Painter, rect: egui::Rect) {
    let rect = rect.intersect(painter.clip_rect());
    if rect.is_finite() && rect.is_positive() {
        GLASS_AREAS.with_borrow_mut(|areas| {
            if areas.len() < MAX_GLASS_AREAS {
                areas.push((painter.layer_id(), rect));
            }
        });
    }
}

/// After a frame is drawn: the text on its glass (`glass_area`) gets a soft halo and a drop
/// shadow behind it, so it stands out from the streaks and whatever shows through the glass. Text
/// lighter than the glass gets a dark one, darker text a light glow. Text anywhere else, and every
/// theme without glass, is left as it is. Forgets the frame's glass.
pub fn shadow_text_on_glass(ctx: &egui::Context) {
    let areas = GLASS_AREAS.take();
    let t = Tokens::get();
    let strength = TEXT_SHADOW_STRENGTH.get();
    if areas.is_empty() || t.borders.glass.is_none() || strength <= 0.0 {
        return;
    }
    let surface = luminance(Color32::from_rgb(t.panel.r(), t.panel.g(), t.panel.b()));
    let mut layers: Vec<egui::LayerId> = Vec::new();
    for (layer, _) in &areas {
        if !layers.contains(layer) {
            layers.push(*layer);
        }
    }
    ctx.graphics_mut(|graphics| {
        for layer in layers {
            let glass: Vec<egui::Rect> = areas.iter().filter(|(l, _)| *l == layer).map(|(_, rect)| *rect).collect();
            let Some(list) = graphics.get_mut(layer) else { continue };
            let count = list.all_entries().len();
            for i in 0..count {
                list.mutate_shape(egui::layers::ShapeIdx(i), |clipped| shadow_text(&mut clipped.shape, &glass, surface, strength));
            }
        }
    });
}

/// The soft glow behind text on glass, as rings of faint copies around a point a pixel below the
/// text (the light falls from above): (how far out, how many copies, how strong each is at half
/// strength, the default). Overlapping, they build up near the letters and fade outwards like a
/// blur; at full strength each is twice as strong (`shadow_text_on_glass`).
const TEXT_SHADOW: [(f32, usize, f32); 4] = [(0.0, 1, 0.2), (1.0, 8, 0.09), (2.0, 12, 0.05), (3.0, 16, 0.025)];

/// Where the glow's copies go and how strong each is, at half strength (`TEXT_SHADOW`).
fn text_shadow_copies() -> impl Iterator<Item = (egui::Vec2, f32)> {
    TEXT_SHADOW.into_iter().flat_map(|(radius, count, alpha)| {
        (0..count).map(move |i| {
            let angle = std::f32::consts::TAU * i as f32 / count as f32;
            (egui::vec2(radius * angle.cos(), 1.0 + radius * angle.sin()), alpha)
        })
    })
}

/// `shape`'s text that lies on `glass`, with its halo and shadow behind it, dark or light against
/// `surface` (the glass's luminance) as the text is, as strong as `strength` (0 to 1)
/// (`shadow_text_on_glass`).
fn shadow_text(shape: &mut egui::Shape, glass: &[egui::Rect], surface: f64, strength: f32) {
    match shape {
        egui::Shape::Vec(shapes) => shapes.iter_mut().for_each(|s| shadow_text(s, glass, surface, strength)),
        egui::Shape::Text(text) => {
            let center = text.visual_bounding_rect().center();
            if !glass.iter().any(|rect| rect.contains(center)) {
                return;
            }
            let ink = text
                .override_text_color
                .or_else(|| text.galley.job.sections.first().map(|s| s.format.color))
                .map(|c| if c == Color32::PLACEHOLDER { text.fallback_color } else { c })
                .unwrap_or(text.fallback_color);
            if ink.a() == 0 {
                return;
            }
            // Twice the table's strength at full strength; fainter text, a fainter shadow.
            let scale = 2.0 * strength * f32::from(ink.a()) / 255.0;
            let behind = if luminance(Color32::from_rgb(ink.r(), ink.g(), ink.b())) > surface { Color32::BLACK } else { Color32::WHITE };
            let mut shapes: Vec<egui::Shape> = text_shadow_copies()
                .map(|(offset, alpha)| {
                    let mut copy = text.clone();
                    copy.pos += offset;
                    copy.override_text_color = Some(behind.gamma_multiply((alpha * scale).min(1.0)));
                    copy.underline = Stroke::NONE;
                    egui::Shape::Text(copy)
                })
                .collect();
            shapes.push(std::mem::replace(shape, egui::Shape::Noop));
            *shape = egui::Shape::Vec(shapes);
        }
        _ => {}
    }
}

/// The sky's colour through a day, round the clock: the hour each colour is at its fullest, from
/// midnight's indigo through dawn's rose, the morning's and midday's blues, the late afternoon's
/// gold and dusk's coral to twilight's purple. Between two the colour shifts gradually.
const DAYLIGHT: [(f32, [u8; 3]); 8] = [
    (0.0, [38, 46, 104]),
    (5.0, [86, 70, 140]),
    (6.5, [240, 150, 120]),
    (9.0, [120, 190, 240]),
    (13.0, [70, 160, 230]),
    (17.5, [245, 180, 90]),
    (19.5, [220, 100, 120]),
    (21.0, [100, 70, 150]),
];

/// The sky's colour at `hours` past midnight (`DAYLIGHT`), for colours that follow the time of
/// day. Any hour is taken round the clock; one that isn't a number is midday.
pub fn daylight(hours: f32) -> Color32 {
    let hours = if hours.is_finite() { hours.rem_euclid(24.0) } else { 12.0 };
    let rgb = |c: [u8; 3]| Color32::from_rgb(c[0], c[1], c[2]);
    let next = DAYLIGHT.iter().position(|(at, _)| *at > hours).unwrap_or(DAYLIGHT.len());
    let (from, from_rgb) = DAYLIGHT[(next + DAYLIGHT.len() - 1) % DAYLIGHT.len()];
    let (to, to_rgb) = DAYLIGHT.get(next).copied().unwrap_or((24.0, DAYLIGHT[0].1));
    let along = if to > from { (hours - from) / (to - from) } else { 0.0 };
    rgb(from_rgb).lerp_to_gamma(rgb(to_rgb), along.clamp(0.0, 1.0))
}

/// `color` with the lightness of `reference`, its hue kept: mixed towards white or black until
/// its relative luminance matches, so strong and pale colours come out equally strong.
pub fn with_lightness_of(color: Color32, reference: Color32) -> Color32 {
    let target = luminance(reference);
    let own = luminance(color);
    let toward = if own < target { Color32::WHITE } else { Color32::BLACK };
    let (mut low, mut high) = (0.0_f32, 1.0_f32);
    for _ in 0..16 {
        let middle = (low + high) * 0.5;
        if (luminance(color.lerp_to_gamma(toward, middle)) < target) == (own < target) {
            low = middle;
        } else {
            high = middle;
        }
    }
    color.lerp_to_gamma(toward, (low + high) * 0.5)
}

/// Relative luminance of an opaque sRGB colour (WCAG).
fn luminance(c: Color32) -> f64 {
    let linear = |v: u8| {
        let v = f64::from(v) / 255.0;
        if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * linear(c.r()) + 0.7152 * linear(c.g()) + 0.0722 * linear(c.b())
}

/// WCAG contrast of two opaque sRGB colours.
pub(crate) fn contrast_ratio(a: Color32, b: Color32) -> f64 {
    let luminance = |c: Color32| {
        let linear = solidcraft_engine::geom::srgb::decode8;
        0.2126 * linear(c.r()) + 0.7152 * linear(c.g()) + 0.0722 * linear(c.b())
    };
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

impl TabAccent {
    pub fn for_tab(t: &Tokens, tab: &str) -> Self {
        let user = user_tab_color(tab).map(|color| {
            let [r, g, b, _] = t.panel.to_srgba_unmultiplied();
            (color, color, Color32::from_rgb(r, g, b).lerp_to_gamma(color, 0.2))
        });
        let (color, ink, soft) = if let Some(user) = user {
            user
        } else {
            match tab {
                "SKETCH" => (t.sketch_accent, t.sketch_ink, t.sketch_soft),
                "SURFACE" => (t.surface_fill, t.surface_ink, t.surface_soft),
                "MESH" => (t.mesh_fill, t.mesh_ink, t.mesh_soft),
                "FORM" => (t.form_fill, t.form_tab_ink, t.form_soft),
                "SHEET METAL" => (t.sheet_fill, t.sheet_ink, t.sheet_soft),
                "RENDER" | "RENDER APPEARANCE" | "RENDER SCENE" | "RENDER CAMERA" => (t.render_fill, t.render_ink, t.render_soft),
                // Model and shared utility tabs retain their existing selection exactly.
                _ => (t.accent, t.selected_ink, t.accent_soft),
            }
        };
        let ink = if contrast_ratio(ink, soft) >= 4.5 {
            ink
        } else if contrast_ratio(color, soft) >= 4.5 {
            color
        } else if contrast_ratio(t.text, soft) >= 4.5 {
            t.text
        } else if contrast_ratio(Color32::BLACK, soft) >= contrast_ratio(Color32::WHITE, soft) {
            Color32::BLACK
        } else {
            Color32::WHITE
        };
        Self { color, ink, soft }
    }

    /// Small text on a tab's solid pill. Brighter fills need dark ink in either theme.
    pub fn on_fill(self, t: &Tokens) -> Color32 {
        if contrast_ratio(t.on_accent, self.color) >= 4.5 {
            t.on_accent
        } else if contrast_ratio(t.tab_on_fill_dark, self.color) >= 4.5 {
            t.tab_on_fill_dark
        } else if contrast_ratio(Color32::BLACK, self.color) >= contrast_ratio(Color32::WHITE, self.color) {
            Color32::BLACK
        } else {
            Color32::WHITE
        }
    }

    pub fn current(ctx: &egui::Context) -> Self {
        ctx.data(|d| d.get_temp::<Self>(egui::Id::new("sc_tab_accent"))).unwrap_or_else(|| Self::for_tab(&Tokens::get(), "MODEL"))
    }

    pub fn current_icon_accent(ctx: &egui::Context) -> Option<&'static str> {
        ctx.data(|d| d.get_temp::<Option<&'static str>>(egui::Id::new("sc_tab_icon_accent"))).flatten()
    }

    /// With icons following their tab's colour (Preferences), the ramp the active tab's icons
    /// take (`None`: the base ramp) and the palette they are drawn with: the theme's, with that
    /// ramp made from the tab's colour where the tab has a colour of its own. `None`: icons keep
    /// their own colours. Only icons on the ribbon use this palette (`icons::paint_on_tab`).
    pub fn tab_icons(ctx: &egui::Context) -> Option<(Option<&'static str>, crate::icon_svg::Palette)> {
        ctx.data(|d| d.get_temp::<Option<(Option<&'static str>, crate::icon_svg::Palette)>>(egui::Id::new("sc_tab_icons"))).flatten()
    }

    /// The active tab's icon ramp and palette (`tab_icons`).
    fn tab_icon_palette(tab: &str) -> (Option<&'static str>, crate::icon_svg::Palette) {
        let t = Tokens::get();
        let own = icon_accent_for_tab(tab);
        let color = Self::for_tab(&t, tab).color;
        let mut palette = t.icon_palette.unwrap_or_default();
        // A colour of the user's, or a tab of the theme's own colour without a ramp for it
        // (Sheet Metal): its ramp is made from that colour, pale, medium and strong.
        let recolour = user_tab_color(tab).or_else(|| (own.is_none() && color != t.accent).then_some(color));
        if let Some(color) = recolour {
            let slot = own.and_then(crate::icon_svg::accent_index).map_or(0, |i| i + 1);
            let rgb = |c: Color32| [c.r(), c.g(), c.b()];
            if let Some(ramp) = palette.accents.get_mut(slot) {
                *ramp = Some([rgb(color.lerp_to_gamma(Color32::WHITE, 0.78)), rgb(color.lerp_to_gamma(Color32::WHITE, 0.3)), rgb(color)]);
            }
        }
        (own, palette)
    }

    pub fn publish(app: &crate::SolveApp, ctx: &egui::Context) {
        let accent = Self::for_tab(&Tokens::get(), &app.ui.tab);
        ctx.data_mut(|d| {
            d.insert_temp(egui::Id::new("sc_tab_accent"), accent);
            d.insert_temp(egui::Id::new("sc_tab_icon_accent"), icon_accent_for_tab(&app.ui.tab));
            d.insert_temp(egui::Id::new("sc_tab_icons"), app.preferences.tab_colored_icons.then(|| Self::tab_icon_palette(&app.ui.tab)));
            d.insert_temp(egui::Id::new("sc_sketch_open"), app.session.active_sketch.is_some());
            d.insert_temp(egui::Id::new("sc_tool_accent"), app.tool.is_some() || app.dialog.is_some());
        });
    }

    /// Paint only the focused numeric editor, without changing sibling buttons or checkboxes.
    pub fn number_focus(ui: &egui::Ui, response: &egui::Response) {
        let active = ui.ctx().data(|d| d.get_temp::<bool>(egui::Id::new("sc_tool_accent"))).unwrap_or(false);
        let t = Tokens::get();
        let sketch = ui.ctx().data(|d| d.get_temp::<bool>(egui::Id::new("sc_sketch_open"))).unwrap_or(false);
        let color = if sketch { t.sketch_accent } else { Self::current(ui.ctx()).color };
        if active && response.has_focus() && color != t.accent {
            ui.painter().rect_stroke(response.rect, t.radius_sm, Stroke::new(1.0, color), egui::StrokeKind::Inside);
        }
    }
}

/// Is the dark theme on?
pub fn is_dark() -> bool {
    DARK.get()
}

/// Is the 3D view dark? As the window, unless Preferences give the view its own (#4146).
pub fn viewport_dark() -> bool {
    VIEWPORT_DARK.get().unwrap_or_else(is_dark)
}

/// The common conversion boundary for compiled icon palette data.
pub(crate) const fn icon_color(rgb: [u8; 3]) -> Color32 {
    Color32::from_rgb(rgb[0], rgb[1], rgb[2])
}

/// Colour vision modes (#778): a second set of values for the colours that carry a meaning, for
/// a weak or missing red (protan), green (deutan) or blue (tritan) sense. Not a filter: Off gives
/// back the theme's colours unchanged. Protan and Deutan share one red-green table, measured
/// clear for both (`colour_vision_tests`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorVision {
    #[default]
    Off,
    Protan,
    Deutan,
    Tritan,
}

impl ColorVision {
    pub const ALL: [ColorVision; 4] = [ColorVision::Off, ColorVision::Protan, ColorVision::Deutan, ColorVision::Tritan];

    /// The saved value (`ui.prefs` `color_vision`).
    pub fn id(self) -> &'static str {
        match self {
            ColorVision::Off => "off",
            ColorVision::Protan => "protan",
            ColorVision::Deutan => "deutan",
            ColorVision::Tritan => "tritan",
        }
    }

    pub fn from_id(id: &str) -> Option<ColorVision> {
        ColorVision::ALL.into_iter().find(|m| m.id() == id)
    }

    /// Its name in the Preferences (a message id).
    pub fn label_id(self) -> &'static str {
        match self {
            ColorVision::Off => "ui.colour_vision_off",
            ColorVision::Protan => "ui.colour_vision_protan",
            ColorVision::Deutan => "ui.colour_vision_deutan",
            ColorVision::Tritan => "ui.colour_vision_tritan",
        }
    }

    /// Who it is for, in one line (its tooltip; a message id).
    pub fn help_id(self) -> &'static str {
        match self {
            ColorVision::Off => "ui.colour_vision_off_tip",
            ColorVision::Protan => "ui.colour_vision_protan_tip",
            ColorVision::Deutan => "ui.colour_vision_deutan_tip",
            ColorVision::Tritan => "ui.colour_vision_tritan_tip",
        }
    }
}

/// From saved preferences: a value this version doesn't know (a later version's mode, a hand
/// edit) is Off rather than an error, which would lose every other saved preference with it.
/// `ui.prefs` refuses such a value before it gets here (`prefs::set`).
impl<'de> Deserialize<'de> for ColorVision {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = serde_json::Value::deserialize(d)?;
        Ok(v.as_str().and_then(ColorVision::from_id).unwrap_or_default())
    }
}

/// The 3D view's colours (#4146): the theme's own, or the built-in Light or Dark theme's whatever
/// the window uses, so a dark window can show the model on a light view and the other way round.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewportTheme {
    #[default]
    Theme,
    Light,
    Dark,
}

impl ViewportTheme {
    pub const ALL: [ViewportTheme; 3] = [ViewportTheme::Theme, ViewportTheme::Light, ViewportTheme::Dark];

    /// The saved value (`ui.prefs` `viewport_theme`).
    pub fn id(self) -> &'static str {
        match self {
            ViewportTheme::Theme => "theme",
            ViewportTheme::Light => "light",
            ViewportTheme::Dark => "dark",
        }
    }

    pub fn from_id(id: &str) -> Option<ViewportTheme> {
        ViewportTheme::ALL.into_iter().find(|m| m.id() == id)
    }

    /// Its name in the Preferences (a message id).
    pub fn label_id(self) -> &'static str {
        match self {
            ViewportTheme::Theme => "ui.viewport_theme_follow",
            ViewportTheme::Light => "ui.light",
            ViewportTheme::Dark => "ui.dark",
        }
    }

    /// The view's own dark or light, or `None` to draw it as the window ([`set_viewport_dark`]).
    pub fn dark(self) -> Option<bool> {
        match self {
            ViewportTheme::Theme => None,
            ViewportTheme::Light => Some(false),
            ViewportTheme::Dark => Some(true),
        }
    }
}

/// From saved preferences: an unknown value is Theme, as [`ColorVision`]'s is Off.
impl<'de> Deserialize<'de> for ViewportTheme {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = serde_json::Value::deserialize(d)?;
        Ok(v.as_str().and_then(ViewportTheme::from_id).unwrap_or_default())
    }
}

/// The colour vision mode on now (on this thread).
pub fn vision() -> ColorVision {
    VISION.with(Cell::get)
}

/// Switch the colour vision mode (the visuals follow on the next [`apply`]).
pub fn set_vision(mode: ColorVision) {
    VISION.with(|v| v.set(mode));
}

/// An sRGB colour carried by model data, including file colours and point-cloud shading.
pub(crate) const fn model_color(rgb: [u8; 3]) -> Color32 {
    Color32::from_rgb(rgb[0], rgb[1], rgb[2])
}

/// An sRGB colour given as 0..1 (the analysis colours), for drawing in the UI.
pub fn srgb(c: [f32; 3]) -> Color32 {
    let byte = |x: f32| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color32::from_rgb(byte(c[0]), byte(c[1]), byte(c[2]))
}

/// Surface analysis colours, straight sRGB 0..1 as the shader takes them (exact, so they read
/// the same as when they were literals in the WGSL). The light theme's are the headless default.
pub use solidcraft_engine::render::AnalysisColors;

/// Colours for native viewport markers and the agent pointer. Command artwork uses the SVG palette.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IconColors {
    pub highlight: Color32,
    pub agent: Color32,
    pub marker_halo: Color32,
}

/// Every colour and size the UI draws with. The colours drawn in the 3D view are listed again in
/// [`Tokens::with_viewport`], which takes them from the other side when the 3D view is set to the
/// other one's colours (#4146): a new colour drawn in the view goes in both.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tokens {
    /// Navigation chrome from the approved Navbar boards.
    pub nav_hover: Color32,
    pub nav_press: Color32,
    pub nav_dim: Color32,
    pub nav_viewport: Color32,
    pub nav_tip: Color32,
    pub nav_tip_ink: Color32,
    pub nav_tip_key_border: Color32,
    pub nav_field: Color32,

    /// Optional structural/control strokes. None retains each surface’s legacy styling.
    pub borders: Borders,
    /// File/start presentation, with explicit foreground/background pairs.
    pub file_dim: Color32,
    pub file_recovery_well: Color32,
    pub file_action_height: f32,
    pub file_back_height: f32,
    pub file_recovery_icon: f32,
    pub file_grid_gap: f32,
    pub file_background: Color32,
    pub file_surface: Color32,
    pub file_border: Color32,
    /// Neutral multiplication tint for File-view model thumbnails.
    pub file_image_tint: Color32,
    pub file_rail: Color32,
    pub file_rail_ink: Color32,
    pub file_rail_hover: Color32,
    pub file_primary: Color32,
    pub file_primary_ink: Color32,
    pub file_primary_border: Color32,
    pub file_recovery: Color32,
    pub file_recovery_ink: Color32,
    pub file_recovery_border: Color32,
    pub file_nav_width: f32,
    pub file_nav_height: f32,
    pub file_tile_height: f32,
    pub file_tile_icon: f32,
    pub file_recent_height: f32,
    pub file_sample_image_height: f32,
    pub file_content_inset_x: f32,
    pub file_content_inset_y: f32,
    pub file_section_gap: f32,
    pub font_start_title: f32,
    // Shared dimensions in logical pixels; identical in both themes.
    pub space_2: f32,
    pub space_3: f32,
    pub space_4: f32,
    pub space_6: f32,
    pub space_8: f32,
    pub space_10: f32,
    pub space_12: f32,
    pub space_14: f32,
    pub space_16: f32,
    pub space_24: f32,
    pub radius_sm: u8,
    pub radius_md: u8,
    pub radius_lg: u8,
    pub radius_xl: u8,
    pub font_badge: f32,
    pub font_micro: f32,
    pub font_panel: f32,
    pub font_small: f32,
    pub font_caption: f32,
    pub font_body: f32,
    pub font_label: f32,
    pub font_button: f32,
    pub font_heading: f32,
    /// Tiny world-axis triad letters.
    pub font_triad: f32,
    /// Bold letters on the navigation compass discs.
    pub font_compass: f32,
    /// Home section titles, home actions and update titles.
    pub font_subtitle: f32,
    /// Introductory text on the home page.
    pub font_intro: f32,
    /// Top-level headings in assistant Markdown.
    pub font_article_heading: f32,
    /// Application title in the About dialog.
    pub font_title: f32,
    /// Application name on the home page.
    pub font_display: f32,
    /// Height of dense form controls and rows.
    pub control_height: f32,
    /// Card text, hint, primary and validation colours.
    pub form_ink: Color32,
    pub form_dim: Color32,
    pub form_primary: Color32,
    /// Primary action fill; theme files without this token inherit `form_primary`.
    pub form_primary_fill: Color32,
    pub form_error: Color32,
    pub form_error_fill: Color32,
    /// Successful validation text and its badge surface.
    pub form_success: Color32,
    pub form_success_fill: Color32,
    /// Corner radius of dense form controls and actions.
    pub control_radius: u8,
    /// Width of the dense form label column.
    pub form_label_width: f32,
    /// Vertical gap between dense form rows.
    pub form_gap_y: f32,
    /// Height of the full-width dense form footer.
    pub form_footer_height: f32,
    /// Height of the dense form header.
    pub form_header_height: f32,
    /// Outer width of a dense feature card.
    pub form_card_width: f32,
    /// Size of the icon in a dense form header.
    pub form_icon_size: f32,
    /// Size of the dense form close button.
    pub form_close_size: f32,
    /// Text inset in controls and gap after the header icon.
    pub form_inset: f32,
    /// Inset of artwork inside a dense form icon button.
    pub form_icon_inset: f32,
    /// Size of the section disclosure chevron.
    pub form_section_icon_size: f32,
    /// Height of a dense form checkbox row.
    pub form_checkbox_height: f32,
    /// Minimum height of a dense form footer action.
    pub form_button_height: f32,
    /// Outer width of the sketch options card.
    pub sketch_palette_width: f32,
    /// Square creation-mode button size and radius.
    pub sketch_mode_size: f32,
    pub sketch_mode_radius: u8,
    pub sketch_mode_border: f32,
    /// Opacity of the sketch card and its selected accent tint.
    pub sketch_palette_alpha: f32,
    pub sketch_mode_tint: f32,
    /// Footer navigation icon size.
    pub sketch_footer_icon_size: f32,
    /// Extra section heading letter spacing as a fraction of font size.
    pub font_section_tracking: f32,
    pub app_bar: Color32,
    /// Dividers between the application bar groups.
    pub app_bar_separator: Color32,
    pub toolbar: Color32,
    pub panel: Color32,
    pub panel_header: Color32,
    pub border: Color32,
    pub text: Color32,
    pub text_dim: Color32,
    pub accent: Color32,
    pub accent_soft: Color32,
    pub sketch_accent: Color32,
    pub surface_accent: Color32,
    pub mesh_accent: Color32,
    pub form_accent: Color32,
    pub render_accent: Color32,
    /// Per-tab fills, small-text inks and selected-row tints. Sketch reuses sketch_accent.
    /// The other fill keys preserve the legacy *_accent values used by other surfaces.
    pub sketch_ink: Color32,
    pub sketch_soft: Color32,
    pub surface_fill: Color32,
    pub surface_ink: Color32,
    pub surface_soft: Color32,
    pub mesh_fill: Color32,
    pub mesh_ink: Color32,
    pub mesh_soft: Color32,
    pub form_fill: Color32,
    pub form_tab_ink: Color32,
    pub form_soft: Color32,
    pub sheet_fill: Color32,
    pub sheet_ink: Color32,
    pub sheet_soft: Color32,
    pub render_fill: Color32,
    pub render_ink: Color32,
    pub render_soft: Color32,
    /// Dark text for tab pills whose fill has insufficient contrast with on_accent.
    pub tab_on_fill_dark: Color32,
    /// Ink on accent-filled buttons and selection chips.
    pub on_accent: Color32,
    /// Agent status on the dark application bar, in either theme.
    pub agent_ok: Color32,
    pub agent_busy: Color32,
    pub error: Color32,
    pub warning: Color32,
    pub icon: Color32,
    pub timeline: Color32,
    pub hover: Color32,
    // ---- viewport selection feedback ----
    /// Selected face (flat fill).
    pub sel_face: Color32,
    /// Selected edge core and its lighter rim.
    pub sel_edge: Color32,
    pub sel_edge_rim: Color32,
    /// Hovered edge: dark core with a light halo.
    pub hover_edge: Color32,
    pub hover_edge_halo: Color32,
    /// How much lighter a hovered face is (grey levels).
    pub hover_face_lift: u8,
    /// Selected profile fill and outline.
    pub sel_profile: Color32,
    pub sel_profile_edge: Color32,
    /// Hovered profile: faint tint and a bright outline.
    pub hover_profile: Color32,
    pub hover_profile_edge: Color32,
    /// Selected vertex dot.
    pub sel_vertex: Color32,
    // ---- origin ----
    pub origin_plane: Color32,
    pub origin_plane_edge: Color32,
    pub origin_plane_hover: Color32,
    pub origin_point: Color32,
    /// The X, Y and Z colours, one set for every axis drawn (#976): the origin axes, the view
    /// triad, the navigation cube's triad, the Move gizmo, a 3D sketch point's triad, joint
    /// origins, measured distances along the axes and the headless picture's origin axes.
    pub axis_x: Color32,
    pub axis_y: Color32,
    pub axis_z: Color32,
    /// Construction planes.
    pub construction_plane: Color32,
    /// Construction axes and points.
    pub construction_geom: Color32,
    // ---- bodies and live previews ----
    pub body: Color32,
    pub body_edge: Color32,
    /// Command manipulators (direction arrows, handles).
    pub manipulator: Color32,
    /// New surface in a feature preview.
    pub preview_add: Color32,
    /// The surface of a body a feature preview creates (New Body), apart from a join (#1805).
    pub preview_new: Color32,
    /// Proposed sketch geometry, distinct from blue selections.
    pub preview_sketch: Color32,
    /// Removed material in a feature preview (translucent).
    pub preview_cut: Color32,
    /// Navigation cube faces, outline and labels.
    pub cube_face: Color32,
    pub cube_border: Color32,
    pub cube_text: Color32,
    /// Light blue hover fill, readable on the pale cube in either theme.
    pub cube_hover: Color32,
    // ---- viewport ----
    /// Viewport background (top and bottom of the gradient; equal for a flat fill).
    pub viewport_top: Color32,
    pub viewport_bottom: Color32,
    pub grid: Color32,
    pub grid_major: Color32,
    /// A whole selected body.
    pub body_selected: Color32,
    /// Sketch curves: under-constrained, fully constrained (and finished sketches).
    pub sketch: Color32,
    pub sketch_fixed: Color32,
    /// Wider rim beneath sketch curves to separate them from shaded faces.
    pub sketch_halo: Color32,
    /// Fixed (locked) sketch geometry.
    pub sketch_locked: Color32,
    /// Sketch dimensions: driving, driven (reference). Each theme gives them a colour of their
    /// own, apart from the sketch curves they measure (#7861).
    pub dimension: Color32,
    pub dimension_driven: Color32,
    /// Sketch points: fully constrained, free.
    pub sketch_point: Color32,
    pub sketch_point_free: Color32,
    /// Command dialogs: panel, border, and the filled selection chip.
    pub dialog_bg: Color32,
    pub dialog_border: Color32,
    pub chip: Color32,
    /// Inside of bodies cut by a section.
    pub section_cap: Color32,
    /// Closed sketch profiles (translucent fill).
    pub profile_fill: Color32,
    /// Constraint glyph boxes.
    pub glyph_bg: Color32,
    pub glyph_edge: Color32,
    pub glyph_text: Color32,
    /// Chips drawn over the viewport (status, navigation bar, dimension labels).
    pub overlay: Color32,
    /// Rubber band of the shape being drawn.
    pub rubber_band: Color32,
    /// Light text on the application bar.
    pub app_bar_text: Color32,
    /// A hovered tab on the application bar (keeps `app_bar_text` readable).
    pub app_bar_hover: Color32,
    pub app_bar_hover_subtle: Color32,
    pub app_bar_hover_button: Color32,
    pub app_bar_hover_strong: Color32,
    /// The active document tab: a solid surface lifted from the bar.
    pub app_bar_tab_active: Color32,
    /// Dense feature dialogs: the body surface, the footer band, the dividers, input controls
    /// (fill and border), field labels, and the selected-element foreground and background.
    pub dialog_body: Color32,
    pub dialog_footer: Color32,
    pub dialog_rule: Color32,
    pub control_fill: Color32,
    pub control_border: Color32,
    pub dialog_label: Color32,
    pub selected_ink: Color32,
    pub selected_bg: Color32,
    /// Timeline items: normal and failed.
    pub timeline_item: Color32,
    pub timeline_item_error: Color32,
    pub timeline_marker: Color32,
    /// Button and field backgrounds on panels.
    pub field: Color32,
    // ---- box selection ----
    pub box_window: Color32,
    pub box_crossing: Color32,
    // ---- colours that came from the engine or were drawn in place ----
    /// Sketch curves the engine tags (`view::colors`): projected, lost link, construction,
    /// centerline.
    pub sketch_projected: Color32,
    pub sketch_lost: Color32,
    pub sketch_construction: Color32,
    pub sketch_centerline: Color32,
    /// Sheet metal flat pattern: bend up, bend down.
    pub bend_up: Color32,
    pub bend_down: Color32,
    /// Interference: the clash fill, before its shading.
    pub interference: Color32,
    /// Continuity Analysis seams, best to worst as `inspect.continuity` judges them: smooth (G2),
    /// tangent with a curvature jump (G1), a crease (G0) and a gap. Bluish green, blue, yellow
    /// and reddish purple stay apart in every colour vision mode, so no mode changes them.
    pub continuity: [Color32; 4],
    pub analysis: AnalysisColors,
    pub icons: IconColors,
    /// Optional SVG display roles and semantic ramps from the active theme file.
    pub icon_palette: Option<crate::icon_svg::Palette>,
    /// The SolidCraft mark on the application bar: light, mid and dark faces (the gaps between
    /// them show the bar).
    pub logo: [Color32; 3],
}

impl Tokens {
    /// An expanded outline follows its fill, including a theme's deliberately square corners.
    pub(crate) fn outset_radius(radius: f32, outset: f32) -> f32 {
        if radius > 0.0 { radius + outset } else { 0.0 }
    }

    /// Subtle shell elevation, derived from the semantic application-bar colour.
    pub(crate) fn shell_shadow(self) -> egui::Shadow {
        egui::Shadow { offset: [0, self.space_2 as i8], blur: self.space_8 as u8, spread: 0, color: self.app_bar.gamma_multiply(0.18) }
    }

    /// The X, Y and Z axis colours, in that order.
    pub const fn axes(&self) -> [Color32; 3] {
        [self.axis_x, self.axis_y, self.axis_z]
    }

    /// The tokens of the current theme and colour vision mode, the colours drawn in the 3D view
    /// from the view's own dark or light ([`viewport_dark`]).
    pub fn get() -> Tokens {
        Tokens::get_in(vision())
    }

    /// [`Tokens::get`] in another colour vision mode: the Colour vision preference previews the
    /// mode being chosen before it is applied (#7152).
    pub fn get_in(mode: ColorVision) -> Tokens {
        let window = Tokens::themed(is_dark(), mode);
        if viewport_dark() == is_dark() { window } else { window.with_viewport(Tokens::themed(viewport_dark(), mode)) }
    }

    /// The tokens of the 3D view's side (#4146), for a window colour drawn straight onto the view
    /// with nothing behind it (a status line, a label or a warning beside the geometry), so it
    /// reads on the view's background. As [`Tokens::get`] unless the view has its own side.
    pub fn view() -> Tokens {
        Tokens::themed(viewport_dark(), vision())
    }

    /// These tokens with the colours drawn in the 3D view taken from `view`: the background and
    /// grid, bodies, selection and hover, the origin and construction geometry, previews, sketches
    /// and their dimensions and glyphs, the navigation cube and triads, box selection and the
    /// analysis colours. The window keeps its own: panels, dialogs, menus, and the chips and cards
    /// over the view (`overlay` with its text, icons and borders).
    pub fn with_viewport(self, view: Tokens) -> Tokens {
        Tokens {
            sel_face: view.sel_face,
            sel_edge: view.sel_edge,
            sel_edge_rim: view.sel_edge_rim,
            hover_edge: view.hover_edge,
            hover_edge_halo: view.hover_edge_halo,
            hover_face_lift: view.hover_face_lift,
            sel_profile: view.sel_profile,
            sel_profile_edge: view.sel_profile_edge,
            hover_profile: view.hover_profile,
            hover_profile_edge: view.hover_profile_edge,
            sel_vertex: view.sel_vertex,
            origin_plane: view.origin_plane,
            origin_plane_edge: view.origin_plane_edge,
            origin_plane_hover: view.origin_plane_hover,
            origin_point: view.origin_point,
            axis_x: view.axis_x,
            axis_y: view.axis_y,
            axis_z: view.axis_z,
            construction_plane: view.construction_plane,
            construction_geom: view.construction_geom,
            body: view.body,
            body_edge: view.body_edge,
            manipulator: view.manipulator,
            preview_add: view.preview_add,
            preview_new: view.preview_new,
            preview_sketch: view.preview_sketch,
            preview_cut: view.preview_cut,
            cube_face: view.cube_face,
            cube_border: view.cube_border,
            cube_text: view.cube_text,
            cube_hover: view.cube_hover,
            viewport_top: view.viewport_top,
            viewport_bottom: view.viewport_bottom,
            grid: view.grid,
            grid_major: view.grid_major,
            body_selected: view.body_selected,
            sketch: view.sketch,
            sketch_fixed: view.sketch_fixed,
            sketch_locked: view.sketch_locked,
            dimension: view.dimension,
            dimension_driven: view.dimension_driven,
            sketch_point: view.sketch_point,
            sketch_point_free: view.sketch_point_free,
            section_cap: view.section_cap,
            profile_fill: view.profile_fill,
            glyph_bg: view.glyph_bg,
            glyph_edge: view.glyph_edge,
            glyph_text: view.glyph_text,
            rubber_band: view.rubber_band,
            box_window: view.box_window,
            box_crossing: view.box_crossing,
            sketch_projected: view.sketch_projected,
            sketch_lost: view.sketch_lost,
            sketch_construction: view.sketch_construction,
            sketch_centerline: view.sketch_centerline,
            bend_up: view.bend_up,
            bend_down: view.bend_down,
            interference: view.interference,
            continuity: view.continuity,
            analysis: view.analysis,
            ..self
        }
    }

    /// The tokens of a theme and colour vision mode: a theme file's palette when one is on for
    /// that base ([`set_custom`]), else the built-in one; the colour vision mode on top.
    pub fn themed(dark: bool, mode: ColorVision) -> Tokens {
        let custom = CUSTOM.get().filter(|c| c.dark == dark).map(|c| c.tokens);
        let t = custom.unwrap_or(if dark { Tokens::dark() } else { Tokens::light() });
        let t = if LOGO_ACCENT.get() { t.with_accent_logo() } else { t };
        t.with_vision(dark, mode)
    }

    /// These tokens with the logo's three faces in shades of the accent: lighter, the accent
    /// itself, darker (as the built-in logo's blues are).
    pub fn with_accent_logo(mut self) -> Tokens {
        let accent = self.accent.to_opaque();
        self.logo = [accent.lerp_to_gamma(Color32::WHITE, 0.22), accent, accent.lerp_to_gamma(Color32::BLACK, 0.2)];
        self
    }

    /// These tokens with a colour vision mode's values on top (the tables in `palettes.rs`); Off
    /// gives them back unchanged.
    pub fn with_vision(self, dark: bool, mode: ColorVision) -> Tokens {
        match mode {
            ColorVision::Off => self,
            ColorVision::Protan | ColorVision::Deutan => self.red_green(dark),
            ColorVision::Tritan => self.blue_yellow(dark),
        }
    }

    /// Fade idle cube colours into the viewport while retaining opaque facets.
    pub fn navigation_cube(self, active: bool) -> Tokens {
        if active {
            return self;
        }
        let mut t = self;
        t.cube_face = t.cube_face.lerp_to_gamma(t.viewport_top, 0.55);
        t.cube_border = t.cube_border.lerp_to_gamma(t.viewport_top, 0.55);
        t.cube_text = t.cube_text.lerp_to_gamma(t.viewport_top, 0.55);
        t.axis_x = t.axis_x.lerp_to_gamma(t.viewport_top, 0.55);
        t.axis_y = t.axis_y.lerp_to_gamma(t.viewport_top, 0.55);
        t.axis_z = t.axis_z.lerp_to_gamma(t.viewport_top, 0.55);
        t
    }
}

/// How a physical material looks (the default appearance follows the material).
pub fn material_color(name: &str) -> Option<Color32> {
    solidcraft_engine::doc::material_color(name).map(|[r, g, b]| Color32::from_rgb(r, g, b))
}

/// Switch themes (the visuals follow on the next [`apply`]).
pub fn set_dark(dark: bool) {
    DARK.set(dark);
}

/// Give the 3D view its own dark or light (#4146), or `None` to draw it as dark as the window.
pub fn set_viewport_dark(dark: Option<bool>) {
    VIEWPORT_DARK.set(dark);
}

fn visuals(dark: bool) -> egui::Visuals {
    let t = Tokens::themed(dark, vision());
    let mut v = if dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    v.panel_fill = t.panel;
    v.window_fill = t.panel;
    v.window_stroke = t.primary_border(t.border);
    v.extreme_bg_color = t.field;
    v.faint_bg_color = t.panel_header;
    v.override_text_color = Some(t.text);
    v.widgets.inactive.weak_bg_fill = t.panel_header;
    v.widgets.inactive.bg_fill = t.panel_header;
    v.widgets.active.weak_bg_fill = t.accent_soft;
    v.selection.bg_fill = t.accent_soft;
    v.selection.stroke = Stroke::new(1.0, t.accent);
    v.widgets.noninteractive.bg_stroke = t.tertiary_border(t.border);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, t.text);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, t.text);
    v.widgets.hovered.weak_bg_fill = t.hover;
    v.widgets.hovered.bg_fill = t.hover;
    // egui uses the open-state fill for a movable window's active title bar.
    v.widgets.open.weak_bg_fill = t.panel_header;
    for state in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        state.bg_stroke = t.borders.stroke(BorderRole::Control, state.bg_stroke);
    }
    v.window_corner_radius = egui::CornerRadius::same(t.radius_md);
    v.menu_corner_radius = egui::CornerRadius::same(t.radius_md);
    // Keep the existing 2/3 point egui shapes under the default 4 point small token.
    // Set every interaction state so hovering or focusing cannot restore rounded corners.
    for widget in [&mut v.widgets.noninteractive, &mut v.widgets.inactive, &mut v.widgets.active, &mut v.widgets.open] {
        widget.corner_radius = egui::CornerRadius::same(t.radius_sm / 2);
    }
    v.widgets.hovered.corner_radius = egui::CornerRadius::same(t.radius_sm.saturating_sub(1));
    // egui's own popups and windows (lists, tooltips) are glass too, as `Container` surfaces are.
    if let Some(glass) = t.borders.glass {
        v.window_fill = glass.menu_fill(v.window_fill);
        v.window_shadow = egui::Shadow::NONE;
        v.popup_shadow = glass.menu_shadow();
    }
    v
}

pub fn apply(ctx: &egui::Context) {
    let dark = is_dark();
    ctx.set_theme(if dark { egui::Theme::Dark } else { egui::Theme::Light });
    ctx.set_visuals(visuals(dark));
    let t = Tokens::get();
    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(t.space_6, t.space_4);
        s.spacing.button_padding = egui::vec2(t.space_6, t.space_3);
    });
    // The new visuals are painted on the next pass: without a repaint request the widgets
    // keep the previous theme's colours until some other event schedules a pass (#5802).
    ctx.request_repaint();
}

#[cfg(test)]
mod tests {
    #[test]
    fn bubbles_stay_inside_their_shape_shine_at_the_top_and_brighten_with_their_state() {
        use super::*;
        for shape in [BubbleShape::Round, BubbleShape::Square] {
            let glass =
                Glass { opacity: 0.7, sheen: Color32::WHITE, sheen_height: 30.0, bubbles: Some(shape), menu_opacity: None, menu_shadow: None };
            let ctx = crate::fonts::test_context();
            let (center, radius) = (egui::pos2(100.0, 100.0), 18.0);
            let paint = |center: egui::Pos2, radius: f32, state: Bubble| {
                let output = crate::test_support::frame(&ctx, egui::RawInput::default(), |ui| {
                    glass.paint_bubble(ui.painter(), center, radius, Color32::from_rgb(10, 120, 194), state);
                    glass.paint_bubble_gloss(ui.painter(), center, radius, state);
                });
                let mut vertices = Vec::new();
                for shape in &output.shapes {
                    crate::test_support::each_shape(&shape.shape, &mut |shape| {
                        if let egui::Shape::Mesh(mesh) = shape {
                            vertices.extend(mesh.vertices.iter().map(|v| (v.pos, v.color)));
                        }
                    });
                }
                (output.shapes.len(), vertices)
            };
            let nothing = crate::test_support::frame(&ctx, egui::RawInput::default(), |_| {}).shapes.len();
            let (shapes, vertices) = paint(center, radius, Bubble::Rest);
            assert!(shapes >= nothing + 4, "body, glow, highlight and rim");
            let inside = |p: egui::Pos2| match shape {
                BubbleShape::Round => p.distance(center) <= radius + 0.01,
                BubbleShape::Square => (p.x - center.x).abs() <= radius + 0.01 && (p.y - center.y).abs() <= radius + 0.01,
            };
            assert!(vertices.iter().all(|(p, _)| inside(*p)), "{shape:?}: the gradients stay inside the bubble");
            let brightest = vertices.iter().max_by_key(|(_, c)| c.a()).map(|(p, _)| *p).unwrap();
            assert!(brightest.y < center.y, "the highlight is in the top half: {brightest:?}");
            let alpha = |state| paint(center, radius, state).1.iter().map(|(_, c)| u32::from(c.a())).sum::<u32>();
            assert!(alpha(Bubble::Disabled) < alpha(Bubble::Rest) && alpha(Bubble::Rest) < alpha(Bubble::Hover), "a hovered bubble shines more");
            for (center, radius) in [(center, 2.0), (center, f32::NAN), (egui::pos2(f32::INFINITY, 0.0), radius)] {
                assert_eq!(paint(center, radius, Bubble::Hover).0, nothing, "{center:?} {radius}");
            }
        }
    }

    #[test]
    fn a_base_colour_tints_surfaces_keeping_text_readable_and_glass_follows_the_sliders() {
        use super::*;
        for base in [Tokens::light(), Tokens::dark(), crate::theme_files::resolve(crate::theme_files::find("frutiger-aero", &[]).unwrap())] {
            assert_eq!(base.personalised(Personal::default()), base, "nothing set: the theme's own");
            for tint in [Color32::from_rgb(0xcc, 0x33, 0x55), Color32::BLACK, Color32::WHITE, Color32::from_rgb(0x20, 0xa0, 0x40)] {
                let t = base.personalised(Personal { color: Some(tint), intensity: 1.0, ..Default::default() });
                assert_ne!(t.app_bar, base.app_bar, "the title bar takes the colour");
                for (fill, ink, before) in [
                    (t.app_bar, t.app_bar_text, base.app_bar),
                    (t.panel, t.text, base.panel),
                    (t.toolbar, t.text, base.toolbar),
                    (t.dialog_body, t.dialog_label, base.dialog_body),
                ] {
                    let floor = contrast_ratio(ink, before).min(4.5) - 1e-6;
                    assert!(contrast_ratio(ink, fill) >= floor, "{tint:?}: {ink:?} on {fill:?}");
                }
                assert_eq!(t.overlay.a(), base.overlay.a(), "translucent surfaces keep their alpha");
                let none = base.personalised(Personal { color: Some(tint), intensity: 0.0, ..Default::default() });
                assert_eq!(none.app_bar, base.app_bar, "no intensity: no tint");
            }
        }
        let aero = crate::theme_files::resolve(crate::theme_files::find("frutiger-aero", &[]).unwrap());
        let glass = aero.personalised(Personal { opacity: Some(0.9), gloss: Some(0.2), ..Default::default() }).borders.glass.unwrap();
        assert_eq!((glass.opacity, glass.sheen.a()), (0.9, 51));
        let clamped = aero.personalised(Personal { opacity: Some(0.0), gloss: Some(f32::NAN), ..Default::default() }).borders.glass.unwrap();
        assert_eq!((clamped.opacity, clamped.sheen), (0.3, aero.borders.glass.unwrap().sheen));
        assert!(Tokens::light().personalised(Personal { opacity: Some(0.5), ..Default::default() }).borders.glass.is_none(), "no glass to change");
    }

    #[test]
    fn tiles_fill_the_whole_button_with_their_gloss_above_the_middle_and_segments_join() {
        use super::*;
        let ctx = crate::fonts::test_context();
        let glass = Glass {
            opacity: 0.7,
            sheen: Color32::WHITE,
            sheen_height: 30.0,
            bubbles: Some(BubbleShape::Square),
            menu_opacity: None,
            menu_shadow: None,
        };
        // A ribbon button with its label: wider and taller than its icon.
        let rect = egui::Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(64.0, 58.0));
        let shapes = |paint: &dyn Fn(&egui::Painter)| {
            let output = crate::test_support::frame(&ctx, egui::RawInput::default(), |ui| paint(ui.painter()));
            let mut out = Vec::new();
            for shape in &output.shapes {
                crate::test_support::each_shape(&shape.shape, &mut |shape| out.push(shape.clone()));
            }
            out
        };
        let tile = shapes(&|p| glass.paint_tile(p, rect, egui::CornerRadius::same(4), Color32::from_rgb(10, 120, 194), Bubble::Hover));
        assert!(tile.iter().any(|s| matches!(s, egui::Shape::Rect(r) if r.rect == rect)), "the tile is the whole button");
        let vertices: Vec<_> =
            tile.iter().filter_map(|s| if let egui::Shape::Mesh(m) = s { Some(m.vertices.clone()) } else { None }).flatten().collect();
        assert!(vertices.iter().all(|v| rect.expand(0.01).contains(v.pos)), "inside the tile");
        let brightest = vertices.iter().max_by_key(|v| v.color.a()).unwrap();
        assert!(brightest.pos.y < rect.center().y, "the gloss is over the top half");
        assert!(
            shapes(&|p| glass.paint_tile(
                p,
                egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(40.0, 4.0)),
                egui::CornerRadius::ZERO,
                Color32::RED,
                Bubble::Rest
            ))
            .is_empty()
        );
        // A tile's gloss is already under its icon: nothing to finish over it.
        assert!(shapes(&|p| glass.paint_bubble_gloss(p, rect.center(), 20.0, Bubble::Hover)).is_empty());

        // Joined segments paint only with square tiles; otherwise the caller falls back.
        let mut t = Tokens::light();
        let corner = egui::CornerRadius { nw: 0, ne: 0, sw: 6, se: 0 };
        assert!(shapes(&|p| assert!(!t.tile_segment(p, rect, corner, Color32::RED, Bubble::Rest))).is_empty());
        t.borders.glass = Some(Glass { bubbles: Some(BubbleShape::Round), ..glass });
        assert!(shapes(&|p| assert!(!t.tile_segment(p, rect, corner, Color32::RED, Bubble::Rest))).is_empty());
        t.borders.glass = Some(glass);
        let segment = shapes(&|p| assert!(t.tile_segment(p, rect, corner, Color32::RED, Bubble::Rest)));
        assert!(segment.iter().any(|s| matches!(s, egui::Shape::Rect(r) if r.rect == rect && r.corner_radius == corner)), "the group's corners");
    }

    #[test]
    fn highlights_stay_flat_without_glass_and_are_top_lit_with_it() {
        use super::*;
        let ctx = crate::fonts::test_context();
        let rect = egui::Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(120.0, 24.0));
        let fill = Color32::from_rgb(0xca, 0xe6, 0xfb);
        let shapes = |t: Tokens| {
            let output = crate::test_support::frame(&ctx, egui::RawInput::default(), |ui| t.paint_highlight(ui.painter(), rect, 4u8, fill));
            let mut out = Vec::new();
            for shape in &output.shapes {
                crate::test_support::each_shape(&shape.shape, &mut |shape| out.push(shape.clone()));
            }
            out
        };
        let flat = shapes(Tokens::light());
        assert!(
            matches!(flat.as_slice(), [egui::Shape::Rect(r)] if r.rect == rect && r.fill == fill && r.stroke.is_empty()),
            "exactly the flat fill: {flat:?}"
        );
        let mut glassy = Tokens::light();
        glassy.borders.glass =
            Some(Glass { opacity: 0.7, sheen: Color32::WHITE, sheen_height: 30.0, bubbles: None, menu_opacity: None, menu_shadow: None });
        let lit = shapes(glassy);
        let mesh = lit.iter().find_map(|s| if let egui::Shape::Mesh(m) = s { Some(m.clone()) } else { None }).expect("the top light");
        let alpha_at = |top: bool| {
            mesh.vertices.iter().filter(|v| (v.pos.y - if top { rect.top() } else { rect.bottom() }).abs() < 0.01).map(|v| v.color.a()).max()
        };
        assert!(alpha_at(true) > alpha_at(false), "lit from the top");
        assert!(lit.iter().any(|s| matches!(s, egui::Shape::Rect(r) if r.stroke.width > 0.0 && r.stroke.color != fill)), "a deeper border");
    }

    #[test]
    fn glossy_buttons_brighten_their_top_half_only_and_dim_when_pressed() {
        use super::*;
        let glass = Glass { opacity: 0.7, sheen: Color32::WHITE, sheen_height: 30.0, bubbles: None, menu_opacity: None, menu_shadow: None };
        let ctx = crate::fonts::test_context();
        let rect = egui::Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(80.0, 24.0));
        let vertices = |rect: egui::Rect, pressed: bool, enabled: bool| {
            let output =
                crate::test_support::frame(&ctx, egui::RawInput::default(), |ui| glass.paint_gloss(ui.painter(), rect, 4u8, pressed, enabled));
            let mut out = Vec::new();
            for shape in &output.shapes {
                crate::test_support::each_shape(&shape.shape, &mut |shape| {
                    if let egui::Shape::Mesh(mesh) = shape {
                        out.extend(mesh.vertices.iter().map(|v| (v.pos, v.color.a())));
                    }
                });
            }
            out
        };
        let face = vertices(rect, false, true);
        assert!(!face.is_empty());
        assert!(face.iter().all(|(p, _)| rect.expand(0.01).contains(*p) && p.y <= rect.center().y + 0.01), "the top half only");
        let top = face.iter().filter(|(p, _)| p.y < rect.top() + 1.0).map(|(_, a)| *a).max().unwrap();
        let split = face.iter().filter(|(p, _)| (p.y - rect.center().y).abs() < 0.01).map(|(_, a)| *a).max().unwrap();
        assert!(top > split && split > 0, "bright at the top, softer at the split: {top} {split}");
        let sum = |v: Vec<(egui::Pos2, u8)>| v.iter().map(|(_, a)| u32::from(*a)).sum::<u32>();
        assert!(sum(vertices(rect, true, true)) < sum(face.clone()), "dimmer while pressed");
        assert!(sum(vertices(rect, false, false)) < sum(vertices(rect, true, true)), "fainter when disabled");
        for tiny in [egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(40.0, 4.0)), egui::Rect::NOTHING] {
            assert!(vertices(tiny, false, true).is_empty(), "{tiny:?}");
        }
    }

    #[test]
    fn widget_states_and_popup_frames_follow_theme_radii_without_changing_defaults() {
        use super::*;
        let old_custom = CUSTOM.get();
        for dark in [false, true] {
            set_custom(None);
            let actual = visuals(dark);
            let original = if dark { egui::Visuals::dark() } else { egui::Visuals::light() };
            assert_eq!(actual.menu_corner_radius, original.menu_corner_radius);
            for (a, b) in [
                (actual.widgets.noninteractive, original.widgets.noninteractive),
                (actual.widgets.inactive, original.widgets.inactive),
                (actual.widgets.hovered, original.widgets.hovered),
                (actual.widgets.active, original.widgets.active),
                (actual.widgets.open, original.widgets.open),
            ] {
                assert_eq!(a.corner_radius, b.corner_radius);
            }
            let tokens = Tokens { radius_sm: 0, radius_md: 0, ..Tokens::dark() };
            set_custom(Some(Custom { dark, tokens }));
            let square = visuals(dark);
            set_custom(old_custom);
            assert_eq!(square.window_corner_radius, egui::CornerRadius::ZERO);
            assert_eq!(square.menu_corner_radius, egui::CornerRadius::ZERO);
            for widget in [square.widgets.noninteractive, square.widgets.inactive, square.widgets.hovered, square.widgets.active, square.widgets.open]
            {
                assert_eq!(widget.corner_radius, egui::CornerRadius::ZERO);
            }
        }
        assert_eq!(Tokens::outset_radius(0.0, 3.0), 0.0);
        assert_eq!(Tokens::outset_radius(6.0, 3.0), 9.0);
    }

    /// A theme change repaints: egui paints the new visuals on the next pass, so without a
    /// repaint request the widgets keep the previous theme's colours until another event
    /// schedules a pass (#5802).
    #[test]
    fn applying_a_theme_asks_for_a_repaint_with_the_new_visuals() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let ctx = egui::Context::default();
        // The integration hook fires on every repaint request, whoever asked for it.
        let requests = std::sync::Arc::new(AtomicUsize::new(0));
        let counter = std::sync::Arc::clone(&requests);
        ctx.set_request_repaint_callback(move |_| {
            counter.fetch_add(1, Ordering::Relaxed);
        });
        // Let egui's own startup repaints settle: the context must be idle before the
        // theme changes, so the only repaint left is the one the theme asks for.
        let mut settled = false;
        for _ in 0..8 {
            ctx.run_ui(egui::RawInput::default(), |_| {}).textures_delta.clear();
            if !ctx.has_requested_repaint() {
                settled = true;
                break;
            }
        }
        assert!(settled, "the context must be idle before the theme changes");
        let before = requests.load(Ordering::Relaxed);
        let previous = super::is_dark();
        super::set_dark(!previous);
        let dark = super::is_dark();
        let expected = super::visuals(dark);
        // The theme is applied outside a pass: inside one egui schedules repaints of its
        // own, and the request of the theme would hide among them.
        super::apply(&ctx);
        super::set_dark(previous);
        assert!(requests.load(Ordering::Relaxed) > before, "a theme change must ask for a repaint");
        assert!(ctx.has_requested_repaint(), "a theme change must leave a repaint pending");
        let theme = if dark { egui::Theme::Dark } else { egui::Theme::Light };
        assert_eq!(ctx.style_of(theme).visuals, expected, "the new theme's visuals must be in place");
    }

    #[test]
    fn file_view_foreground_pairs_hold_small_text_contrast_without_changing_accent() {
        for dark in [false, true] {
            for vision in super::ColorVision::ALL {
                let t = super::Tokens::themed(dark, vision);
                for (ink, surface) in [
                    (t.file_rail_ink, t.file_rail),
                    (t.file_rail_ink, t.file_rail_hover),
                    (t.file_primary_ink, t.file_primary),
                    (t.file_primary_ink, t.file_background),
                    (t.file_recovery_ink, t.file_recovery),
                    (t.form_ink, t.file_surface),
                    (t.file_dim, t.file_surface),
                    (t.file_dim, t.file_background),
                    (t.file_dim, t.file_primary),
                ] {
                    assert!(super::contrast_ratio(ink, surface) >= 4.5, "dark={dark}: {ink:?} on {surface:?}");
                }
            }
        }
    }

    #[test]
    fn tab_accents_map_explicit_tokens_without_changing_model() {
        for dark in [false, true] {
            for vision in super::ColorVision::ALL {
                let t = super::Tokens::themed(dark, vision);
                // Explicit palette tints and text replace the old gamma lerps. They can be
                // overridden independently by theme files and hold small-text contrast.
                for (tab, color, ink, soft) in [
                    ("SKETCH", t.sketch_accent, t.sketch_ink, t.sketch_soft),
                    ("SURFACE", t.surface_fill, t.surface_ink, t.surface_soft),
                    ("MESH", t.mesh_fill, t.mesh_ink, t.mesh_soft),
                    ("FORM", t.form_fill, t.form_tab_ink, t.form_soft),
                    ("SHEET METAL", t.sheet_fill, t.sheet_ink, t.sheet_soft),
                    ("RENDER", t.render_fill, t.render_ink, t.render_soft),
                ] {
                    let accent = super::TabAccent::for_tab(&t, tab);
                    assert_eq!(accent, super::TabAccent { color, ink, soft });
                    assert_ne!(accent.soft, color);
                    assert_ne!(accent.soft, t.panel);
                }
                for tab in ["MODEL", "SOLID", "ASSEMBLE", "INSERT", "INSPECT", "MANAGE", "VIEW", "unknown"] {
                    assert_eq!(super::TabAccent::for_tab(&t, tab), super::TabAccent { color: t.accent, ink: t.selected_ink, soft: t.accent_soft });
                }
            }
        }
    }

    #[test]
    fn tab_accents_match_approved_palette() {
        let rgb = |hex: u32| egui::Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8);
        for (tab, light, dark) in [
            ("SKETCH", [0x057f45, 0x057f45, 0xedf7f1], [0x128d45, 0x46be78, 0x1f3b2b]),
            ("SURFACE", [0xa45000, 0xa45000, 0xfbebd8], [0xb26a14, 0xe0914a, 0x45331e]),
            ("MESH", [0xe0b200, 0x8a6d00, 0xfff8dc], [0xc6a800, 0xd4b726, 0x41391a]),
            ("FORM", [0x7349cd, 0x7349cd, 0xeee8fa], [0x8a6fcc, 0xb39bee, 0x372f4f]),
            ("SHEET METAL", [0x1e6f96, 0x1e6f96, 0xe1f0f7], [0x2a7fa8, 0x4fb3d9, 0x1f3a47]),
            ("RENDER", [0xa8326e, 0xa8326e, 0xf8e6ef], [0xc8588f, 0xe074ae, 0x402336]),
        ] {
            for (t, values) in [(super::Tokens::light(), light), (super::Tokens::dark(), dark)] {
                let [color, ink, soft] = values.map(rgb);
                assert_eq!(super::TabAccent::for_tab(&t, tab), super::TabAccent { color, ink, soft }, "{tab}");
            }
            let accent = super::TabAccent::for_tab(&super::Tokens::light(), tab);
            assert_eq!(accent.ink != accent.color, tab == "MESH", "only light Mesh separates ink from fill");
        }
        // These existing Model values intentionally take precedence over the board's proposal.
        for (t, values) in [(super::Tokens::light(), [0x2678da, 0x1a507f, 0xd6e6fa]), (super::Tokens::dark(), [0x3a88ee, 0x9cc7f5, 0x2a466e])] {
            let [color, ink, soft] = values.map(rgb);
            assert_eq!(super::TabAccent::for_tab(&t, "MODEL"), super::TabAccent { color, ink, soft });
        }
    }

    #[test]
    fn built_in_selected_tab_labels_hold_wcag_small_text_contrast() {
        let mut themes = vec![("Dark", super::Tokens::dark()), ("Light", super::Tokens::light())];
        themes.extend(crate::theme_files::builtin().iter().map(|theme| (theme.name.as_str(), crate::theme_files::resolve(theme))));
        let mut failures = Vec::new();
        for (name, tokens) in themes {
            for tab in crate::workspace::TABS.iter().copied().chain(["SKETCH", "MESH", "FORM", "SHEET METAL", "RENDER"]) {
                let accent = super::TabAccent::for_tab(&tokens, tab);
                let ratio = super::contrast_ratio(accent.ink, accent.soft);
                if ratio < 4.5 {
                    failures.push(format!("{name} {tab}: {ratio:.4}:1 ({:?} on {:?})", accent.ink, accent.soft));
                }
            }
        }
        assert!(failures.is_empty(), "selected tab contrast below 4.5:1:\n{}", failures.join("\n"));
    }

    #[test]
    fn tab_accents_hold_wcag_small_text_contrast_in_both_themes() {
        for (dark, t) in [(false, super::Tokens::light()), (true, super::Tokens::dark())] {
            for tab in ["MODEL", "SKETCH", "SURFACE", "MESH", "FORM", "SHEET METAL", "RENDER"] {
                let accent = super::TabAccent::for_tab(&t, tab);
                let card = if dark { t.panel } else { egui::Color32::WHITE };
                for (name, ink, ground) in [
                    ("ink on card", accent.ink, card),
                    ("ink on panel", accent.ink, t.panel),
                    ("ink on soft", accent.ink, accent.soft),
                    ("text on fill", accent.on_fill(&t), accent.color),
                ] {
                    let ratio = super::contrast_ratio(ink, ground);
                    assert!(ratio >= 4.5, "{tab} dark={dark}: {name} is {ratio:.4}:1 ({ink:?} on {ground:?})");
                }
                let white_ok = super::contrast_ratio(t.on_accent, accent.color) >= 4.5;
                assert_eq!(accent.on_fill(&t), if white_ok { t.on_accent } else { t.tab_on_fill_dark });
            }
        }
    }

    #[test]
    fn tab_accent_context_is_per_app_and_does_not_change_widget_visuals() {
        let mut harness = crate::scenario::Harness::new();
        let app = &mut harness.app;
        let first = crate::fonts::test_context();
        let second = crate::fonts::test_context();
        let before = first.style_of(egui::Theme::Dark).visuals.clone();
        app.ui.tab = "SURFACE".into();
        super::TabAccent::publish(app, &first);
        assert_eq!(super::TabAccent::current(&first).color, super::Tokens::get().surface_fill);
        assert_eq!(super::TabAccent::current(&second).color, super::Tokens::get().accent);
        assert_eq!(super::TabAccent::current_icon_accent(&first), Some("surface"));
        assert_eq!(super::TabAccent::current_icon_accent(&second), None);
        assert_eq!(first.style_of(egui::Theme::Dark).visuals, before, "dialog OK, checkboxes and status styles must not inherit the tab");
        app.ui.tab = "MODEL".into();
        super::TabAccent::publish(app, &first);
        assert_eq!(super::TabAccent::current(&first).soft, super::Tokens::get().accent_soft);
        assert_eq!(super::TabAccent::current_icon_accent(&first), None);
    }

    #[test]
    fn concurrent_ui_renderers_keep_their_own_theme_and_vision() {
        let barrier = std::sync::Barrier::new(2);
        std::thread::scope(|scope| {
            for (dark, vision) in [(false, super::ColorVision::Off), (true, super::ColorVision::Protan)] {
                let barrier = &barrier;
                scope.spawn(move || {
                    super::set_dark(dark);
                    super::set_vision(vision);
                    barrier.wait();
                    assert_eq!(super::is_dark(), dark);
                    assert_eq!(super::vision(), vision);
                    assert_eq!(super::Tokens::get().sketch_accent, super::Tokens::themed(dark, vision).sketch_accent);
                });
            }
        });
    }

    /// A 3D view set to the other side's colours (#4146): the view's colours come from the other
    /// side and everything else stays the window's, in every colour vision mode. A view as dark as
    /// the window gives the window's tokens unchanged.
    #[test]
    fn a_view_on_the_other_side_takes_its_colours() {
        use super::{ColorVision, Tokens};
        for mode in ColorVision::ALL {
            super::set_vision(mode);
            for window in [false, true] {
                super::set_dark(window);
                for same in [None, Some(window)] {
                    super::set_viewport_dark(same);
                    assert_eq!(super::viewport_dark(), window);
                    assert_eq!(Tokens::get(), Tokens::themed(window, mode), "{mode:?} {window} {same:?}");
                    assert_eq!(Tokens::view(), Tokens::get(), "{mode:?} {window} {same:?}");
                }
                super::set_viewport_dark(Some(!window));
                assert_eq!(super::viewport_dark(), !window);
                let (t, w, v) = (Tokens::get(), Tokens::themed(window, mode), Tokens::themed(!window, mode));
                // What is drawn straight onto the view takes all of its side's colours.
                assert_eq!(Tokens::view(), v);
                // The two sides differ, so the checks below tell them apart.
                assert_ne!((w.viewport_top, w.panel), (v.viewport_top, v.panel));
                // The view's: background and grid, bodies, selection and hover, sketches with their
                // dimensions and glyphs, the origin, the cube and triads, box selection, analysis.
                assert_eq!((t.viewport_top, t.viewport_bottom, t.grid, t.grid_major), (v.viewport_top, v.viewport_bottom, v.grid, v.grid_major));
                assert_eq!(
                    (t.body, t.body_edge, t.body_selected, t.sel_face, t.sel_edge, t.hover_edge),
                    (v.body, v.body_edge, v.body_selected, v.sel_face, v.sel_edge, v.hover_edge)
                );
                assert_eq!(
                    (t.sketch, t.sketch_fixed, t.dimension, t.glyph_bg, t.glyph_text, t.profile_fill),
                    (v.sketch, v.sketch_fixed, v.dimension, v.glyph_bg, v.glyph_text, v.profile_fill)
                );
                assert_eq!(
                    (t.origin_plane, t.axis_x, t.cube_face, t.cube_text, t.axis_y, t.axis_z),
                    (v.origin_plane, v.axis_x, v.cube_face, v.cube_text, v.axis_y, v.axis_z)
                );
                assert_eq!(
                    (t.box_window, t.section_cap, t.preview_add, t.sketch_construction, t.continuity),
                    (v.box_window, v.section_cap, v.preview_add, v.sketch_construction, v.continuity)
                );
                assert_eq!(t.analysis, v.analysis);
                // The window's: panels, text, accents, dialogs, and the chips over the view with
                // their borders and icons.
                assert_eq!(
                    (t.panel, t.text, t.border, t.accent, t.accent_soft, t.icon),
                    (w.panel, w.text, w.border, w.accent, w.accent_soft, w.icon)
                );
                assert_eq!(
                    (t.overlay, t.hover, t.dialog_bg, t.field, t.timeline, t.app_bar),
                    (w.overlay, w.hover, w.dialog_bg, w.field, w.timeline, w.app_bar)
                );
            }
        }
        super::set_viewport_dark(None);
    }

    #[test]
    fn the_time_of_days_colour_goes_round_the_clock_without_a_jump() {
        use super::{Color32, DAYLIGHT, daylight};
        // Each colour at its hour, and midnight's again at the end of the day.
        for (at, [r, g, b]) in DAYLIGHT {
            assert_eq!(daylight(at), Color32::from_rgb(r, g, b), "{at}");
        }
        assert_eq!(daylight(24.0), daylight(0.0));
        assert_eq!(daylight(-1.0), daylight(23.0), "any hour, round the clock");
        assert_eq!(daylight(f32::NAN), daylight(12.0), "not a number: midday");
        // Minute by minute, no step is more than a shade.
        let mut before = daylight(0.0);
        for minute in 1..=24 * 60 {
            let now = daylight(minute as f32 / 60.0);
            let step = [now.r().abs_diff(before.r()), now.g().abs_diff(before.g()), now.b().abs_diff(before.b())];
            assert!(step.iter().all(|d| *d <= 3), "minute {minute}: {before:?} to {now:?}");
            before = now;
        }
        // Night is deep and midday bright.
        assert!(super::luminance(daylight(1.0)) < 0.05 && super::luminance(daylight(13.0)) > 0.3);
    }

    #[test]
    fn the_title_bar_follows_a_strong_background_colour_and_its_text_stays_readable() {
        use super::*;
        let opaque = |c: Color32| Color32::from_rgb(c.r(), c.g(), c.b());
        for base in [Tokens::light(), Tokens::dark()] {
            let floor = contrast_ratio(base.app_bar_text, opaque(base.app_bar)).min(4.5) - 0.01;
            // Night's indigo, a strong yellow and a strong red, as they are (not evened out).
            for strong in [daylight(0.0), Color32::from_rgb(255, 230, 0), Color32::from_rgb(200, 0, 40)] {
                let t = base.personalised(Personal { sky: Some([strong, strong]), ..Default::default() });
                assert_ne!(t.app_bar, base.app_bar, "{strong:?} shows in the title bar");
                assert!(contrast_ratio(t.app_bar_text, opaque(t.app_bar)) >= floor, "{strong:?}: {:?} on {:?}", t.app_bar_text, t.app_bar);
            }
        }
    }
}
