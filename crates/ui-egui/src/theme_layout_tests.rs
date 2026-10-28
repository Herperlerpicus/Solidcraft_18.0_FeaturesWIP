use super::*;
use serde_json::json;

fn layout(v: Value) -> BTreeMap<String, Value> {
    v.as_object().map(|o| o.iter().map(|(k, v)| (k.clone(), v.clone())).collect()).unwrap_or_default()
}

fn theme(layout: Value) -> Value {
    json!({"format": crate::theme_files::FORMAT, "name": "Arranged", "base": "light", "layout": layout})
}

#[test]
fn sections_read_strictly_and_unknown_ones_are_kept_for_a_newer_version() {
    let read = Layout::parse(&layout(json!({
        "ribbon": {"panels": {"MODEL/CREATE": ["solid.extrude", "solid.revolve"]}, "galleries": {}},
        "preferences": {"ribbon_labels": false, "dialog_place": "left_column", "future_setting": 3},
        "icons": {"remap": {"fillet": "chamfer", "extrude": "an_icon_from_the_future"}},
        "toolbars": {"top": []}
    })))
    .unwrap();
    assert_eq!(read.ribbon.as_ref().and_then(|r| r.panel("MODEL", "CREATE")), Some(&vec!["solid.extrude".to_string(), "solid.revolve".to_string()]));
    let values = &read.preferences.as_ref().unwrap().values;
    assert_eq!((values.get("ribbon_labels"), values.len()), (Some(&json!(false)), 2));
    assert_eq!(read.icons.as_ref().unwrap().remap.get("fillet").map(String::as_str), Some("chamfer"));
    let mut unknown = read.unknown.clone();
    unknown.sort();
    assert_eq!(unknown, ["icons.remap.extrude", "preferences.future_setting", "ribbon.galleries", "toolbars"]);
    assert_eq!(SECTIONS, ["ribbon", "preferences", "icons", "tabs"]);

    // A theme file keeps the whole object, sections this version doesn't read included.
    let file = crate::theme_files::from_value(&theme(json!({"toolbars": {"top": ["x"]}, "ribbon": {"panels": {}}}))).unwrap();
    assert_eq!(file.layout.get("toolbars"), Some(&json!({"top": ["x"]})));
    assert_eq!(serde_json::from_value::<crate::theme_files::ThemeFile>(serde_json::to_value(&file).unwrap()).unwrap(), file);

    for bad in [
        json!([]),
        json!({"ribbon": []}),
        json!({"ribbon": {"panels": {"CREATE": ["solid.extrude"]}}}),
        json!({"ribbon": {"panels": {"MODEL/": ["solid.extrude"]}}}),
        json!({"ribbon": {"panels": {"MODEL/CREATE": "solid.extrude"}}}),
        json!({"ribbon": {"panels": {"MODEL/CREATE": [1]}}}),
        json!({"ribbon": {"panels": {"MODEL/CREATE": [" "]}}}),
        json!({"ribbon": {"panels": {"MODEL/CREATE": vec!["solid.extrude"; MAX_ITEMS + 1]}}}),
        json!({"ribbon": {"panels": {"MODEL/CREATE": ["x".repeat(MAX_ID + 1)]}}}),
        json!({"preferences": {"ribbon_labels": "no"}}),
        json!({"preferences": {"layout": "sideways"}}),
        json!({"preferences": {"navigation_bar": {"position": 7}}}),
        json!({"icons": {"remap": {"fillet": 3}}}),
        json!({"icons": {"remap": []}}),
    ] {
        let source = theme(bad.clone());
        assert!(crate::theme_files::from_value(&source).is_err(), "{bad}");
    }
    let many: Map<String, Value> = (0..=MAX_SECTIONS).map(|i| (format!("s{i}"), json!({}))).collect();
    assert!(crate::theme_files::from_value(&theme(Value::Object(many))).is_err());
}

#[test]
fn a_theme_arranges_ribbon_panels_under_the_users_own_lists() {
    let mut h = crate::scenario::Harness::new();
    let lists = crate::toolbar_custom::panel_lists("MODEL");
    let (panel, cmds, n) = lists.iter().find(|(_, cmds, n)| cmds.len() >= 3 && *n >= 2).cloned().unwrap();
    let ids = |app: &crate::SolveApp| crate::toolbar_custom::buttons(app, "MODEL", panel, &cmds, n).iter().map(|c| c.id).collect::<Vec<_>>();
    let defaults = ids(&h.app);
    // The theme's panel: the third command, then the first, and one this version doesn't have.
    let themed = vec![cmds[2].id, cmds[0].id];
    let file = theme(json!({"ribbon": {"panels": {format!("MODEL/{panel}"): [cmds[2].id, cmds[0].id, "solid.from_the_future"]}}}));
    let out = h.call("ui.prefs", json!({"theme_import": file}));
    assert_eq!(out["result"]["theme_imported"], "arranged", "{out}");
    h.frames(2);
    assert_eq!(ids(&h.app), themed, "the theme's buttons, without the unknown command");

    // The user's own changes sit on top; Reset Toolbar goes back to the theme's, not the defaults.
    crate::toolbar_custom::remove(&mut h.app, "MODEL", cmds[0].id);
    assert_eq!(ids(&h.app), vec![cmds[2].id]);
    crate::toolbar_custom::reset(&mut h.app, "MODEL");
    assert_eq!(ids(&h.app), themed);

    // Another theme: the panel's own buttons again.
    h.call("ui.prefs", json!({"theme": "light"}));
    h.frames(2);
    assert_eq!(ids(&h.app), defaults);
}

#[test]
fn theme_preferences_are_set_once_and_the_users_own_come_back_after() {
    let mut h = crate::scenario::Harness::new();
    h.call("ui.prefs", json!({"ribbon_labels": true, "dialog_place": "edge"}));
    let file = theme(json!({"preferences": {"ribbon_labels": false, "dialog_place": "left_column"}}));
    h.call("ui.prefs", json!({"theme_import": file}));
    h.frames(2);
    let place = |h: &crate::scenario::Harness| serde_json::to_value(h.app.preferences.dialog_place).unwrap();
    assert!(!h.app.preferences.ribbon_labels);
    assert_eq!(place(&h), json!("left_column"));

    // The user may change them while the theme is on; it doesn't set them again.
    h.call("ui.prefs", json!({"ribbon_labels": true}));
    h.frames(3);
    assert!(h.app.preferences.ribbon_labels, "set once, not every frame or start");

    // Turning the theme off gives back the user's values from before it.
    h.call("ui.prefs", json!({"ribbon_labels": false, "theme": "dark"}));
    h.frames(2);
    assert!(h.app.preferences.ribbon_labels);
    assert_eq!(place(&h), json!("edge"));
    assert!(h.app.preferences.theme_saved.is_empty());

    // A theme without preferences changes none.
    h.call("ui.prefs", json!({"theme": "gruvbox-dark"}));
    h.frames(2);
    assert!(h.app.preferences.ribbon_labels);
}

#[test]
fn a_theme_setting_part_of_the_navigation_bar_keeps_the_rest_of_the_users() {
    let mut h = crate::scenario::Harness::new();
    h.call("ui.prefs", json!({"navigation_bar": {"slots": ["fit", "home"], "idle": "fade"}}));
    let own = h.app.preferences.navigation_bar.clone();
    h.call("ui.prefs", json!({"theme_import": theme(json!({"preferences": {"navigation_bar": {"idle": "always"}}}))}));
    h.frames(2);
    let bar = &h.app.preferences.navigation_bar;
    assert_eq!((bar.idle, &bar.slots), (crate::navigation_bar::Idle::Always, &own.slots), "the theme's idle, the user's buttons");
    h.call("ui.prefs", json!({"theme": "dark"}));
    h.frames(2);
    assert_eq!(h.app.preferences.navigation_bar, own);
}

#[test]
fn built_in_theme_layouts_read_completely() {
    for file in crate::theme_files::builtin() {
        let read = Layout::parse(&file.layout).unwrap();
        assert!(read.unknown.is_empty(), "{}: {:?}", file.id, read.unknown);
    }
    let aero = crate::theme_files::find("frutiger-aero", &[]).map(|f| Layout::parse(&f.layout).unwrap()).unwrap();
    assert!(aero.preferences.is_some_and(|p| p.values.contains_key("navigation_bar")), "a taskbar-like navigation bar");
}

#[test]
fn icon_packs_read_only_the_masters_svg_subset_within_caps() {
    let square = r##"<svg viewBox="0 0 64 64"><path d="M4 4 L60 4 L60 60 L4 60 Z" fill="#328bd3"/></svg>"##;
    let read = Layout::parse(&layout(json!({"icons": {"pack": {"extrude": square}, "remap": {"extrude": "chamfer"}}}))).unwrap();
    assert!(read.icons.as_ref().is_some_and(|icons| icons.pack.contains_key("extrude")));
    set_current(Some(&layout(json!({"icons": {"pack": {"extrude": square}}}))));
    assert!(pack("extrude").is_some() && pack("fillet").is_none());
    set_current(None);
    assert!(pack("extrude").is_none());

    let nested = format!("<svg viewBox=\"0 0 64 64\">{}{}</svg>", "<g>".repeat(40), "</g>".repeat(40));
    let big = format!("<svg viewBox=\"0 0 64 64\"><path d=\"M0 0 {}Z\" fill=\"#328bd3\"/></svg>", "L1 1 ".repeat(20_000));
    for bad in [
        json!({"icons": {"pack": {"extrude": 5}}}),
        json!({"icons": {"pack": []}}),
        json!({"icons": {"pack": {"extrude": "<svg viewBox=\"0 0 64 64\"><path d=\"M0 0 L9 9 Z\" fill=\"#ff0000\"/></svg>"}}}),
        json!({"icons": {"pack": {"extrude": "<svg"}}}),
        json!({"icons": {"pack": {"extrude": nested}}}),
        json!({"icons": {"pack": {"extrude": big}}}),
    ] {
        assert!(Layout::parse(&layout(bad.clone())).is_err(), "{}", bad.to_string().chars().take(120).collect::<String>());
    }
    // Built-in themes draw SolidCraft's reviewed icons only.
    for file in crate::theme_files::builtin() {
        assert!(!Layout::parse(&file.layout).unwrap().icons.is_some_and(|icons| !icons.pack.is_empty()), "{}", file.id);
    }
}

#[test]
fn themes_travel_as_packages_with_their_icons_as_files() {
    use std::io::{Read, Write};
    let square = r##"<svg viewBox="0 0 64 64"><path d="M4 4 L60 4 L60 60 L4 60 Z" fill="#328bd3"/></svg>"##;
    let source = json!({"format": crate::theme_files::FORMAT, "name": "Packed", "base": "light",
        "colors": {"panel": "#eef8fe"},
        "layout": {"icons": {"pack": {"extrude": square}}, "tabs": {"colors": {"SKETCH": "#2e9e3e"}}}});
    let bytes = crate::theme_files::package(&source).unwrap();
    assert!(crate::theme_files::is_package(&bytes));
    // The pack's drawings are files of their own, and the theme file has none left.
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(&bytes[..])).unwrap();
    let mut svg = String::new();
    archive.by_name("icons/extrude.svg").unwrap().read_to_string(&mut svg).unwrap();
    assert_eq!(svg, square);
    let mut text = String::new();
    archive.by_name("theme.json").unwrap().read_to_string(&mut text).unwrap();
    assert!(!text.contains("<svg"), "{text}");
    // It reads back as the same theme.
    assert_eq!(crate::theme_files::from_package(&bytes).unwrap(), crate::theme_files::from_value(&source).unwrap());

    // Made by hand: a theme file in a folder, its drawings beside it, other files ignored.
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    for (name, body) in [("Packed/theme.json", text.as_str()), ("Packed/icons/extrude.svg", square), ("Packed/readme.txt", "hello")] {
        zip.start_file(name, options).unwrap();
        zip.write_all(body.as_bytes()).unwrap();
    }
    let by_hand = zip.finish().unwrap().into_inner();
    assert_eq!(crate::theme_files::from_package(&by_hand).unwrap(), crate::theme_files::from_value(&source).unwrap());

    // Installed through the app, it is on.
    let mut h = crate::scenario::Harness::new();
    let id = crate::prefs::install_theme_bytes(&mut h.app, &bytes).unwrap();
    assert_eq!((id.as_str(), h.app.preferences.theme.as_str()), ("packed", "packed"));

    // Hostile packages are refused, never read past their caps.
    let package = |files: &[(&str, String)]| {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        for (name, body) in files {
            zip.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
            zip.write_all(body.as_bytes()).unwrap();
        }
        zip.finish().unwrap().into_inner()
    };
    for bad in [
        package(&[("icons/extrude.svg", square.to_string())]),
        package(&[("theme.json", text.clone()), ("icons/extrude.svg", "x".repeat(crate::icon_svg::MAX_PACK_SVG + 1))]),
        package(&[("theme.json", "{".to_string())]),
        package(&[("theme.json", text.clone()), ("icons/extrude.svg", "<svg".to_string())]),
        b"PK\x03\x04 not really a zip".to_vec(),
    ] {
        assert!(crate::theme_files::from_package(&bad).is_err());
    }
}

#[test]
fn out_of_the_box_the_look_is_the_original_dark_theme() {
    let mut h = crate::scenario::Harness::new();
    h.frames(3);
    let p = &h.app.preferences;
    assert_eq!(p.theme, "dark");
    // None of the enhanced theme options is on.
    assert!(p.ui_color.is_empty() && p.tab_colors.is_empty() && !p.tab_colored_icons && !p.tab_tint);
    assert!(p.background_top.is_empty() && p.background_bottom.is_empty());
    assert_eq!(p.background_kind, crate::backdrop::Kind::Gradient);
    assert!(p.glass_opacity.is_none() && p.glass_gloss.is_none() && p.glass_menu_opacity.is_none() && p.glass_menu_shadow.is_none());
    // The window draws exactly the original Dark palette: no glass, no layout, no moving light.
    assert_eq!(crate::theme::Tokens::get(), crate::theme::Tokens::dark());
    assert!(crate::theme::Tokens::get().borders.glass.is_none());
    assert!(with(|layout| *layout == Layout::default()));
    assert_eq!(crate::theme::light(), egui::Vec2::ZERO);
}

#[test]
fn a_theme_draws_one_icon_as_another_in_one_step() {
    let plain = crate::icon_svg::resolve("chamfer").master;
    assert!(crate::icon_svg::exists("fillet") && crate::icon_svg::exists("chamfer"));
    assert!(!crate::icon_svg::exists("an_icon_from_the_future"));
    let remap = layout(json!({"icons": {"remap": {"fillet": "chamfer", "chamfer": "fillet"}}}));
    set_current(Some(&remap));
    assert_eq!(crate::icon_svg::resolve("fillet").master, plain, "drawn as the chamfer");
    assert_ne!(crate::icon_svg::resolve("chamfer").master, plain, "a remap is not followed again");
    assert!(!set_current(Some(&remap)), "unchanged: not read again");
    assert!(set_current(None));
    assert_eq!(crate::icon_svg::resolve("chamfer").master, plain);
    assert_eq!(icon("fillet"), None);
}
