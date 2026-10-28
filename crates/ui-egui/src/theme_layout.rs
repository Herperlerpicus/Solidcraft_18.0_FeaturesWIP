//! What a theme arranges besides its colours: the optional `layout` object of a
//! `solidcraft-theme/1` file (`theme_files`), made of named sections. Each section is one row of
//! `sections!` below: a field, its type, and that type's [`Section::parse`], a strict reading of
//! hostile input. A part of the UI that becomes themeable adds a section (a type and a row); the
//! file format, import, export and storage need no change.
//!
//! Forward compatible: a section, or a setting inside one, that this version doesn't know is kept
//! in the file (exporting it loses nothing) and listed in [`Layout::unknown`], never refused, so a
//! theme written for a newer SolidCraft still imports into this one. A known setting with a bad
//! value is refused, as everywhere in theme files.
//!
//! The layout of the theme on now belongs to the UI thread ([`with`]); `prefs::apply` sets it
//! with the theme's colours, and the parts of the UI a section arranges read it from there:
//!
//! - `ribbon`: the buttons of ribbon panels (`toolbar_custom`), under the user's own lists.
//! - `preferences`: values for the preferences in [`THEMEABLE`] (where panels and dialogs go,
//!   the navigation bar, ribbon labels), set when the theme is turned on; the user's own values
//!   come back when it is turned off (`prefs::apply_theme_preferences`).
//! - `icons`: another SolidCraft icon for an icon id (`icon_svg::resolve`), or the theme's own
//!   drawing from its pack (`icons::paint_resolved`).
//! - `tabs`: the theme's own colours for ribbon tabs, under the user's (`prefs::apply`).
//!
//! ```json
//! "layout": {
//!   "ribbon": {"panels": {"MODEL/CREATE": ["solid.extrude", "solid.revolve", "solid.hole"]}},
//!   "preferences": {"ribbon_labels": false, "dialog_place": "left_column"},
//!   "icons": {"remap": {"fillet": "chamfer"}}
//! }
//! ```

use std::cell::Cell;
use std::collections::BTreeMap;
use std::rc::Rc;

use serde_json::{Map, Value};

/// Hostile-input caps: sections, entries in one map, ids in one list, characters in one id.
const MAX_SECTIONS: usize = 32;
const MAX_ENTRIES: usize = 512;
const MAX_ITEMS: usize = 100;
const MAX_ID: usize = 120;

/// One section of a theme's layout.
pub trait Section: Sized {
    /// Read the section, refusing bad values. A setting this version doesn't know goes onto
    /// `unknown` (its path inside the section) and is otherwise skipped.
    fn parse(value: &Value, unknown: &mut Vec<String>) -> Result<Self, String>;
}

macro_rules! sections {
    ($($(#[$doc:meta])* $field:ident: $ty:ty),* $(,)?) => {
        /// A theme's layout, read. A section the theme leaves out is `None`.
        #[derive(Clone, Debug, Default, PartialEq)]
        pub struct Layout {
            $($(#[$doc])* pub $field: Option<$ty>,)*
            /// Sections and settings this version doesn't know (`toolbars`, `ribbon.galleries`…).
            pub unknown: Vec<String>,
        }

        /// The section keys this version reads.
        pub const SECTIONS: &[&str] = &[$(stringify!($field)),*];

        impl Layout {
            /// Read a theme file's `layout` object.
            pub fn parse(layout: &BTreeMap<String, Value>) -> Result<Layout, String> {
                if layout.len() > MAX_SECTIONS {
                    return Err(format!("`layout` has more than {MAX_SECTIONS} sections"));
                }
                let mut out = Layout::default();
                for (key, value) in layout {
                    let mut unknown = Vec::new();
                    match key.as_str() {
                        $(stringify!($field) => {
                            out.$field = Some(<$ty>::parse(value, &mut unknown).map_err(|e| format!("`layout.{key}`: {e}"))?);
                            out.unknown.extend(unknown.into_iter().map(|path| format!("{key}.{path}")));
                        })*
                        _ => out.unknown.push(key.clone()),
                    }
                }
                Ok(out)
            }
        }
    };
}

sections! {
    /// The buttons of ribbon panels.
    ribbon: Ribbon,
    /// Preference values set while the theme is on.
    preferences: Preferences,
    /// Other icons for icon ids.
    icons: Icons,
    /// Ribbon tabs' own colours.
    tabs: Tabs,
}

fn object<'a>(value: &'a Value, what: &str) -> Result<&'a Map<String, Value>, String> {
    let object = value.as_object().ok_or_else(|| format!("{what} must be an object"))?;
    if object.len() > MAX_ENTRIES {
        return Err(format!("{what} has more than {MAX_ENTRIES} entries"));
    }
    Ok(object)
}

fn id(value: &str) -> bool {
    !value.trim().is_empty() && value.chars().count() <= MAX_ID && !value.chars().any(char::is_control)
}

fn id_list(value: &Value) -> Result<Vec<String>, String> {
    let items = value.as_array().filter(|items| items.len() <= MAX_ITEMS);
    let ids = items.map(|items| items.iter().map(|item| item.as_str().filter(|s| id(s)).map(str::to_string)).collect::<Option<Vec<_>>>());
    ids.flatten().ok_or_else(|| format!("must be a list of at most {MAX_ITEMS} ids"))
}

/// `ribbon`: `{"panels": {"<TAB>/<PANEL>": ["command.id", …]}}`, a panel's buttons in order, for
/// example `"MODEL/CREATE"`. A panel the theme leaves out keeps its own buttons; the user's own
/// list for a panel (Customize Toolbar) wins over the theme's, and Reset Toolbar returns to the
/// theme's. A command this version doesn't have is skipped where it is drawn.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Ribbon {
    pub panels: BTreeMap<String, Vec<String>>,
}

impl Ribbon {
    /// The theme's buttons for a panel of a tab.
    pub fn panel(&self, tab: &str, panel: &str) -> Option<&Vec<String>> {
        self.panels.get(&format!("{tab}/{panel}"))
    }
}

impl Section for Ribbon {
    fn parse(value: &Value, unknown: &mut Vec<String>) -> Result<Self, String> {
        let mut ribbon = Ribbon::default();
        for (key, value) in object(value, "the section")? {
            if key != "panels" {
                unknown.push(key.clone());
                continue;
            }
            for (panel, ids) in object(value, "`panels`")? {
                let named = id(panel) && panel.split_once('/').is_some_and(|(tab, name)| !tab.is_empty() && !name.is_empty());
                if !named {
                    return Err("`panels` keys must be named TAB/PANEL, such as MODEL/CREATE".into());
                }
                ribbon.panels.insert(panel.clone(), id_list(ids).map_err(|e| format!("`panels.{panel}` {e}"))?);
            }
        }
        Ok(ribbon)
    }
}

/// The preferences a theme may set: how the workspace is laid out (floating or docked), where
/// dialogs open, the navigation bar (its buttons, place, size and labels), the ribbon's labels
/// and whether icons follow their tab's colour, whether colours follow the active tab or the time
/// of day, and the background. A preference that becomes themeable joins this list; its value is checked as the
/// preferences check it.
pub const THEMEABLE: &[&str] = &[
    "layout",
    "dialog_place",
    "navigation_bar",
    "ribbon_labels",
    "tab_colored_icons",
    "tab_tint",
    "time_of_day",
    "background_kind",
    "background_pattern",
];

/// `preferences`: `{"<preference>": value}` for preferences in [`THEMEABLE`], in the form the
/// preferences file and `ui.prefs` use. A preference this version doesn't know, or doesn't let a
/// theme set, is skipped.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Preferences {
    pub values: BTreeMap<String, Value>,
}

impl Section for Preferences {
    fn parse(value: &Value, unknown: &mut Vec<String>) -> Result<Self, String> {
        let defaults = serde_json::to_value(crate::prefs::Prefs::default()).map_err(|e| e.to_string())?;
        let mut values = BTreeMap::new();
        for (key, value) in object(value, "the section")? {
            if !THEMEABLE.contains(&key.as_str()) {
                unknown.push(key.clone());
                continue;
            }
            if key == "navigation_bar" {
                crate::navigation_bar::validate(value)?;
            }
            // Read as the preferences file is: a value of the wrong kind doesn't read.
            let mut probe = defaults.clone();
            if let Some(fields) = probe.as_object_mut() {
                fields.insert(key.clone(), value.clone());
            }
            if serde_json::from_value::<crate::prefs::Prefs>(probe).is_err() {
                return Err(format!("`{key}` is not a value that preference takes"));
            }
            values.insert(key.clone(), value.clone());
        }
        Ok(Preferences { values })
    }
}

/// `icons`: what an icon id draws as wherever it is shown (ribbon, menus, browser, timeline).
///
/// - `remap`: `{"<icon id>": "<icon id>"}`, another SolidCraft icon (a master or an alias). An
///   icon this version doesn't have is skipped.
/// - `pack`: `{"<icon id>": "<svg>…</svg>"}`, the theme's own drawing, in the masters' SVG
///   subset and palette colours (`icon_svg::parse_untrusted`), so the theme's icon palette and
///   colour vision modes still colour it. It wins over a remap of the same id. Built-in themes
///   use SolidCraft's reviewed icons only; packs come with imported themes, the user's own or
///   openly licensed artwork.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Icons {
    pub remap: BTreeMap<String, String>,
    pub pack: BTreeMap<String, PackIcon>,
}

/// One drawing of a theme's icon pack, read; `key` tells drawings apart (textures, equality).
#[derive(Clone, Debug)]
pub struct PackIcon {
    pub key: u64,
    pub doc: Rc<crate::icon_svg::Doc>,
}

impl PartialEq for PackIcon {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl Section for Icons {
    fn parse(value: &Value, unknown: &mut Vec<String>) -> Result<Self, String> {
        let mut icons = Icons::default();
        for (key, value) in object(value, "the section")? {
            match key.as_str() {
                "remap" => {
                    for (from, to) in object(value, "`remap`")? {
                        let Some(to) = value_id(to).filter(|_| id(from)) else {
                            return Err("`remap` must map icon ids to icon ids".into());
                        };
                        if crate::icon_svg::exists(to) {
                            icons.remap.insert(from.clone(), to.to_string());
                        } else {
                            unknown.push(format!("remap.{from}"));
                        }
                    }
                }
                "pack" => {
                    for (name, svg) in object(value, "`pack`")? {
                        let Some(svg) = svg.as_str().filter(|_| id(name)) else {
                            return Err("`pack` must map icon ids to SVG drawings".into());
                        };
                        let doc = crate::icon_svg::parse_untrusted(svg).map_err(|e| format!("`pack.{name}` {e}"))?;
                        icons.pack.insert(name.clone(), PackIcon { key: content_key(svg), doc: Rc::new(doc) });
                    }
                }
                _ => unknown.push(key.clone()),
            }
        }
        Ok(icons)
    }
}

/// A key for a drawing's text, the same for the same text within a run.
fn content_key(text: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

/// `tabs`: `{"colors": {"<TAB>": "#rrggbb"}}`, the theme's own colour for ribbon tabs
/// (`theme::COLOURED_TABS`): their pills, lines, soft selections and, with icons following their
/// tab, icons. The user's own tab colours win over the theme's. A tab this version doesn't have
/// is skipped.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tabs {
    pub colors: BTreeMap<String, egui::Color32>,
}

impl Section for Tabs {
    fn parse(value: &Value, unknown: &mut Vec<String>) -> Result<Self, String> {
        let mut tabs = Tabs::default();
        for (key, value) in object(value, "the section")? {
            if key != "colors" {
                unknown.push(key.clone());
                continue;
            }
            for (tab, color) in object(value, "`colors`")? {
                let Some(color) = color.as_str().and_then(crate::theme_files::color) else {
                    return Err(format!("`colors.{}` must be #rrggbb", tab.chars().take(MAX_ID).collect::<String>()));
                };
                if crate::theme::COLOURED_TABS.iter().any(|(id, _)| id == tab) {
                    tabs.colors.insert(tab.clone(), color);
                } else {
                    unknown.push(format!("colors.{tab}"));
                }
            }
        }
        Ok(tabs)
    }
}

fn value_id(value: &Value) -> Option<&str> {
    value.as_str().filter(|s| id(s))
}

thread_local! {
    /// The layout of the theme on now and the `layout` object it was read from, per thread like
    /// the theme's colours (`theme::CUSTOM`). `None`: no theme layout.
    static CURRENT: Cell<Option<Rc<(BTreeMap<String, Value>, Layout)>>> = const { Cell::new(None) };
}

/// Run `read` with the layout of the theme on now (empty when it has none), without copying it.
pub fn with<R>(read: impl FnOnce(&Layout) -> R) -> R {
    let held = CURRENT.take();
    let out = match &held {
        Some(held) => read(&held.1),
        None => read(&Layout::default()),
    };
    CURRENT.set(held);
    out
}

/// Use a theme's `layout` object (`None`: the theme on has none). It is read again only when it
/// changes; one that doesn't read leaves no layout (it was checked when imported, so only a hand
/// edit of saved preferences gets here). Returns whether anything changed.
pub fn set_current(layout: Option<&BTreeMap<String, Value>>) -> bool {
    let held = CURRENT.take();
    let same = match (&held, layout) {
        (Some(held), Some(layout)) => held.0 == *layout,
        (None, None) => true,
        (None, Some(layout)) => layout.is_empty(),
        (Some(_), None) => false,
    };
    if same {
        CURRENT.set(held);
        return false;
    }
    let read = layout.filter(|layout| !layout.is_empty()).map(|layout| {
        let parsed = Layout::parse(layout).unwrap_or_else(|e| {
            log::warn!("theme layout: {e}; using none");
            Layout::default()
        });
        Rc::new((layout.clone(), parsed))
    });
    CURRENT.set(read);
    true
}

/// The icon a theme draws for an icon id, if it remaps it.
pub fn icon(id: &str) -> Option<String> {
    with(|layout| layout.icons.as_ref().and_then(|icons| icons.remap.get(id).cloned()))
}

/// The theme's own drawing for an icon id, if its pack has one.
pub fn pack(id: &str) -> Option<PackIcon> {
    with(|layout| layout.icons.as_ref().and_then(|icons| icons.pack.get(id).cloned()))
}

#[cfg(test)]
#[path = "theme_layout_tests.rs"]
mod tests;
