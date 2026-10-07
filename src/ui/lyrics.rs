//! The words of the playing track, in a side panel that follows the song.

use egui::{Align, Color32, Frame, Layout, Margin, Rect, Sense, UiBuilder, pos2, vec2};

use crate::app::App;
use crate::i18n::{gettext, pgettext};
use crate::model::{Action, Loadable};
use crate::theme::{self, Icon};

use super::widgets;

const LINE_SIZE: f32 = 19.0;
const LINE_GAP: f32 = 10.0;
/// Where the line being sung sits, as a fraction of the visible lyrics from
/// the top: high up, so the lines to come fill most of the view.
const SUNG_LINE_AT: f32 = 0.2;

/// Scrolls so the middle of `line` sits `SUNG_LINE_AT` of the way down the
/// visible lyrics.
fn show_sung_line(ui: &egui::Ui, line: Rect, animation: Option<egui::style::ScrollAnimation>) {
    let above = (ui.clip_rect().height() * SUNG_LINE_AT - line.height() / 2.0).max(0.0);
    let target = Rect::from_min_max(pos2(line.left(), line.top() - above), line.max);
    match animation {
        Some(animation) => ui.scroll_to_rect_animation(target, Some(Align::Min), animation),
        None => ui.scroll_to_rect(target, Some(Align::Min)),
    }
}
/// How long a line takes to light up or fade.
const LIGHT_UP_SECONDS: f32 = 0.22;

fn blend(from: egui::Color32, to: egui::Color32, t: f32) -> egui::Color32 {
    let t = t.clamp(0.0, 1.0);
    egui::Color32::from(egui::Rgba::from(from) * (1.0 - t) + egui::Rgba::from(to) * t)
}

/// Marks the timed line under the pointer as one a click jumps to:
/// brightened to `color` and underlined, like a link, before the click.
fn mark_hovered_line(
    ui: &egui::Ui,
    response: &egui::Response,
    galley: &std::sync::Arc<egui::Galley>,
    color: Color32,
) {
    if !response.hovered() {
        return;
    }
    let painter = ui.painter();
    painter.galley_with_override_text_color(response.rect.min, galley.clone(), color);
    let offset = response.rect.min.to_vec2();
    for row in &galley.rows {
        let row = row.rect().translate(offset);
        painter.hline(
            row.x_range(),
            row.bottom() - 1.0,
            egui::Stroke::new(1.5, color),
        );
    }
}

pub fn side_panel(app: &mut App, ui: &mut egui::Ui) {
    let palette = app.palette;
    let fit = super::yielding_panel(
        ui.ctx(),
        "lyrics-panel",
        theme::SIDE_PANEL_MIN_WIDTH..=640.0,
        app.settings.lyrics_width,
        ui.available_width() - super::topbar::least_width(ui.ctx()),
    );
    let panel = egui::Panel::right("lyrics-panel")
        .resizable(true)
        .default_size(app.settings.lyrics_width)
        .size_range(fit.range.clone())
        .show_separator_line(false)
        .frame(
            Frame::new()
                .fill(palette.panel)
                .inner_margin(Margin::symmetric(12, 12)),
        );
    let response = panel.show(ui, |ui| {
        let window_controls = super::window_controls_reservation(
            ui.ctx(),
            app.show_queue_panel,
            app.show_lyrics_panel,
            ui.available_width(),
        );
        ui.add_space(window_controls.lyrics_top);
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            theme::text(
                ui,
                gettext(app.locale, "Lyrics"),
                theme::bold(18.0),
                palette.text,
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if theme::icon_button(
                    ui,
                    Icon::X,
                    18.0,
                    palette.secondary,
                    palette.text,
                    &gettext(app.locale, "Close"),
                )
                .clicked()
                {
                    app.actions.push(Action::ToggleLyricsPanel);
                }
                if theme::icon_button(
                    ui,
                    Icon::Expand,
                    18.0,
                    palette.secondary,
                    palette.text,
                    &gettext(app.locale, "Full screen lyrics"),
                )
                .clicked()
                {
                    app.actions.push(Action::SetLyricsFullscreen(true));
                }
                let loaded = matches!(&app.lyrics, Loadable::Loaded(Some(_)));
                if loaded
                    && !app.lyrics_following
                    && theme::pill_button(
                        ui,
                        &palette,
                        &pgettext(app.locale, "lyrics", "Sync"),
                        false,
                    )
                    .clicked()
                {
                    app.lyrics_following = true;
                    app.lyrics_line_shown = None;
                }
            });
        });
        ui.add_space(8.0);
        contents(app, ui);
    });
    let current_width = response.response.rect.width();
    if (app.settings.lyrics_width - current_width).abs() > 1.0
        && super::panel_width_chosen(ui.ctx(), "lyrics-panel", &fit)
    {
        app.settings.lyrics_width = current_width;
        app.actions.push(Action::SettingsChanged);
    }
}

fn contents(app: &mut App, ui: &mut egui::Ui) {
    let palette = app.palette;
    let Some(now) = app.now_playing() else {
        widgets::empty_state(
            ui,
            &palette,
            Icon::Mic,
            &gettext(app.locale, "Nothing playing"),
            &gettext(app.locale, "Play a song to see its lyrics."),
        );
        return;
    };
    let lyrics = match &app.lyrics {
        Loadable::NotLoaded | Loadable::Loading => {
            widgets::loading_row(ui, &palette, app.locale);
            return;
        }
        Loadable::Failed(error) => {
            let message = gettext(
                app.locale,
                // Translators: Keep {error} unchanged. It is the original failure detail.
                "Couldn't fetch the lyrics: {error}",
            )
            .replace("{error}", error);
            ui.add_space(8.0);
            theme::text(ui, message, theme::regular(13.0), palette.secondary);
            ui.add_space(8.0);
            if theme::pill_button(ui, &palette, &gettext(app.locale, "Try again"), false).clicked()
            {
                app.request_lyrics();
            }
            return;
        }
        Loadable::Loaded(None) => {
            widgets::empty_state(
                ui,
                &palette,
                Icon::Mic,
                &gettext(app.locale, "No lyrics"),
                &gettext(app.locale, "No lyrics found for this track."),
            );
            return;
        }
        Loadable::Loaded(Some(lyrics)) if lyrics.instrumental => {
            widgets::empty_state(
                ui,
                &palette,
                Icon::Music,
                &gettext(app.locale, "Instrumental"),
                &gettext(app.locale, "No timed lyrics for this track."),
            );
            return;
        }
        Loadable::Loaded(Some(lyrics)) => lyrics.clone(),
    };

    let active = lyrics.active_line(now.position_ms);
    let follow = app.lyrics_following && app.lyrics_line_shown != Some(active);
    // The line being sung is bold and in the accent colour; every other
    // line is quiet, regular text, the same before and after it has been
    // sung. A line takes 220 ms to light up or fade, as in omarchy-lyrics.
    let quiet = palette.text.gamma_multiply(0.45);
    let scroll = crate::autoscroll::show(
        ui,
        egui::ScrollArea::vertical()
            .id_salt("lyrics-scroll")
            .auto_shrink([false, false]),
        egui::Vec2b::new(false, true),
        |ui| {
            // Before the first line there is nothing to highlight, so the
            // panel sits at the top rather than wherever it was left.
            if follow && lyrics.synced && active.is_none() {
                let top = ui.cursor().min;
                ui.scroll_to_rect(
                    egui::Rect::from_min_size(top, egui::vec2(1.0, 1.0)),
                    Some(Align::Min),
                );
            }
            ui.add_space(12.0);
            for (index, line) in lyrics.lines.iter().enumerate() {
                let is_active = active == Some(index);
                let lit = ui.ctx().animate_bool_with_time(
                    egui::Id::new("lyric-line").with(index),
                    is_active,
                    LIGHT_UP_SECONDS,
                );
                let color = blend(quiet, palette.accent, lit);
                let font = if lit > 0.5 {
                    theme::bold(LINE_SIZE)
                } else {
                    theme::regular(LINE_SIZE)
                };
                // A timed line with no words is the band playing on.
                let text = if line.text.is_empty() && lyrics.synced {
                    "\u{266a}"
                } else {
                    line.text.as_str()
                };
                let sense = if lyrics.synced {
                    Sense::click()
                } else {
                    Sense::hover()
                };
                let galley = if crate::bidi::is_rtl(text) {
                    crate::bidi::layout(
                        ui.painter(),
                        text,
                        font,
                        color,
                        ui.available_width(),
                        usize::MAX,
                        None,
                    )
                } else {
                    ui.painter()
                        .layout(text.to_owned(), font, color, ui.available_width())
                };
                let response = ui.add(egui::Label::new(galley.clone()).sense(sense));
                if lyrics.synced {
                    mark_hovered_line(ui, &response, &galley, palette.text);
                }
                crate::autoscroll::row(ui, &response);
                let rect = response.rect;
                if lyrics.synced
                    && response.clicked()
                    && let Some(at_ms) = line.at_ms
                {
                    app.actions.push(Action::Seek(at_ms));
                    app.lyrics_following = true;
                }
                if is_active && follow {
                    show_sung_line(ui, rect, None);
                }
                ui.add_space(LINE_GAP);
            }
            // Words without timing can only be followed by the clock: sit
            // at the part of the text the song is probably at.
            if app.lyrics_following && !lyrics.synced && now.duration_ms > 0 {
                let fraction =
                    (f64::from(now.position_ms) / f64::from(now.duration_ms)).clamp(0.0, 1.0);
                let content = ui.min_rect();
                let y = content.top() + content.height() * fraction as f32;
                ui.scroll_to_rect(
                    egui::Rect::from_min_max(
                        egui::pos2(content.left(), y),
                        egui::pos2(content.right(), y + 1.0),
                    ),
                    Some(Align::Center),
                );
            }
            // Room for the last line to rise to where a sung line sits.
            ui.add_space((ui.clip_rect().height() * (1.0 - SUNG_LINE_AT)).max(60.0));
        },
    );
    crate::autoscroll::lyrics(ui, scroll.id);
    // Scrolling by hand means the reader wants to look elsewhere; the
    // Follow button in the header picks the song back up.
    if ui.rect_contains_pointer(scroll.inner_rect)
        && ui.input(|input| input.smooth_scroll_delta.y != 0.0)
    {
        app.lyrics_following = false;
    }
    app.lyrics_line_shown = Some(active);
}

pub fn fullscreen(app: &mut App, ui: &mut egui::Ui) {
    // Drawn over the backdrop the window's layout has already painted.
    egui::CentralPanel::default()
        .frame(Frame::new())
        .show(ui, |ui| {
            theme::apply_local(ui, &theme::Palette::dark());
            // With the controls gone, the pointer goes too.
            if cover_controls_hidden(app, ui.ctx()) >= 1.0 {
                ui.ctx().set_cursor_icon(egui::CursorIcon::None);
            }
            let rect = ui.max_rect();
            let top = theme::titlebar_inset(ui.ctx()) + 24.0;
            if app.fullscreen_cover {
                cover_only(app, ui, rect, top);
                return;
            }
            // In the window, the lyrics' header starts below the Windows
            // caption buttons when they sit over it.
            let caption = super::window_controls_reservation(ui.ctx(), false, false, rect.width());
            let top = if caption.topbar_top > 0.0 || caption.topbar_width > 0.0 {
                top.max(super::WINDOWS_WINDOW_CONTROLS_HEIGHT + 8.0)
            } else {
                top
            };
            if app.now_playing().is_some() && rect.width() >= COVER_BESIDE_MIN_WIDTH {
                with_cover(app, ui, rect, top);
                return;
            }
            let width = fullscreen_content_width(rect.width());
            let region = Rect::from_min_max(
                pos2(rect.center().x - width / 2.0, rect.top() + top),
                pos2(rect.center().x + width / 2.0, rect.bottom()),
            );
            let mut content = ui.new_child(UiBuilder::new().max_rect(region));
            fullscreen_header(app, &mut content);
            content.add_space(20.0);
            track_heading(app, &mut content);
            content.add_space(16.0);
            fullscreen_contents(app, &mut content);
        });
}

/// The widest the lyrics get beside the cover, so lines stay easy to read.
const LYRICS_BESIDE_WIDTH: f32 = 640.0;

/// The narrowest window that shows the cover beside the lyrics; narrower
/// ones keep a single column with a small cover in the heading.
const COVER_BESIDE_MIN_WIDTH: f32 = 900.0;

/// Full screen with the cover large: beside the lyrics when there are
/// words to follow, and alone in the middle when there are none, a calm
/// view of what is playing.
fn with_cover(app: &mut App, ui: &mut egui::Ui, rect: Rect, top: f32) {
    let outer = Rect::from_min_max(
        pos2(rect.left() + 48.0, rect.top() + top),
        pos2(rect.right() - 48.0, rect.bottom() - 40.0),
    );
    let mut header = ui.new_child(UiBuilder::new().max_rect(outer));
    fullscreen_header(app, &mut header);
    let below = Rect::from_min_max(
        pos2(outer.left(), header.min_rect().bottom() + 24.0),
        outer.max,
    );
    // The cover moves aside only for words to read. While they load it
    // stays in the middle, so a song that turns out to have none never
    // moves at all.
    let words = matches!(&app.lyrics, Loadable::Loaded(Some(lyrics)) if !lyrics.instrumental);
    if words {
        // The cover and the lyrics are one group, centred in the window.
        let gap = 64.0;
        let side = (below.width() * 0.38)
            .min(below.height() - 90.0)
            .clamp(200.0, 520.0);
        let lyrics_width = (below.width() - side - gap).min(LYRICS_BESIDE_WIDTH);
        let left = below.center().x - (side + gap + lyrics_width) / 2.0;
        let column = Rect::from_min_size(
            pos2(left, below.center().y - (side + 90.0) / 2.0),
            vec2(side, side + 90.0),
        );
        big_cover(app, ui, column, Align::Min, 1.0);
        let lyrics = Rect::from_min_max(
            pos2(column.right() + gap, below.top()),
            pos2(column.right() + gap + lyrics_width, below.bottom()),
        );
        let mut content = ui.new_child(UiBuilder::new().max_rect(lyrics));
        fullscreen_contents(app, &mut content);
    } else {
        let side = (below.height() - 140.0)
            .min(below.width() * 0.5)
            .clamp(200.0, 560.0);
        let column = Rect::from_center_size(below.center(), vec2(side, side + 90.0));
        big_cover(app, ui, column, Align::Center, 1.0);
        // Why there are no words, quietly, under the song, or that they
        // are still being fetched.
        let (heading, detail) = match &app.lyrics {
            Loadable::Loaded(Some(_)) => (
                gettext(app.locale, "Instrumental"),
                gettext(app.locale, "No timed lyrics for this track."),
            ),
            Loadable::Loaded(None) => (
                gettext(app.locale, "No lyrics"),
                gettext(app.locale, "No lyrics found for this track."),
            ),
            Loadable::Failed(error) => (
                gettext(
                    app.locale,
                    // Translators: Keep {error} unchanged. It is the original failure detail.
                    "Couldn't fetch the lyrics: {error}",
                )
                .replace("{error}", error)
                .into(),
                Default::default(),
            ),
            Loadable::NotLoaded | Loadable::Loading => {
                (gettext(app.locale, "Loading…"), Default::default())
            }
        };
        let heading = ui.painter().text(
            pos2(column.center().x, column.bottom() + 8.0),
            egui::Align2::CENTER_TOP,
            heading,
            theme::semibold(13.0),
            Color32::from_gray(200),
        );
        if matches!(app.lyrics, Loadable::Failed(_)) {
            let retry = Rect::from_center_size(
                pos2(column.center().x, heading.bottom() + 24.0),
                vec2(column.width(), 32.0),
            );
            let mut retry_ui = ui.new_child(
                UiBuilder::new()
                    .max_rect(retry)
                    .layout(Layout::top_down(Align::Center)),
            );
            let label = gettext(app.locale, "Try again");
            if theme::pill_button(&mut retry_ui, &theme::Palette::dark(), &label, false).clicked() {
                app.actions.push(Action::RetryLyrics);
            }
            return;
        }
        ui.painter().text(
            pos2(column.center().x, heading.bottom() + 4.0),
            egui::Align2::CENTER_TOP,
            detail,
            theme::regular(13.0),
            Color32::from_gray(170),
        );
    }
}

/// Only the cover, as large as the space allows, at any window width: full
/// screen, or the Now playing view in the window. Clicking it opens its
/// album or show.
fn cover_only(app: &mut App, ui: &mut egui::Ui, rect: Rect, top: f32) {
    let hidden = cover_controls_hidden(app, ui.ctx());
    let controls = controls_opacity(hidden);
    let outer = Rect::from_min_max(
        pos2(rect.left() + 48.0, rect.top() + top),
        pos2(rect.right() - 48.0, rect.bottom() - 40.0),
    );
    // In the window, the Windows caption buttons may sit at the header's
    // right end when no panel is open beside it. The view draws only the
    // queue's panel: a lyrics panel left open behind it is not on screen.
    let caption =
        super::window_controls_reservation(ui.ctx(), app.show_queue_panel, false, rect.width());
    let header_rect = Rect::from_min_max(
        pos2(outer.left(), outer.top() + caption.topbar_top),
        pos2(
            outer
                .right()
                .min(rect.right() - caption.topbar_width - 12.0),
            outer.bottom(),
        ),
    );
    let mut header = ui.new_child(UiBuilder::new().max_rect(header_rect));
    fullscreen_header(app, &mut header);
    let below = Rect::from_min_max(
        pos2(outer.left(), header.min_rect().bottom() + 24.0),
        outer.max,
    );
    let Some(now) = app.now_playing() else {
        let mut content = ui.new_child(UiBuilder::new().max_rect(below));
        widgets::empty_state(
            &mut content,
            &theme::Palette::dark(),
            Icon::Music,
            &gettext(app.locale, "Nothing playing"),
            &gettext(app.locale, "Pick a song, album, or playlist"),
        );
        return;
    };
    let side = cover_only_side(below.size());
    let column = Rect::from_center_size(below.center(), vec2(side, side + 90.0));
    big_cover(app, ui, column, Align::Center, controls);
    // Once the bar has gone, the title and artists come to rest in its
    // place at the bottom left, out of the cover's way.
    let corner_opacity = corner_words_opacity(hidden);
    if corner_opacity > 0.0 {
        let bottom = ui.ctx().content_rect().bottom() - 40.0;
        let corner = Rect::from_min_max(
            pos2(rect.left() + 48.0, bottom - 120.0),
            pos2(rect.center().x, bottom),
        );
        let mut words = ui.new_child(
            UiBuilder::new()
                .max_rect(corner)
                .layer_id(egui::LayerId::new(
                    egui::Order::Middle,
                    egui::Id::new("cover-corner-words"),
                ))
                .layout(Layout::bottom_up(Align::Min)),
        );
        words.set_clip_rect(corner);
        words.multiply_opacity(corner_opacity);
        track_words(&mut words, &now.title, &now.subtitle);
    }
    let cover = Rect::from_min_size(column.min, vec2(side, side));
    let response = ui.interact(cover, egui::Id::new("fullscreen-cover"), Sense::click());
    // A click that wakes the hidden controls only wakes them.
    if response.clicked() && hidden < 0.5 {
        if let Some(id) = now.album_id {
            app.actions
                .push(Action::Open(crate::model::Page::Album(id)));
        } else if let Some(id) = now.show_id {
            app.actions.push(Action::Open(crate::model::Page::Show(id)));
        }
    }
    if hidden < 1.0 && response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
}

/// How long the Now playing cover waits, without the pointer moving or a
/// key being pressed, before its controls and the player bar fade away.
const COVER_IDLE_SECONDS: f64 = 3.0;
/// How long the whole change takes, out or back in. It runs in two steps:
/// the controls, the player bar among them, fade first; then the title and
/// artists fade in at the bottom left. Coming back runs the steps in
/// reverse.
const COVER_FADE_SECONDS: f32 = 0.8;

/// The first step: how much of the controls still shows.
pub(super) fn controls_opacity(hidden: f32) -> f32 {
    1.0 - smoothstep(hidden * 2.0)
}

/// The second step: how much of the title and artists in the corner shows.
fn corner_words_opacity(hidden: f32) -> f32 {
    smoothstep(hidden * 2.0 - 1.0)
}

/// Eases a step in and out, so neither end starts or stops abruptly.
fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// How far the Now playing cover's controls, the player bar among them,
/// have faded away: 0 shown, 1 gone. Moving the pointer or pressing a key
/// brings them back, and they stay while the pointer rests on the bar.
pub(super) fn cover_controls_hidden(app: &App, ctx: &egui::Context) -> f32 {
    let id = egui::Id::new("fullscreen-cover-activity");
    if !app.view_open() {
        ctx.data_mut(|data| data.remove::<f64>(id));
        return 0.0;
    }
    // Paused, the controls stay: they are what plays the song again.
    let paused = app.now_playing().is_none_or(|now| !now.playing);
    let (now, active) = ctx.input(|input| {
        let over_bar = input.pointer.hover_pos().is_some_and(|position| {
            position.y >= input.content_rect().bottom() - theme::PLAYER_BAR_HEIGHT
        });
        let acted = input.events.iter().any(|event| {
            matches!(
                event,
                egui::Event::PointerMoved(_)
                    | egui::Event::PointerButton { .. }
                    | egui::Event::MouseWheel { .. }
                    | egui::Event::Key { .. }
                    | egui::Event::Text(_)
                    | egui::Event::Touch { .. }
            )
        });
        (input.time, over_bar || acted || paused || app.show_devices)
    });
    let last = ctx.data_mut(|data| {
        let last = data.get_temp_mut_or_insert_with(id, || now);
        if active {
            *last = now;
        }
        *last
    });
    let idle = now - last >= COVER_IDLE_SECONDS;
    if !idle {
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(
            COVER_IDLE_SECONDS - (now - last),
        ));
    }
    ctx.animate_bool_with_time(
        egui::Id::new("fullscreen-cover-hidden"),
        idle,
        COVER_FADE_SECONDS,
    )
}

/// The cover's side in the full screen cover view: as large as the space
/// below the header allows, leaving room for the title and artists beneath.
fn cover_only_side(space: egui::Vec2) -> f32 {
    (space.y - 140.0).min(space.x).clamp(0.0, 900.0)
}

/// The playing song's cover filling the top of `column`, with its title and
/// artists beneath, aligned to its left edge or centred, the words at
/// `words_opacity`.
fn big_cover(app: &App, ui: &mut egui::Ui, column: Rect, align: Align, words_opacity: f32) {
    let Some(now) = app.now_playing() else {
        return;
    };
    let side = column.width();
    let cover = Rect::from_min_size(column.min, vec2(side, side));
    let radius = 10.0;
    ui.painter().add(
        egui::epaint::Shadow {
            offset: [0, 18],
            blur: 48,
            spread: 0,
            color: Color32::from_black_alpha(140),
        }
        .as_shape(cover, radius),
    );
    widgets::paint_cover(
        ui,
        &theme::Palette::dark(),
        now.art_url.as_deref().or(now.art_small.as_deref()),
        cover,
        radius,
        Icon::Music,
        Some(app.backend.art()),
    );
    let words = Rect::from_min_max(pos2(column.left(), cover.bottom() + 18.0), column.max);
    let mut text = ui.new_child(
        UiBuilder::new()
            .max_rect(words)
            .layout(Layout::top_down(align)),
    );
    text.multiply_opacity(words_opacity);
    track_words(&mut text, &now.title, &now.subtitle);
}

/// The song's title over its artists, in the order of `ui`'s layout: a
/// bottom-up layout is given the artists first.
fn track_words(ui: &mut egui::Ui, title: &str, artists: &str) {
    ui.spacing_mut().item_spacing.y = 4.0;
    let title = egui::Label::new(
        egui::RichText::new(title)
            .font(theme::semibold(22.0))
            .color(Color32::WHITE),
    )
    .truncate();
    let artists = egui::Label::new(
        egui::RichText::new(artists)
            .font(theme::regular(14.0))
            .color(Color32::from_gray(225)),
    )
    .truncate();
    if ui.layout().main_dir() == egui::Direction::BottomUp {
        ui.add(artists);
        ui.add(title);
    } else {
        ui.add(title);
        ui.add(artists);
    }
}

fn fullscreen_content_width(viewport_width: f32) -> f32 {
    let available = (viewport_width - 48.0).max(0.0);
    (viewport_width * 0.72).clamp(400.0, 960.0).min(available)
}

fn preferred_backdrop_art(small: Option<String>, large: Option<String>) -> Option<String> {
    small.or(large)
}

/// The blurred cover behind the Now playing view. It is painted across the
/// whole window, under the player bar too, so the bar fades into it rather
/// than into black, and nothing moves when the bar goes.
pub(super) fn backdrop(app: &mut App, ui: &mut egui::Ui, rect: Rect) {
    let art = app
        .now_playing()
        .and_then(|now| preferred_backdrop_art(now.art_small, now.art_url));
    let painter = ui.painter().with_clip_rect(rect);
    painter.rect_filled(rect, 0.0, theme::Palette::dark().window);
    if let Some(texture) = app
        .lyrics_backdrop
        .texture(ui.ctx(), app.backend.art(), art.as_deref())
    {
        painter.image(
            texture.id(),
            rect,
            cover_uv(rect.size(), texture.size_vec2()),
            Color32::from_gray(180),
        );
    }
    painter.rect_filled(rect, 0.0, Color32::from_black_alpha(120));
    widgets::paint_vertical_gradient(
        ui,
        rect,
        Color32::from_black_alpha(0),
        Color32::from_black_alpha(95),
    );
}

fn cover_uv(view: egui::Vec2, image: egui::Vec2) -> Rect {
    let ratio = (view.x / view.y.max(1.0)) / (image.x / image.y.max(1.0));
    let size = if ratio > 1.0 {
        vec2(1.0, 1.0 / ratio)
    } else {
        vec2(ratio, 1.0)
    };
    Rect::from_center_size(pos2(0.5, 0.5), size)
}

/// The view's header, fading away with the player bar while nothing moves.
fn fullscreen_header(app: &mut App, ui: &mut egui::Ui) {
    let palette = theme::Palette::dark();
    let windowed = app.lyrics_fullscreen.is_none();
    let controls = controls_opacity(cover_controls_hidden(app, ui.ctx()));
    ui.horizontal(|ui| {
        ui.multiply_opacity(controls);
        if controls < 0.5 {
            ui.disable();
        }
        theme::text(
            ui,
            if app.fullscreen_cover {
                gettext(app.locale, "Now playing")
            } else {
                gettext(app.locale, "Lyrics")
            },
            theme::bold(18.0),
            palette.text,
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            // In the window the view closes or fills the screen; full
            // screen steps back to the window.
            if windowed {
                if theme::icon_button(
                    ui,
                    Icon::Minimize2,
                    18.0,
                    palette.text,
                    palette.text,
                    &gettext(app.locale, "Close Now playing view (Esc)"),
                )
                .clicked()
                {
                    app.actions.push(Action::ToggleCoverView);
                }
                if theme::icon_button(
                    ui,
                    Icon::Maximize,
                    18.0,
                    palette.text,
                    palette.text,
                    &gettext(app.locale, "Full screen"),
                )
                .clicked()
                {
                    app.actions.push(Action::ShowCoverFullscreen);
                }
            } else if theme::icon_button(
                ui,
                Icon::Minimize,
                18.0,
                palette.text,
                palette.text,
                &gettext(app.locale, "Leave full screen (Esc)"),
            )
            .clicked()
            {
                app.actions.push(Action::SetLyricsFullscreen(false));
            }
            // Switches between the cover alone and the cover with lyrics.
            let (mic, tooltip) = if app.fullscreen_cover {
                (palette.secondary, gettext(app.locale, "Show lyrics"))
            } else {
                (palette.accent, gettext(app.locale, "Hide lyrics"))
            };
            if theme::icon_button(ui, Icon::Mic, 18.0, mic, palette.text, &tooltip).clicked() {
                app.actions.push(Action::ToggleLyricsPanel);
            }
            let loaded = matches!(&app.lyrics, Loadable::Loaded(Some(_)));
            if !app.fullscreen_cover
                && loaded
                && !app.lyrics_following
                && theme::pill_button(ui, &palette, &pgettext(app.locale, "lyrics", "Sync"), false)
                    .clicked()
            {
                app.actions.push(Action::FollowLyrics);
            }
        });
    });
}

fn track_heading(app: &App, ui: &mut egui::Ui) {
    if let Some(now) = app.now_playing() {
        ui.horizontal(|ui| {
            let size = 52.0;
            let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
            widgets::paint_cover(
                ui,
                &theme::Palette::dark(),
                now.art_small.as_deref().or(now.art_url.as_deref()),
                rect,
                4.0,
                Icon::Music,
                Some(app.backend.art()),
            );
            ui.vertical(|ui| {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(&now.title)
                            .font(theme::semibold(22.0))
                            .color(Color32::WHITE),
                    )
                    .truncate(),
                );
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(&now.subtitle)
                            .font(theme::regular(13.0))
                            .color(Color32::from_gray(235)),
                    )
                    .truncate(),
                );
            });
        });
    }
}

fn fullscreen_contents(app: &mut App, ui: &mut egui::Ui) {
    let palette = theme::Palette::dark();
    let Some(now) = app.now_playing() else {
        widgets::empty_state(
            ui,
            &palette,
            Icon::Mic,
            &gettext(app.locale, "Nothing playing"),
            &gettext(app.locale, "Play a song to see its lyrics."),
        );
        return;
    };
    let lyrics = match &app.lyrics {
        Loadable::NotLoaded | Loadable::Loading => {
            widgets::loading_row(ui, &palette, app.locale);
            return;
        }
        Loadable::Failed(error) => {
            let message = gettext(
                app.locale,
                // Translators: Keep {error} unchanged. It is the original failure detail.
                "Couldn't fetch the lyrics: {error}",
            )
            .replace("{error}", error);
            ui.add_space(8.0);
            theme::text(ui, message, theme::regular(13.0), palette.text);
            ui.add_space(8.0);
            if theme::pill_button(ui, &palette, &gettext(app.locale, "Try again"), false).clicked()
            {
                app.actions.push(Action::RetryLyrics);
            }
            return;
        }
        Loadable::Loaded(None) => {
            widgets::empty_state(
                ui,
                &palette,
                Icon::Mic,
                &gettext(app.locale, "No lyrics"),
                &gettext(app.locale, "No lyrics found for this track."),
            );
            return;
        }
        Loadable::Loaded(Some(lyrics)) if lyrics.instrumental => {
            widgets::empty_state(
                ui,
                &palette,
                Icon::Music,
                &gettext(app.locale, "Instrumental"),
                &gettext(app.locale, "No timed lyrics for this track."),
            );
            return;
        }
        Loadable::Loaded(Some(lyrics)) => lyrics.clone(),
    };

    let active = lyrics.active_line(now.position_ms);
    // A Follow click resets the remembered line after drawing. Other frames
    // record the shown line before any line-click action restores following.
    if !app
        .actions
        .iter()
        .any(|action| matches!(action, Action::FollowLyrics))
    {
        app.actions.push(Action::LyricsLineShown(active));
    }
    let viewport = ui.available_rect_before_wrap();
    let manual_scroll = ui.rect_contains_pointer(viewport)
        && ui.input(|input| {
            input.smooth_scroll_delta.y != 0.0
                || (input.pointer.primary_down() && input.pointer.delta().y != 0.0)
        });
    let following = app.lyrics_following && !manual_scroll;
    let follow = following && app.lyrics_line_shown != Some(active);
    let animation = egui::style::ScrollAnimation::duration(0.45);
    let size = (ui.available_width() * 0.046).clamp(28.0, 42.0);
    // The line being sung brightens; all lines keep the same font metrics
    // so highlighting cannot rewrap the words during a transition.
    // A line takes 300 ms to light up or fade.
    let quiet = palette.text.gamma_multiply(0.68);
    ui.spacing_mut().scroll.fade.strength = 0.0;
    egui::ScrollArea::vertical()
        .id_salt(("fullscreen-lyrics-scroll", &now.uri))
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // Before the first line there is nothing to highlight, so the
            // panel sits at the top rather than wherever it was left.
            if follow && lyrics.synced && active.is_none() {
                let top = ui.cursor().min;
                ui.scroll_to_rect_animation(
                    egui::Rect::from_min_size(top, egui::vec2(1.0, 1.0)),
                    Some(Align::Min),
                    animation,
                );
            }
            // Room for the first line to sit where a sung line sits, and for
            // the last to rise to it.
            let (padding, below) = if lyrics.synced {
                (
                    (viewport.height() * SUNG_LINE_AT - size).max(12.0),
                    viewport.height() * (1.0 - SUNG_LINE_AT),
                )
            } else {
                (12.0, 60.0)
            };
            ui.add_space(padding);
            for (index, line) in lyrics.lines.iter().enumerate() {
                let is_active = active == Some(index);
                let lit = ui.ctx().animate_bool_with_time(
                    egui::Id::new("lyric-line").with(("fullscreen", &now.uri, index)),
                    is_active,
                    0.3,
                );
                let color = if lyrics.synced {
                    blend(quiet, palette.text, lit)
                } else {
                    palette.text
                };
                let font = theme::bold(size);
                // A timed line with no words is the band playing on.
                let text = if line.text.is_empty() && lyrics.synced {
                    "\u{266a}"
                } else {
                    line.text.as_str()
                };
                let sense = if lyrics.synced {
                    Sense::click()
                } else {
                    Sense::hover()
                };
                let galley = crate::bidi::layout(
                    ui.painter(),
                    text,
                    font,
                    color,
                    ui.available_width(),
                    usize::MAX,
                    None,
                );
                let center = ui.cursor().top() + galley.size().y * 0.5;
                let edge = ((center - viewport.top()).min(viewport.bottom() - center)
                    / (size * 2.0))
                    .clamp(0.0, 1.0);
                let response = ui
                    .scope(|ui| {
                        ui.multiply_opacity(edge * edge * (3.0 - 2.0 * edge));
                        let response = ui.add(egui::Label::new(galley.clone()).sense(sense));
                        if lyrics.synced {
                            mark_hovered_line(ui, &response, &galley, palette.text);
                        }
                        response
                    })
                    .inner;
                let rect = response.rect;
                if lyrics.synced
                    && response.clicked()
                    && let Some(at_ms) = line.at_ms
                {
                    app.actions.push(Action::Seek(at_ms));
                    app.actions.push(Action::FollowLyrics);
                }
                if is_active && follow {
                    show_sung_line(ui, rect, Some(animation));
                }
                ui.add_space(27.0);
            }
            // Words without timing can only be followed by the clock: sit
            // at the part of the text the song is probably at.
            if following && !lyrics.synced && now.duration_ms > 0 {
                let fraction =
                    (f64::from(now.position_ms) / f64::from(now.duration_ms)).clamp(0.0, 1.0);
                let content = ui.min_rect();
                let y = content.top() + content.height() * fraction as f32;
                ui.scroll_to_rect_animation(
                    egui::Rect::from_min_max(
                        egui::pos2(content.left(), y),
                        egui::pos2(content.right(), y + 1.0),
                    ),
                    Some(Align::Center),
                    animation,
                );
            }
            ui.add_space(below.max(60.0));
        });
    // Scrolling by hand means the reader wants to look elsewhere; the
    // Follow button in the header picks the song back up.
    if manual_scroll && app.lyrics_following {
        app.actions.push(Action::PauseLyricsFollow);
    }
    if now.playing
        && lyrics.synced
        && let Some(next) = lyrics
            .lines
            .iter()
            .filter_map(|line| line.at_ms)
            .find(|at| *at > now.position_ms)
    {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(u64::from(
                next - now.position_ms,
            )));
    }
}

#[cfg(test)]
mod tests {
    use super::{fullscreen_content_width, preferred_backdrop_art};

    /// The title and artists come to the corner only once the bar has
    /// gone, and the bar comes back only once they have left it.
    #[test]
    fn corner_words_never_show_beside_the_fading_bar() {
        use super::{controls_opacity, corner_words_opacity};
        for step in 0..=100 {
            let hidden = step as f32 / 100.0;
            let bar = controls_opacity(hidden);
            let words = corner_words_opacity(hidden);
            assert!(bar == 0.0 || words == 0.0, "both show at {hidden}");
        }
        assert_eq!(controls_opacity(0.0), 1.0);
        assert_eq!(corner_words_opacity(1.0), 1.0);
    }

    #[test]
    fn fullscreen_backdrop_prefers_small_art_with_large_art_as_fallback() {
        let small = "small".to_string();
        let large = "large".to_string();
        assert_eq!(
            preferred_backdrop_art(Some(small.clone()), Some(large.clone())),
            Some(small)
        );
        assert_eq!(
            preferred_backdrop_art(None, Some(large.clone())),
            Some(large)
        );
        assert_eq!(preferred_backdrop_art(None, None), None);
    }

    #[test]
    fn fullscreen_content_width_never_inverts_a_narrow_viewport() {
        for viewport_width in [0.0, 24.0, 47.0, 48.0, 64.0, 760.0, 2_000.0] {
            let width = fullscreen_content_width(viewport_width);
            assert!(width >= 0.0);
            assert!(width <= (viewport_width - 48.0).max(0.0));
        }
        assert_eq!(fullscreen_content_width(47.0), 0.0);
        assert_eq!(fullscreen_content_width(2_000.0), 960.0);
    }
}
