//! The only constructors for outer UI hosts. Content widgets belong to the other kit modules.
//! A surface declares its purpose; the shell supplies the layout once per frame. Never infer
//! floating/docked presentation from a panel name or keep a second layout preference here.
use crate::frame::Side;
use crate::{shell::LayoutMode, theme::Tokens};
use egui::{Color32, Context, Frame, Id, Stroke, WidgetText};

/// Persistent panels adapt to the layout; transient UI retains its anchoring and modality.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    /// Browser, History, Render settings and future workspace panels.
    Panel,
    /// Feature forms and the Assistant sheet.
    Feature,
    /// Sketch options, including their translucent background.
    Palette,
    /// Workspace switcher and ribbon cards.
    Chrome,
    /// Native application/title bar and the optional status bar along the window's bottom edge;
    /// their presentation is independent of workspace layout.
    ApplicationBar,
    /// Movable system windows and modal dialogs; never become workspace docks.
    Window,
    /// Anchored menus, completion lists and rename editors.
    Popup,
    /// Model-anchored editors and manipulation feedback.
    Canvas,
    /// Status/progress notifications, independent of the workspace layout.
    Toast,
    /// Viewport navigation controls (the compass itself is drawn geometry).
    Navigation,
    /// Full-page content, the model viewport and native pop-out content.
    Viewport,
    /// Deliberate input-blocking backdrops; must not inherit CAD input blocking.
    InputShield,
}

#[derive(Clone, Copy, Debug)]
pub enum Edge {
    Left,
    Right,
    Top,
    Bottom,
}

/// Edges that meet the window or another docked surface. Only exposed corners round.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DockEdges {
    pub left: bool,
    pub right: bool,
    pub top: bool,
    pub bottom: bool,
}

impl DockEdges {
    pub const ALL: Self = Self { left: true, right: true, top: true, bottom: true };
}

/// Where a glass host's sheen goes: a place reserved under its contents
/// (`Container::begin_content`). Empty without glass.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct GlassSlot(Option<egui::layers::ShapeIdx>);

/// Universal outer host and surface policy. Both host geometry and frame construction must go
/// through this API, including code in nested modules and platform apps.
#[derive(Clone, Copy)]
pub struct Container {
    surface: Surface,
    layout: LayoutMode,
    joined: Option<DockEdges>,
}

impl Container {
    pub fn new(ctx: &Context, surface: Surface) -> Self {
        Self::for_layout(LayoutMode::current(ctx), surface)
    }

    pub(crate) fn for_layout(layout: LayoutMode, surface: Surface) -> Self {
        Self { surface, layout, joined: None }
    }

    /// Floating workspace cards retain their shared chrome even in compact ribbon layouts.
    pub(crate) fn floating_card(surface: Surface) -> Self {
        Self::for_layout(LayoutMode::Floating, surface)
    }

    /// Shared bottom-strip placement, used by History and other workspace strips.
    pub(crate) fn bottom_strip_rect(self, available: egui::Rect, height: f32) -> egui::Rect {
        let inset = available.shrink(self.gap());
        egui::Rect::from_min_max(egui::pos2(inset.left(), (inset.bottom() - height).max(inset.top())), inset.max)
    }

    /// Anchored navigation surfaces share the compass/nav-bar frame and token padding.
    pub(crate) fn navigation_card<R>(
        ctx: &Context,
        id: Id,
        anchor: egui::Pos2,
        pivot: egui::Align2,
        add: impl FnOnce(&mut egui::Ui) -> R,
    ) -> egui::InnerResponse<R> {
        let container = Self::new(ctx, Surface::Navigation);
        container.area(id).movable(false).pivot(pivot).fixed_pos(anchor).order(egui::Order::Middle).show(ctx, |ui| {
            let frame = container.frame(ctx, Tokens::get().overlay, Side::Left).inner_margin(Tokens::get().space_4);
            container.show_frame(ui, frame, ctx.content_rect(), |ui| super::scope(ui, add)).inner
        })
    }

    /// Reserve an anchored one-row Navigation card and the common card gap.
    pub(crate) fn below_navigation_row(bounds: egui::Rect) -> egui::Rect {
        let t = Tokens::get();
        egui::Rect::from_min_max(egui::pos2(bounds.left(), (bounds.top() + t.control_height + t.space_8 * 2.0).min(bounds.bottom())), bounds.max)
    }

    /// Inline navigation band using the same chrome and padding as anchored HUDs.
    pub(crate) fn navigation_contents<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> egui::InnerResponse<R> {
        let container = Self::new(ui.ctx(), Surface::Navigation);
        let frame = container.frame(ui.ctx(), Tokens::get().overlay, Side::Left).inner_margin(Tokens::get().space_4);
        container.show_frame(ui, frame, ui.max_rect(), |ui| super::scope(ui, add))
    }

    /// Declare contacts from shell geometry; styling remains entirely in the container.
    pub fn joined(mut self, edges: DockEdges) -> Self {
        self.joined = Some(edges);
        self
    }

    /// Shell-owned placement policy, shared by current and future adaptive panels.
    pub fn floating(self) -> bool {
        matches!(self.surface, Surface::Panel | Surface::Feature | Surface::Palette | Surface::Chrome) && self.layout == LayoutMode::Floating
    }

    /// Separation between adaptive workspace surfaces; compact surfaces meet their edge.
    pub fn gap(self) -> f32 {
        if self.floating() { crate::shell::SHELL_GAP } else { 0.0 }
    }

    /// Tokenised outer chrome. Callers may supply content padding, not another card surface.
    pub fn frame(self, ctx: &Context, fill: Color32, side: crate::frame::Side) -> Frame {
        let t = Tokens::get();
        let flat = Frame::NONE.fill(fill);
        let frame = match self.surface {
            Surface::Feature | Surface::Palette if !self.floating() => {
                let edges = self.joined.unwrap_or(DockEdges { left: side == Side::Left, right: side == Side::Right, ..Default::default() });
                Self::compact_frame(fill, t.primary_border(t.dialog_border), edges)
            }
            Surface::Panel | Surface::Chrome if !self.floating() => {
                Self::compact_frame(fill, t.primary_border(t.border), self.joined.unwrap_or(DockEdges::ALL))
            }
            Surface::Feature | Surface::Window => flat.stroke(t.primary_border(t.dialog_border)).corner_radius(t.radius_xl).shadow(t.shell_shadow()),
            Surface::Palette => flat.stroke(t.primary_border(t.border)).corner_radius(t.radius_xl).shadow(t.shell_shadow()),
            Surface::Panel | Surface::Chrome => flat.stroke(t.primary_border(t.border)).corner_radius(t.radius_lg).shadow(t.shell_shadow()),
            Surface::Navigation => flat.stroke(t.primary_border(t.border)).corner_radius(t.radius_md),
            Surface::Toast => flat.stroke(t.primary_border(t.border)).corner_radius(t.radius_md).shadow(t.shell_shadow()),
            Surface::Popup => Self::popup_frame(&ctx.global_style()).fill(fill),
            Surface::ApplicationBar | Surface::Canvas | Surface::Viewport | Surface::InputShield => flat,
        };
        self.glazed(frame)
    }

    /// A glass theme's surfaces over the 3D view let it show through, without a drop shadow
    /// (it would show through too); menus, popups and notifications by the theme's menu opacity,
    /// so they stay readable over busy panels. `decorate` adds their sheen.
    fn glazed(self, frame: Frame) -> Frame {
        let over_view = self.floating() || matches!(self.surface, Surface::Window | Surface::Popup | Surface::Toast | Surface::Navigation);
        match Tokens::get().borders.glass {
            // Menus, popups and notifications cast the theme's menu shadow; nearly solid, they
            // don't show it through.
            Some(glass) if over_view => match self.surface {
                Surface::Popup | Surface::Toast => frame.fill(glass.menu_fill(frame.fill)).shadow(glass.menu_shadow()),
                _ => frame.fill(glass.fill(frame.fill)).shadow(egui::Shadow::NONE),
            },
            _ => frame,
        }
    }

    /// Movable tool sheets round all exposed corners, joining only their right dock edge (or the
    /// edges they were declared `joined` at).
    pub(crate) fn tool_frame(self, mut frame: Frame, right_docked: bool) -> Frame {
        if self.surface == Surface::Feature && !self.floating() {
            frame.corner_radius = Self::compact_rounding(self.joined.unwrap_or(DockEdges { right: right_docked, ..Default::default() }));
        }
        frame
    }

    /// Surface of an allocated dock or internal pinned band. Floating card decoration is
    /// supplied by `frame`; edge-hosted panes must not acquire outer shadows or card gaps.
    pub fn edge_frame(self, fill: Color32) -> Frame {
        if self.adaptive() && !self.floating() {
            Self::compact_frame(fill, Tokens::get().primary_border(Tokens::get().border), self.joined.unwrap_or(DockEdges::ALL))
        } else {
            Frame::NONE.fill(fill)
        }
    }

    fn adaptive(self) -> bool {
        matches!(self.surface, Surface::Panel | Surface::Feature | Surface::Palette | Surface::Chrome)
    }

    fn compact_rounding(edges: DockEdges) -> egui::CornerRadius {
        let radius = Tokens::get().radius_lg;
        egui::CornerRadius {
            nw: if edges.left || edges.top { 0 } else { radius },
            ne: if edges.right || edges.top { 0 } else { radius },
            sw: if edges.left || edges.bottom { 0 } else { radius },
            se: if edges.right || edges.bottom { 0 } else { radius },
        }
    }

    fn compact_frame(fill: Color32, border: Stroke, edges: DockEdges) -> Frame {
        // Collapse adjoining borders onto the same themed-width strip. North/west outlines
        // sit outside the allocation; south/east outlines sit inside it. Logical panel
        // rectangles still touch exactly, with neither gaps nor doubled strokes.
        Frame::NONE
            .fill(fill)
            .stroke(border)
            .outer_margin(egui::Margin { left: -(border.width.ceil() as i8), top: -(border.width.ceil() as i8), right: 0, bottom: 0 })
            .corner_radius(Self::compact_rounding(edges))
    }

    /// Measure content before rounding a compact card. Reaching a dock boundary joins
    /// that edge, so a content-hugging card and a full-height card share one composition.
    pub(crate) fn show_frame<R>(
        self,
        ui: &mut egui::Ui,
        frame: Frame,
        bounds: egui::Rect,
        body: impl FnOnce(&mut egui::Ui) -> R,
    ) -> egui::InnerResponse<R> {
        let mut prepared = frame.begin(ui);
        let glass = Self::begin_content(&prepared.content_ui);
        let inner = body(&mut prepared.content_ui);
        if self.adaptive() && !self.floating() {
            let rect = prepared.frame.outer_rect(prepared.content_ui.min_rect());
            let mut edges = self.joined.unwrap_or(DockEdges::ALL);
            edges.left |= rect.left() <= bounds.left();
            edges.right |= rect.right() >= bounds.right();
            edges.top |= rect.top() <= bounds.top();
            edges.bottom |= rect.bottom() >= bounds.bottom();
            prepared.frame.corner_radius = Self::compact_rounding(edges);
        }
        let frame = prepared.frame;
        let response = prepared.end(ui);
        Self::decorate(ui.painter(), response.rect - frame.outer_margin, frame, glass, 0.0);
        egui::InnerResponse { inner, response }
    }

    /// Reserve, before a host's contents, the place of its glass sheen, so the sheen paints
    /// under them and never over text or buttons. Pass it to `finish_content`.
    pub(crate) fn begin_content(ui: &egui::Ui) -> GlassSlot {
        GlassSlot(Tokens::get().borders.glass.map(|_| ui.painter().add(egui::Shape::Noop)))
    }

    /// Optional companion to a primary outline, painted inside the allocated surface.
    /// It consumes no layout space or input target. Tiny/collapsed surfaces omit it.
    pub(crate) fn paint_inset(painter: &egui::Painter, widget: egui::Rect, frame: Frame) {
        Self::decorate(painter, widget, frame, GlassSlot::default(), 0.0);
    }

    /// A host's own decoration: its glass sheen (into `glass`, under its contents), bevel and
    /// inset outline. It stays inside what clips the host (its scroll area, its window), so
    /// scrolled-away content leaves nothing behind; `padding` widens that clip for a content
    /// painter, whose clip leaves out the host's padding the decoration belongs to.
    fn decorate(painter: &egui::Painter, widget: egui::Rect, frame: Frame, glass: GlassSlot, padding: f32) {
        let t = Tokens::get();
        let mut chrome = painter.clone();
        chrome.set_clip_rect(widget.intersect(painter.clip_rect().expand(padding)).intersect(painter.ctx().content_rect()));
        // Glass surfaces are the translucent ones (`glazed`); an opaque or empty fill has no sheen.
        if glass.0.is_some() && t.borders.glass.is_some() && frame.fill.a() > 0 && frame.fill.a() < u8::MAX {
            crate::theme::glass_area(&chrome, widget);
        }
        if let (Some(slot), Some(glass)) = (glass.0, t.borders.glass)
            && frame.fill.a() > 0
            && frame.fill.a() < u8::MAX
            && let Some(sheen) = glass.sheen_mesh(widget.shrink(frame.stroke.width), frame.corner_radius)
        {
            let streaks = glass.streaks_mesh(painter.ctx().content_rect(), widget.shrink(frame.stroke.width + 1.0));
            chrome.set(slot, egui::Shape::Vec(std::iter::once(sheen.into()).chain(streaks.map(Into::into)).collect()));
        }
        if let Some(bevel) = t.borders.bevel {
            bevel.paint(&chrome, widget, frame.corner_radius, false, true);
        }
        let Some(inset) = t.borders.primary_inset else {
            return;
        };
        let distance = frame.stroke.width + inset.gap;
        let rect = widget.shrink(distance);
        if inset.stroke.width <= 0.0 || !rect.is_finite() || rect.width() <= 2.0 * inset.stroke.width || rect.height() <= 2.0 * inset.stroke.width {
            return;
        }
        let reduce = |r: u8| r.saturating_sub(distance.ceil() as u8);
        let radius = frame.corner_radius;
        chrome.rect_stroke(
            rect,
            egui::CornerRadius { nw: reduce(radius.nw), ne: reduce(radius.ne), sw: reduce(radius.sw), se: reduce(radius.se) },
            inset.stroke,
            egui::StrokeKind::Inside,
        );
    }

    /// A frame's shape around `content` with its glass sheen, for a host that paints its frame
    /// itself before its contents (ribbon cards, the command search sheet): the sheen comes out
    /// under them. `paint_inset` still adds the rim afterwards. `painter` is the one it goes into.
    pub(crate) fn painted(painter: &egui::Painter, frame: Frame, content: egui::Rect) -> egui::Shape {
        let widget = frame.widget_rect(content);
        let glass = Tokens::get().borders.glass.filter(|_| frame.fill.a() > 0 && frame.fill.a() < u8::MAX);
        let Some(glass) = glass else { return frame.paint(content) };
        crate::theme::glass_area(painter, widget);
        let mut shapes = vec![frame.paint(content)];
        shapes.extend(glass.sheen_mesh(widget.shrink(frame.stroke.width), frame.corner_radius).map(Into::into));
        // Painted in the host's own painter, outside a clip of its own: the mesh is cut to the host.
        shapes.extend(glass.streaks_mesh(Tokens::screen(), widget.shrink(frame.stroke.width + 1.0)).map(Into::into));
        egui::Shape::Vec(shapes)
    }

    /// Manually painted chrome uses exactly the same outer and inset policy as hosts. The
    /// caller paints its contents afterwards, so the sheen goes in now, under them.
    pub(crate) fn paint_frame(painter: &egui::Painter, frame: Frame, content: egui::Rect) {
        painter.add(frame.paint(content));
        let glass = GlassSlot(Tokens::get().borders.glass.map(|_| painter.add(egui::Shape::Noop)));
        Self::decorate(painter, frame.widget_rect(content), frame, glass, 0.0);
    }

    /// Decorate the content-hugging frame of a popup or feature sheet after its contents, with
    /// the sheen in the place `begin_content` reserved before them.
    pub(crate) fn finish_content(ui: &egui::Ui, frame: Frame, glass: GlassSlot) {
        let m = frame.inner_margin;
        let padding = [m.leftf(), m.rightf(), m.topf(), m.bottomf()].into_iter().fold(0.0, f32::max) + frame.stroke.width;
        Self::decorate(ui.painter(), frame.widget_rect(ui.min_rect()), frame, glass, padding);
    }

    pub fn area(self, id: Id) -> egui::Area {
        // Persistent workspace cards stay below tool/system windows even when clicked.
        // Other areas keep egui's normal window layer; popups choose their own order.
        let order = if self.surface == Surface::Panel { egui::Order::Background } else { egui::Order::Middle };
        egui::Area::new(id).enabled(self.input_enabled()).order(order)
    }

    pub fn edge(self, id: impl Into<Id>, edge: Edge) -> egui::Panel {
        let panel = match edge {
            Edge::Left => egui::Panel::left(id),
            Edge::Right => egui::Panel::right(id),
            Edge::Top => egui::Panel::top(id),
            Edge::Bottom => egui::Panel::bottom(id),
        };
        if self.adaptive() && !self.floating() { panel.show_separator_line(false) } else { panel }
    }

    pub(super) fn window<'a>(self, title: impl Into<WidgetText>) -> egui::Window<'a> {
        egui::Window::new(title).enabled(self.input_enabled())
    }

    /// Anchored editors inherit their containing widget's local style. Their host stays a
    /// popup in both layouts; only the universal module constructs the egui popup frame.
    pub fn popup_frame(style: &egui::Style) -> Frame {
        let mut frame = Frame::popup(style);
        frame.stroke = Tokens::get().borders.stroke(crate::theme::BorderRole::Primary, frame.stroke);
        frame
    }

    pub(crate) fn menu_frame() -> Frame {
        let t = Tokens::get();
        let frame = egui::Frame::NONE
            .fill(t.dialog_body)
            .stroke(t.primary_border(t.dialog_border))
            .corner_radius(t.radius_xl)
            .inner_margin(egui::Margin::symmetric(t.space_6 as i8, t.space_6 as i8))
            .shadow(egui::Shadow {
                offset: [0, t.space_14 as i8],
                blur: (t.space_24 + t.space_12) as u8,
                spread: 0,
                color: t.app_bar.gamma_multiply(0.18),
            });
        Self::for_layout(LayoutMode::Floating, Surface::Popup).glazed(frame)
    }

    pub(crate) fn submenu(self) -> egui::menu::SubMenu {
        egui::menu::SubMenu::new()
    }

    pub fn menu<'a>(self, response: &egui::Response) -> egui::Popup<'a> {
        egui::Popup::menu(response)
    }

    pub fn context_menu<'a>(self, response: &egui::Response) -> egui::Popup<'a> {
        egui::Popup::context_menu(response)
    }

    pub(crate) fn tooltip_frame(self, ctx: &Context) -> Frame {
        let t = Tokens::get();
        self.frame(ctx, t.dialog_body, crate::frame::Side::Right)
            .stroke(t.primary_border(t.dialog_border))
            .inner_margin(egui::Margin::symmetric(t.space_8 as i8, t.space_4 as i8))
    }

    pub fn modal(self, id: Id) -> egui::Modal {
        egui::Modal::new(id).area(egui::Modal::default_area(id).fade_in(false))
    }

    pub fn central(self) -> egui::CentralPanel {
        egui::CentralPanel::default()
    }

    fn input_enabled(self) -> bool {
        self.surface == Surface::InputShield || !crate::issue_report::cad_input_blocked()
    }
}
