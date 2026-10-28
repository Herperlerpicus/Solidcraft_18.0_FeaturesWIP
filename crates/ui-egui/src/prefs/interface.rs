//! Interface settings composition, using shared controls and previews.
use super::*;

/// Origin plane names: each choice shows the three names it gives (`XY / XZ / YZ`), with a note
/// that only the names shown change.
fn origin_plane_names(app: &mut SolveApp, ui: &mut egui::Ui) {
    ui_kit::settings_section(ui, "origin_plane_names", app.tr("prefs.origin_plane_names"), |ui| {
        let choices: Vec<String> =
            OriginPlaneNames::ALL.iter().map(|names| OriginPlaneNames::PLANES.map(|plane| names.label(plane)).join(" / ")).collect();
        let options: Vec<&str> = choices.iter().map(String::as_str).collect();
        let mut selected = OriginPlaneNames::ALL.iter().position(|names| *names == app.preferences.origin_plane_names).unwrap_or(0);
        if ui_kit::settings_segments(ui, app.tr("prefs.origin_plane_names"), &mut selected, &options)
            && let Some(names) = OriginPlaneNames::ALL.get(selected)
        {
            app.preferences.origin_plane_names = *names;
        }
        // Handles that don't depend on the language, by the choice's saved value.
        for (choice, key) in choices.iter().zip(["xyz", "top_front_right"]) {
            if let Some(at) = crate::scenario::handle_at(&format!("dialog:{}:{choice}", app.tr("prefs.origin_plane_names"))) {
                crate::scenario::publish_handle(&format!("prefs:origin_plane_names:{key}"), at);
            }
        }
        let top = OriginPlaneNames::TopFrontRight;
        let note =
            crate::i18n::format("prefs.origin_plane_names_note", &[("top", top.label("XY")), ("front", top.label("XZ")), ("right", top.label("YZ"))]);
        ui_kit::text(ui, &note, ui_kit::Text::Hint("origin_plane_names"));
    });
}

/// The colours of a day from midnight to midnight, as the interface takes them following the time
/// of day, with a mark where the day is now.
fn day_strip(ui: &mut egui::Ui, t: &Tokens) {
    let width = ui.available_width().max(t.space_16);
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, t.space_10), egui::Sense::hover());
    const STEPS: usize = 96;
    let step = rect.width() / STEPS as f32;
    for i in 0..STEPS {
        let left = rect.left() + step * i as f32;
        let part = egui::Rect::from_min_max(egui::pos2(left, rect.top()), egui::pos2(left + step + 0.5, rect.bottom()));
        ui.painter().rect_filled(part, 0.0, crate::theme::daylight((i as f32 + 0.5) * 24.0 / STEPS as f32));
    }
    let now = local_hours();
    let x = rect.left() + rect.width() * now / 24.0;
    ui.painter().line_segment([egui::pos2(x, rect.top() - 2.0), egui::pos2(x, rect.bottom() + 2.0)], egui::Stroke::new(2.0, t.text));
    crate::scenario::publish_target("prefs:time_of_day_strip", rect);
    response.on_hover_text(format!("{:02}:{:02}", now as u32, (now.fract() * 60.0) as u32));
}

/// The user's own base colour over the theme, how strongly it tints, and a glass theme's
/// opacity and gloss (`theme::Personal`). Each can go back to the theme's own.
fn colour_and_glass(app: &mut SolveApp, ui: &mut egui::Ui, t: &Tokens) {
    let percent = |v: f64| format!("{:.0} %", v * 100.0);
    ui_kit::settings_section(ui, "colour_and_glass", app.tr("prefs.colour_and_glass"), |ui| {
        ui_kit::settings_form(ui, t.form_label_width + t.space_16 + t.space_6, |ui| {
            // Following the time of day, the day's colour is the base colour.
            ui_kit::row(ui, app.tr("prefs.base_colour"), |ui| {
                ui.add_enabled_ui(!app.preferences.time_of_day, |ui| {
                    ui.horizontal(|ui| {
                        let shown = crate::theme_files::color(&app.preferences.ui_color).unwrap_or(t.app_bar);
                        let mut rgb = [shown.r(), shown.g(), shown.b()];
                        if ui_kit::color_field(ui, &mut rgb).changed() {
                            app.preferences.ui_color = format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2]);
                        }
                        let own = app.preferences.ui_color.is_empty();
                        if ui_kit::button(ui, app.tr("prefs.theme_own"), ui_kit::Button::Enabled(!own)).clicked() {
                            app.preferences.ui_color.clear();
                        }
                    });
                });
            });
            // The day's colours, with where the day is now, under the choice.
            ui_kit::row(ui, "", |ui| {
                let check = ui_kit::checkbox(ui, app.tr("prefs.time_of_day"), &mut app.preferences.time_of_day);
                crate::scenario::publish_target("prefs:time_of_day", check.rect);
            });
            if app.preferences.time_of_day {
                ui_kit::row(ui, "", |ui| day_strip(ui, t));
            }
            ui_kit::row(ui, "", |ui| {
                ui_kit::checkbox(ui, app.tr("prefs.tab_tint"), &mut app.preferences.tab_tint);
            });
            // The light and the background's pattern drift with the pointer.
            ui_kit::row(ui, "", |ui| {
                ui_kit::checkbox(ui, app.tr("prefs.background_motion"), &mut app.preferences.background_motion);
            });
            // What the background is: the gradient, a solid colour, a pattern or a picture.
            ui_kit::row(ui, app.tr("prefs.background"), |ui| {
                let kinds = crate::backdrop::Kind::ALL;
                let labels: Vec<&str> = kinds.iter().map(|k| app.tr(k.label_id())).collect();
                let mut selected = kinds.iter().position(|k| *k == app.preferences.background_kind).unwrap_or(0);
                if ui_kit::segmented(ui, &mut selected, &labels, Default::default())
                    && let Some(kind) = kinds.get(selected)
                {
                    app.preferences.background_kind = *kind;
                }
            });
            match app.preferences.background_kind {
                crate::backdrop::Kind::Pattern => {
                    ui_kit::row(ui, app.tr("prefs.background_pattern"), |ui| {
                        let labels: Vec<&str> = crate::backdrop::PATTERNS.iter().map(|(_, label)| app.tr(label)).collect();
                        let mut selected =
                            crate::backdrop::PATTERNS.iter().position(|(id, _)| *id == app.preferences.background_pattern).unwrap_or(0);
                        if ui_kit::segmented(ui, &mut selected, &labels, Default::default())
                            && let Some((id, _)) = crate::backdrop::PATTERNS.get(selected)
                        {
                            app.preferences.background_pattern = id.to_string();
                        }
                    });
                }
                crate::backdrop::Kind::Image => {
                    ui_kit::row(ui, app.tr("prefs.background_image"), |ui| {
                        ui.horizontal(|ui| {
                            if ui_kit::button(ui, app.tr("prefs.choose_image"), ui_kit::Button::Small).clicked() {
                                app.pick_open_for(&["png", "jpg", "jpeg"], |app, path| {
                                    if let Some(path) = path {
                                        app.preferences.background_image = path;
                                    }
                                });
                            }
                            let name = std::path::Path::new(&app.preferences.background_image)
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_else(|| app.tr("prefs.no_image").to_string());
                            ui_kit::text(ui, &name, ui_kit::Text::Hint("background_image"));
                        });
                    });
                }
                _ => {}
            }
            // The 3D view's background gradient: the user's own (the theme's until set); following
            // the active tab, a gradient of its colour as light as this.
            for (label, top) in [(app.tr("prefs.background_top"), true), (app.tr("prefs.background_bottom"), false)] {
                ui_kit::row(ui, label, |ui| {
                    let shown = if top { t.viewport_top } else { t.viewport_bottom };
                    let mut rgb = [shown.r(), shown.g(), shown.b()];
                    if ui_kit::color_field(ui, &mut rgb).changed() {
                        let hex = |c: [u8; 3]| format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]);
                        // Both ends are set together, the other one as it shows now.
                        let (other, other_shown) = if top {
                            (&mut app.preferences.background_bottom, t.viewport_bottom)
                        } else {
                            (&mut app.preferences.background_top, t.viewport_top)
                        };
                        if other.is_empty() {
                            *other = hex([other_shown.r(), other_shown.g(), other_shown.b()]);
                        }
                        if top {
                            app.preferences.background_top = hex(rgb);
                        } else {
                            app.preferences.background_bottom = hex(rgb);
                        }
                    }
                });
            }
            ui_kit::row(ui, "", |ui| {
                let own = app.preferences.background_top.is_empty() && app.preferences.background_bottom.is_empty();
                if ui_kit::button(ui, app.tr("prefs.background_own"), ui_kit::Button::Enabled(!own)).clicked() {
                    app.preferences.background_top.clear();
                    app.preferences.background_bottom.clear();
                }
            });
            // How strong the user's or the tab's background is, and whether strong colours are
            // evened out to the theme's own lightness.
            ui_kit::row(ui, app.tr("prefs.background_strength"), |ui| {
                let response = ui_kit::slider(
                    ui,
                    &mut app.preferences.background_strength,
                    0.0..=1.0,
                    ui_kit::Slider { step: Some(0.05), format: percent, ..Default::default() },
                );
                crate::scenario::publish_target("prefs:background_strength", response.rect);
            });
            ui_kit::row(ui, "", |ui| {
                ui_kit::checkbox(ui, app.tr("prefs.background_even"), &mut app.preferences.background_even);
            });
            ui_kit::row(ui, app.tr("prefs.colour_intensity"), |ui| {
                ui.add_enabled_ui(!app.preferences.ui_color.is_empty() || app.preferences.time_of_day, |ui| {
                    let response = ui_kit::slider(
                        ui,
                        &mut app.preferences.ui_color_intensity,
                        0.0..=1.0,
                        ui_kit::Slider { step: Some(0.05), format: percent, ..Default::default() },
                    );
                    crate::scenario::publish_target("prefs:ui_color_intensity", response.rect);
                });
            });
            // Each workspace tab's colour (its pill, line and soft selections), and whether the
            // icons on a tab follow it.
            let tokens = Tokens::get();
            for (id, name) in crate::theme::COLOURED_TABS {
                ui_kit::row(ui, crate::i18n::tr_label(name), |ui| {
                    let shown = crate::theme::TabAccent::for_tab(&tokens, id).color;
                    let mut rgb = [shown.r(), shown.g(), shown.b()];
                    if ui_kit::color_field(ui, &mut rgb).changed() {
                        app.preferences.tab_colors.insert(id.to_string(), format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2]));
                    }
                });
            }
            ui_kit::row(ui, "", |ui| {
                ui_kit::checkbox(ui, app.tr("prefs.tab_icons"), &mut app.preferences.tab_colored_icons);
            });
            ui_kit::row(ui, "", |ui| {
                if ui_kit::button(ui, app.tr("prefs.tab_colours_own"), ui_kit::Button::Enabled(!app.preferences.tab_colors.is_empty())).clicked() {
                    app.preferences.tab_colors.clear();
                }
            });
            // The theme's own glass, before the user's changes, is what the sliders start from.
            let theme_glass = crate::theme_files::find(&app.preferences.theme, &app.preferences.custom_themes)
                .and_then(|f| crate::theme_files::resolve(f).borders.glass);
            ui.add_enabled_ui(theme_glass.is_some(), |ui| {
                let shadow = theme_glass.and_then(|g| g.menu_shadow).unwrap_or(0.0);
                let (opacity, gloss, menus) =
                    theme_glass.map_or((1.0, 0.0, 1.0), |g| (g.opacity, f32::from(g.sheen.a()) / 255.0, g.menu_opacity.unwrap_or(g.opacity)));
                ui_kit::row(ui, app.tr("prefs.glass_opacity"), |ui| {
                    let mut value = app.preferences.glass_opacity.unwrap_or(opacity);
                    let response =
                        ui_kit::slider(ui, &mut value, 0.3..=1.0, ui_kit::Slider { step: Some(0.01), format: percent, ..Default::default() });
                    crate::scenario::publish_target("prefs:glass_opacity", response.rect);
                    if response.changed() {
                        app.preferences.glass_opacity = Some(value);
                    }
                });
                ui_kit::row(ui, app.tr("prefs.gloss"), |ui| {
                    let mut value = app.preferences.glass_gloss.unwrap_or(gloss);
                    let response =
                        ui_kit::slider(ui, &mut value, 0.0..=1.0, ui_kit::Slider { step: Some(0.01), format: percent, ..Default::default() });
                    crate::scenario::publish_target("prefs:glass_gloss", response.rect);
                    if response.changed() {
                        app.preferences.glass_gloss = Some(value);
                    }
                });
                // Menus and popups apart from panels, so they can stay readable over busy ones.
                ui_kit::row(ui, app.tr("prefs.menu_opacity"), |ui| {
                    let mut value = app.preferences.glass_menu_opacity.unwrap_or(menus);
                    let response =
                        ui_kit::slider(ui, &mut value, 0.3..=1.0, ui_kit::Slider { step: Some(0.01), format: percent, ..Default::default() });
                    crate::scenario::publish_target("prefs:glass_menu_opacity", response.rect);
                    if response.changed() {
                        app.preferences.glass_menu_opacity = Some(value);
                    }
                });
                // The shadow under menus, popups and notifications.
                ui_kit::row(ui, app.tr("prefs.menu_shadow"), |ui| {
                    let mut value = app.preferences.glass_menu_shadow.unwrap_or(shadow);
                    let response =
                        ui_kit::slider(ui, &mut value, 0.0..=1.0, ui_kit::Slider { step: Some(0.01), format: percent, ..Default::default() });
                    crate::scenario::publish_target("prefs:glass_menu_shadow", response.rect);
                    if response.changed() {
                        app.preferences.glass_menu_shadow = Some(value);
                    }
                });
                // The halo and shadow behind text on the glass, so it reads over what shows through.
                let text_shadow_default = Prefs::default().glass_text_shadow;
                ui_kit::row(ui, app.tr("prefs.text_shadow"), |ui| {
                    let response = ui_kit::slider(
                        ui,
                        &mut app.preferences.glass_text_shadow,
                        0.0..=1.0,
                        ui_kit::Slider { step: Some(0.05), format: percent, ..Default::default() },
                    );
                    crate::scenario::publish_target("prefs:glass_text_shadow", response.rect);
                });
                // One button puts the glass sliders back, under them, so the rows keep the page's width.
                let changed = app.preferences.glass_opacity.is_some()
                    || app.preferences.glass_gloss.is_some()
                    || app.preferences.glass_menu_opacity.is_some()
                    || app.preferences.glass_menu_shadow.is_some()
                    || app.preferences.glass_text_shadow != text_shadow_default;
                ui_kit::row(ui, "", |ui| {
                    if ui_kit::button(ui, app.tr("prefs.theme_own"), ui_kit::Button::Enabled(changed)).clicked() {
                        app.preferences.glass_opacity = None;
                        app.preferences.glass_gloss = None;
                        app.preferences.glass_menu_opacity = None;
                        app.preferences.glass_menu_shadow = None;
                        app.preferences.glass_text_shadow = text_shadow_default;
                    }
                });
            });
            if theme_glass.is_none() {
                ui_kit::text(ui, app.tr("prefs.glass_note"), ui_kit::Text::Hint("colour_and_glass"));
            }
        });
    });
}

pub(super) fn interface(app: &mut SolveApp, ui: &mut egui::Ui, t: &Tokens) {
    ui_kit::settings_heading(ui, app.tr("prefs.interface"), app.tr("prefs.interface_note"));
    ui_kit::settings_section(ui, "theme", app.tr("ui.theme"), |ui| {
        theme_row(app, ui);
        ui_kit::tab_accent_strip(ui);
        // The 3D view in the theme's colours, or the Light or Dark theme's (#4146).
        ui_kit::settings_form(ui, t.form_label_width + t.space_16 + t.space_6, |ui| {
            let label = app.tr("prefs.viewport");
            ui_kit::row(ui, label, |ui| {
                let views = crate::theme::ViewportTheme::ALL;
                let labels: Vec<_> = views.iter().map(|v| crate::i18n::tr(v.label_id())).collect();
                let mut selected = views.iter().position(|v| *v == app.preferences.viewport_theme).unwrap_or(0);
                if ui_kit::settings_segments(ui, label, &mut selected, &labels)
                    && let Some(view) = views.get(selected)
                {
                    app.preferences.viewport_theme = *view;
                }
            });
            // The logo in the theme's own logo colours or in its accent.
            let label = app.tr("prefs.logo");
            ui_kit::row(ui, label, |ui| {
                let labels = [crate::i18n::tr("prefs.logo_theme"), crate::i18n::tr("prefs.logo_accent")];
                let mut selected = usize::from(app.preferences.logo_accent);
                if ui_kit::settings_segments(ui, label, &mut selected, &labels) {
                    app.preferences.logo_accent = selected == 1;
                }
            });
            // A theme file another tool writes, reloaded when it changes (#7573).
            if FOLLOW_AVAILABLE {
                follow_row(app, ui);
            }
        });
    });
    ui_kit::settings_section(ui, "layout", app.tr("prefs.layout"), |ui| {
        ui_kit::settings_pair(
            ui,
            |ui| {
                ui_kit::settings_form(ui, t.form_label_width + t.space_16 + t.space_6, |ui| {
                    ui_kit::row(ui, app.tr("prefs.workspace"), |ui| {
                        let mut selected = usize::from(app.ui.layout == crate::shell::LayoutMode::Docked);
                        if ui_kit::segmented(
                            ui,
                            &mut selected,
                            &[crate::i18n::tr("ui.layout_floating"), crate::i18n::tr("ui.layout_compact")],
                            Default::default(),
                        ) {
                            let _ = app.run("view.layout", json!({"mode": if selected == 1 { "compact" } else { "floating" }}));
                        }
                    });
                    ui_kit::row(ui, app.tr("prefs.toolbar"), |ui| {
                        let mut selected = usize::from(!app.preferences.ribbon_labels);
                        if ui_kit::segmented(ui, &mut selected, &[app.tr("prefs.labels"), app.tr("prefs.icons_only")], Default::default()) {
                            app.preferences.ribbon_labels = selected == 0;
                        }
                    });
                    // The SKETCH tab outside a sketch too (#7904).
                    ui_kit::row(ui, "", |ui| {
                        let check = setting::sketch_tab_always(app, ui).on_hover_text(app.tr("prefs.sketch_tab_always_note"));
                        crate::scenario::publish_target("prefs:sketch_tab_always", check.rect);
                    });
                    // Where every feature dialog puts its OK and Cancel (#6708).
                    ui_kit::row(ui, app.tr("prefs.dialog_buttons"), |ui| {
                        let placements = [crate::ui_kit::DialogButtons::Bottom, crate::ui_kit::DialogButtons::Top];
                        let mut selected = placements.iter().position(|p| *p == app.preferences.dialog_buttons).unwrap_or(0);
                        if ui_kit::segmented(ui, &mut selected, &[app.tr("prefs.bottom"), app.tr("prefs.top")], Default::default())
                            && let Some(placement) = placements.get(selected)
                        {
                            app.preferences.dialog_buttons = *placement;
                        }
                    });
                    // Handles that don't depend on the language, by the saved value.
                    let row = app.tr("prefs.dialog_buttons");
                    for (label, key) in [("prefs.bottom", "bottom"), ("prefs.top", "top")] {
                        if let Some(at) = crate::scenario::handle_at(&format!("dialog:{row}:{}", app.tr(label))) {
                            crate::scenario::publish_handle(&format!("prefs:dialog_buttons:{key}"), at);
                        }
                    }
                    ui_kit::row(ui, app.tr("prefs.history"), |ui| {
                        let mut selected = if app.preferences.history_in_browser {
                            3
                        } else if app.preferences.timeline_left {
                            if app.preferences.timeline_under_browser { 2 } else { 1 }
                        } else {
                            0
                        };
                        if ui_kit::segmented(
                            ui,
                            &mut selected,
                            &[
                                crate::i18n::tr("prefs.bottom"),
                                crate::i18n::tr("prefs.left"),
                                crate::i18n::tr("prefs.under_browser"),
                                crate::i18n::tr("prefs.in_browser"),
                            ],
                            ui_kit::Segments { proportional: true, compact: true, ..Default::default() },
                        ) {
                            let dock = match selected {
                                1 => "left",
                                2 => "under_browser",
                                3 => "browser",
                                _ => "bottom",
                            };
                            let _ = app.run("view.history_layout", json!({ "dock": dock }));
                        }
                    });
                    for (short, legacy) in [
                        ("prefs.bottom", "ui.timeline_bottom"),
                        ("prefs.left", "ui.timeline_left_sidebar"),
                        ("prefs.under_browser", "ui.history_below_browser"),
                    ] {
                        let row = app.tr("prefs.history");
                        if let Some(at) = crate::scenario::handle_at(&format!("dialog:{row}:{}", app.tr(short))) {
                            crate::scenario::publish_handle(&format!("dialog:{row}:{}", app.tr(legacy)), at);
                        }
                    }
                    // The History at the left and the one in the browser can list a sketch under its
                    // feature (#6489); the timeline at the bottom can't.
                    ui_kit::row(ui, app.tr("prefs.history_sketches"), |ui| {
                        ui.add_enabled_ui(app.preferences.timeline_left || app.preferences.history_in_browser, |ui| {
                            let mut selected = usize::from(app.preferences.history_sketches == HistorySketches::UnderFeature);
                            if ui_kit::segmented(
                                ui,
                                &mut selected,
                                &[app.tr("prefs.history_sketches_in_order"), app.tr("prefs.history_sketches_under_feature")],
                                ui_kit::Segments { proportional: true, compact: true, ..Default::default() },
                            ) {
                                app.preferences.history_sketches =
                                    if selected == 1 { HistorySketches::UnderFeature } else { HistorySketches::InOrder };
                            }
                        });
                    });
                    // Only the history on the left or under the browser has room for these rows.
                    ui_kit::row(ui, "", |ui| {
                        ui.add_enabled_ui(app.preferences.timeline_left, |ui| {
                            let check = ui_kit::checkbox(ui, app.tr("prefs.history_origin_bodies"), &mut app.preferences.timeline_origin_bodies)
                                .on_hover_text(app.tr("prefs.history_origin_bodies_note"))
                                .on_disabled_hover_text(app.tr("prefs.history_origin_bodies_note"));
                            crate::scenario::publish_target("prefs:history_origin_bodies", check.rect);
                        });
                    });
                    ui_kit::row(ui, app.tr("prefs.browser"), |ui| {
                        ui.add_enabled_ui(app.ui.layout == crate::shell::LayoutMode::Floating, |ui| {
                            let mut selected = usize::from(app.preferences.browser_float);
                            if ui_kit::segmented(ui, &mut selected, &[app.tr("prefs.opaque"), app.tr("prefs.translucent")], Default::default()) {
                                app.preferences.browser_float = selected == 1;
                            }
                        });
                    });
                    // Where command dialogs open (#6707).
                    ui_kit::row(ui, app.tr("prefs.dialogs"), |ui| {
                        use crate::dialog_column::DialogPlace;
                        let mut selected = usize::from(app.preferences.dialog_place == DialogPlace::LeftColumn);
                        let options = [app.tr("prefs.dialogs_edge"), app.tr("prefs.dialogs_left_column")];
                        let segments = ui.scope(|ui| ui_kit::segmented(ui, &mut selected, &options, Default::default()));
                        segments.response.on_hover_text(app.tr("prefs.dialogs_note"));
                        if segments.inner {
                            app.preferences.dialog_place = if selected == 1 { DialogPlace::LeftColumn } else { DialogPlace::Edge };
                        }
                    });
                    // Where a dialog's and a sketch's OK and Cancel also sit (#6709).
                    ui_kit::row(ui, "", |ui| {
                        let check = setting::view_confirm_corner(app, ui).on_hover_text(app.tr("prefs.view_confirm_corner_note"));
                        crate::scenario::publish_target("prefs:view_confirm_corner", check.rect);
                    });
                    ui_kit::row(ui, app.tr("prefs.bodies_folders"), |ui| {
                        let modes = BodiesFolders::ALL;
                        let labels: Vec<_> = modes.iter().map(|m| app.tr(m.label_id())).collect();
                        let mut selected = modes.iter().position(|m| *m == app.preferences.browser_bodies_folders).unwrap_or(0);
                        if ui_kit::segmented(ui, &mut selected, &labels, ui_kit::Segments { proportional: true, compact: true, ..Default::default() })
                            && let Some(mode) = modes.get(selected)
                        {
                            app.preferences.browser_bodies_folders = *mode;
                        }
                    });
                    // What is being edited, the sketch's state and the units, along the bottom (#7243).
                    ui_kit::row(ui, "", |ui| {
                        let check = setting::status_bar(app, ui).on_hover_text(app.tr("prefs.status_bar_note"));
                        crate::scenario::publish_target("prefs:status_bar", check.rect);
                    });
                    ui_kit::text(ui, app.tr("prefs.layout_note"), ui_kit::Text::Hint("layout"));
                    ui_kit::text(ui, app.tr("prefs.history_sketches_note"), ui_kit::Text::Hint("history_sketches"));
                    ui_kit::LayoutPreview {
                        compact: app.ui.layout == crate::shell::LayoutMode::Docked,
                        labels: app.preferences.ribbon_labels,
                        history: if app.preferences.history_in_browser {
                            3
                        } else if app.preferences.timeline_left {
                            if app.preferences.timeline_under_browser { 2 } else { 1 }
                        } else {
                            0
                        },
                        translucent: app.preferences.browser_float,
                    }
                })
            },
            ui_kit::layout_preview,
        );
    });
    origin_plane_names(app, ui);
    super::navbar::show(app, ui);
    ui_kit::settings_section(ui, "scale", app.tr("prefs.scale_text"), |ui| {
        ui_kit::settings_pair(
            ui,
            |ui| {
                let response = ui_kit::slider(
                    ui,
                    &mut app.preferences.ui_scale,
                    0.5..=2.5,
                    ui_kit::Slider { step: Some(0.05), format: |v| format!("{:.0} %", v * 100.0), ..Default::default() },
                );
                crate::scenario::publish_target("prefs:scale", response.rect);
                ui.horizontal_wrapped(|ui| {
                    for value in [0.75, 1.0, 1.1, 1.25, 1.5, 2.0] {
                        if ui_kit::button(
                            ui,
                            format!("{:.0}%", value * 100.0),
                            if (app.preferences.ui_scale - value).abs() < 0.001 { ui_kit::Button::SmallSelected } else { ui_kit::Button::Small },
                        )
                        .clicked()
                        {
                            app.preferences.ui_scale = value;
                        }
                    }
                });
                ui_kit::text(ui, app.tr("prefs.scale_note"), ui_kit::Text::Hint("scale"));
                app.preferences.ui_scale
            },
            ui_kit::scale_specimen,
        );
        ui_kit::divider(ui, "scale_font");
        if !cfg!(target_arch = "wasm32") {
            ui_kit::settings_font_row(ui, crate::i18n::tr("prefs.font"), |ui, part| {
                if part == 0 {
                    ui_font_row(app, ui);
                    return;
                }
                if ui_kit::button(ui, app.tr("prefs.choose_font"), ui_kit::Button::Normal).clicked() {
                    app.pick_open_for(&["ttf", "otf", "ttc"], |app, path| {
                        if let Some(path) = path {
                            if crate::fonts::ui_font(&path).is_some() {
                                app.preferences.ui_font = path;
                            } else {
                                app.set_status(crate::i18n::tr("prefs.font_unreadable"), true);
                            }
                        }
                    });
                }
                if ui_kit::button(ui, app.tr_label("Reset"), ui_kit::Button::Ghost).clicked() {
                    app.preferences.ui_font.clear();
                }
            });
            ui_kit::font_specimen(ui, app.tr("prefs.font_sample"));
            ui_kit::text(ui, app.tr("prefs.font_fallback"), ui_kit::Text::Hint("font_fallback"));
        }
    });
    ui_kit::settings_section(ui, "colour_vision", app.tr("ui.colour_vision"), |ui| {
        let mode = app.preferences.color_vision;
        ui.horizontal_wrapped(|ui| {
            let modes = crate::theme::ColorVision::ALL;
            let labels: Vec<_> = modes.iter().map(|m| crate::i18n::tr(m.label_id())).collect();
            let mut selected = modes.iter().position(|m| *m == mode).unwrap_or(0);
            if ui_kit::settings_segments(ui, app.tr("ui.colour_vision_mode"), &mut selected, &labels)
                && let Some(mode) = modes.get(selected)
            {
                app.preferences.color_vision = *mode;
            }
            if let Some(rect) = crate::scenario::rect_of(&format!("dialog:segments:{}", app.tr("ui.colour_vision_mode"))) {
                crate::scenario::publish_target("prefs:color_vision", rect);
            }
            ui_kit::status_palette_sample(ui, app.preferences.color_vision);
        });
        // What the modes recolour, in the mode being chosen (#7152): the status chips above keep
        // their colours in every mode.
        ui_kit::colour_vision_sample(ui, &crate::theme::Tokens::get_in(app.preferences.color_vision));
        if app.preferences.color_vision != mode {
            ui.ctx().request_repaint();
        }
        ui_kit::text(ui, app.tr("ui.colour_vision_note"), ui_kit::Text::Hint("colour_vision"));
    });
    colour_and_glass(app, ui, t);
}
