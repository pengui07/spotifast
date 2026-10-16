//! The artist page.

use std::sync::Arc;

use crate::api::models::{Album, Artist, PlayableItem, pick_image};
use crate::app::App;
use crate::i18n::{gettext, ngettext, pgettext};
use crate::model::{Action, DiscographyFilter, Loadable, Page, RowContext};
use crate::theme::{self, Icon};
use crate::util;

use super::collection::{Hero, hero, hero_images};
use super::widgets::{self, TrackRow};

pub fn show(app: &mut App, ui: &mut egui::Ui, id: &str) {
    if !app.artist_pages.contains_key(id) {
        app.ensure_loaded(Page::Artist(id.to_string()));
    }
    let Some(page) = app.artist_pages.remove(id) else {
        return;
    };
    let preview =
        super::loading_preview(ui.ctx(), id, &page.artist, || app.known_artist(id).cloned());
    let palette = app.palette;
    let locale = app.locale;
    match &page.artist {
        Loadable::Loaded(artist) => {
            artist_hero(app, ui, artist, preview.as_deref());
            artist_actions(app, ui, artist);

            // Popular.
            theme::section_title(ui, &palette, &gettext(locale, "Popular"));
            ui.add_space(4.0);
            match &page.top_tracks {
                Loadable::Loaded(tracks) if !tracks.is_empty() => {
                    let uris: Arc<[String]> = tracks
                        .iter()
                        .map(|track| track.uri.clone())
                        .collect::<Vec<_>>()
                        .into();
                    let context = RowContext::Uris(Arc::clone(&uris));
                    let items: Vec<PlayableItem> =
                        tracks.iter().cloned().map(PlayableItem::Track).collect();
                    let limit = if page.show_all_top { items.len() } else { 5 };
                    for (index, item) in items.iter().take(limit).enumerate() {
                        widgets::track_row(
                            ui,
                            app,
                            TrackRow {
                                index,
                                number: Some(index + 1),
                                item,
                                context: &context,
                                show_cover: !app.settings.tracklist_compact,
                                show_album: false,
                                added_at: None,
                                added_by: None,
                                show_added_by: false,
                                compact: false,
                                thin: app.settings.tracklist_compact,
                                shift: 0.0,
                                picked: false,
                                picked_songs: &[],
                            },
                        );
                    }
                    if items.len() > 5 {
                        ui.add_space(6.0);
                        if theme::soft_button(
                            ui,
                            &palette,
                            None,
                            &if page.show_all_top {
                                gettext(locale, "Show less")
                            } else {
                                gettext(locale, "See more")
                            },
                            false,
                        )
                        .clicked()
                        {
                            app.actions.push(Action::ToggleShowAllTop(id.to_string()));
                        }
                    }
                }
                Loadable::Loaded(_) => {
                    theme::subtle(ui, &palette, &gettext(locale, "No popular songs to show."));
                }
                Loadable::Loading | Loadable::NotLoaded => {
                    widgets::loading_row(ui, &palette, app.locale)
                }
                Loadable::Failed(error) => {
                    let error = error.clone();
                    widgets::error_row(ui, app, &error, None);
                }
            }
            ui.add_space(20.0);

            // Discography preview, ranked across the complete catalogue.
            theme::section_title(ui, &palette, &gettext(locale, "Discography"));
            ui.add_space(6.0);
            match &page.discography {
                Loadable::Loaded(releases) => {
                    let albums = ordered_releases(releases, DiscographyFilter::All, true);
                    if albums.is_empty() {
                        theme::subtle(ui, &palette, &gettext(locale, "Nothing in this category."));
                    }
                    release_grid(app, ui, albums.iter().take(5).copied());
                    if !albums.is_empty()
                        && theme::soft_button(
                            ui,
                            &palette,
                            None,
                            &gettext(locale, "Show more"),
                            false,
                        )
                        .clicked()
                    {
                        app.actions.push(Action::SetDiscographyFilter {
                            artist_id: id.to_string(),
                            filter: DiscographyFilter::All,
                        });
                        app.actions
                            .push(Action::Open(Page::Discography(id.to_string())));
                    }
                }
                Loadable::Failed(error) => {
                    widgets::error_row(ui, app, error, Some(Page::Artist(id.to_string())))
                }
                _ => widgets::loading_row(ui, &palette, locale),
            }
            ui.add_space(20.0);
            // Related.
            if let Loadable::Loaded(related) = &page.related
                && !related.is_empty()
            {
                let artist_label = gettext(locale, "Artist");
                let title = gettext(locale, "Fans also like");
                let fade_edge = app.settings.faithful_visuals;
                widgets::shelf(ui, &palette, fade_edge, "related", &title, |ui| {
                    for artist in related {
                        let playing_here = app.playing_context_uri().as_deref()
                            == Some(artist.uri.as_str())
                            && app.believed_playing();
                        let card = widgets::card(
                            ui,
                            app,
                            pick_image(&artist.images, 640),
                            &artist.name,
                            &artist_label,
                            widgets::CardCover::portrait(playing_here),
                        );
                        if card.play {
                            if playing_here {
                                app.actions.push(Action::TogglePlay);
                            } else {
                                app.actions.push(Action::PlayContext {
                                    uri: artist.uri.clone(),
                                    offset_uri: None,
                                    offset_index: None,
                                });
                            }
                        }
                        if card.clicked {
                            app.actions
                                .push(Action::Open(Page::Artist(artist.id.clone())));
                        }
                        egui::Popup::context_menu(&card.response)
                            .id(ui.make_persistent_id(("related-artist-menu", &artist.uri)))
                            .frame(widgets::menu_frame(&palette))
                            .show(|ui| {
                                widgets::context_menu_items(
                                    ui,
                                    app,
                                    &artist.uri,
                                    &artist.name,
                                    None,
                                )
                            });
                    }
                });
            }
        }
        Loadable::Loading | Loadable::NotLoaded => {
            if let Some(artist) = &preview {
                artist_hero(app, ui, artist, None);
                ui.add_enabled_ui(false, |ui| artist_actions(app, ui, artist));
            } else {
                ui.add_space(40.0);
            }
            widgets::loading_row(ui, &palette, app.locale);
        }
        Loadable::Failed(error) => {
            let error = error.clone();
            if let Some(artist) = &preview {
                artist_hero(app, ui, artist, None);
                ui.add_enabled_ui(false, |ui| artist_actions(app, ui, artist));
            } else {
                ui.add_space(40.0);
            }
            widgets::error_row(ui, app, &error, Some(Page::Artist(id.to_string())));
        }
    }
    app.artist_pages.insert(id.to_string(), page);
}

/// A separate navigation destination, so Back returns to the artist overview.
pub fn discography(app: &mut App, ui: &mut egui::Ui, id: &str) {
    if !app.artist_pages.contains_key(id) {
        app.ensure_loaded(Page::Discography(id.to_string()));
    }
    let Some(page) = app.artist_pages.remove(id) else {
        return;
    };
    let palette = app.palette;
    let locale = app.locale;
    if let Some(artist) = page.artist.get() {
        theme::text(ui, &artist.name, theme::bold(30.0), palette.text);
    }
    theme::section_title(ui, &palette, &gettext(locale, "Discography"));
    let labels: Vec<_> = DiscographyFilter::ALL
        .iter()
        .map(|filter| (*filter, filter.label(locale)))
        .collect();
    let options: Vec<_> = labels
        .iter()
        .map(|(filter, label)| (*filter, label.as_ref()))
        .collect();
    if let Some(filter) = widgets::chips(ui, &palette, &options, page.filter) {
        app.actions.push(Action::SetDiscographyFilter {
            artist_id: id.to_string(),
            filter,
        });
    }
    ui.add_space(10.0);
    if page.filter == DiscographyFilter::AppearsOn {
        if let Some(list) = page.albums.get(page.filter.groups()) {
            release_grid(app, ui, ordered_releases(&list.items, page.filter, false));
            if list.loading {
                widgets::loading_row(ui, &palette, locale);
            } else if let Some(error) = &list.error {
                widgets::error_row(ui, app, error, Some(Page::Discography(id.to_string())));
            } else if list.can_load_more() {
                ui.add_space(8.0);
                if theme::soft_button(ui, &palette, None, &gettext(locale, "Load more"), false)
                    .clicked()
                {
                    app.actions
                        .push(Action::LoadMoreArtistAlbums(id.to_string()));
                }
            } else if list.items.is_empty() {
                theme::subtle(ui, &palette, &gettext(locale, "Nothing in this category."));
            }
        } else {
            widgets::loading_row(ui, &palette, locale);
        }
    } else {
        match &page.discography {
            Loadable::Loaded(releases) => {
                let albums = ordered_releases(releases, page.filter, false);
                if albums.is_empty() {
                    theme::subtle(ui, &palette, &gettext(locale, "Nothing in this category."));
                }
                release_grid(app, ui, albums);
            }
            Loadable::Failed(error) => {
                widgets::error_row(ui, app, error, Some(Page::Discography(id.to_string())))
            }
            _ => widgets::loading_row(ui, &palette, locale),
        }
    }
    app.artist_pages.insert(id.to_string(), page);
}

fn ordered_releases(albums: &[Album], filter: DiscographyFilter, popular: bool) -> Vec<&Album> {
    let mut albums: Vec<_> = albums
        .iter()
        .filter(|album| {
            let group = album.album_group.as_deref().or(album.album_type.as_deref());
            match filter {
                DiscographyFilter::All | DiscographyFilter::AppearsOn => true,
                DiscographyFilter::Albums => group == Some("album"),
                DiscographyFilter::Singles => group == Some("single"),
            }
        })
        .collect();
    albums.sort_by(|a, b| {
        let date = release_date(b).cmp(&release_date(a));
        if popular {
            b.popularity.cmp(&a.popularity).then(date)
        } else {
            date
        }
        .then_with(|| a.name.cmp(&b.name))
        .then_with(|| a.id.cmp(&b.id))
    });
    // Choose the best-ranked edition before collapsing duplicate titles.
    let mut seen = std::collections::HashSet::new();
    albums.retain(|album| seen.insert(album.name.to_lowercase()));
    albums
}

fn release_date(album: &Album) -> Option<jiff::civil::Date> {
    let date = album.release_date.as_deref()?;
    match date.len() {
        4 => format!("{date}-01-01").parse().ok(),
        7 => format!("{date}-01").parse().ok(),
        _ => date.parse().ok(),
    }
}

fn release_grid<'a>(app: &mut App, ui: &mut egui::Ui, albums: impl IntoIterator<Item = &'a Album>) {
    let palette = app.palette;
    widgets::grid(ui, |ui| {
        for album in albums {
            let subtitle = format!(
                "{} • {}",
                album.year().unwrap_or(""),
                app.album_kind_label(album)
            );
            let playing_here = app.playing_context_uri().as_deref() == Some(album.uri.as_str())
                && app.believed_playing();
            let card = widgets::card(
                ui,
                app,
                pick_image(&album.images, 640),
                &album.name,
                subtitle.trim_start_matches(" • "),
                widgets::CardCover::square(playing_here),
            );
            if card.play {
                app.actions.push(if playing_here {
                    Action::TogglePlay
                } else {
                    Action::PlayContext {
                        uri: album.uri.clone(),
                        offset_uri: None,
                        offset_index: None,
                    }
                });
            }
            if card.clicked {
                app.actions
                    .push(Action::Open(Page::Album(album.id.clone())));
            }
            egui::Popup::context_menu(&card.response)
                .id(ui.make_persistent_id(("discography-menu", &album.uri)))
                .frame(widgets::menu_frame(&palette))
                .show(|ui| widgets::context_menu_items(ui, app, &album.uri, &album.name, None));
        }
    });
}

fn artist_hero(app: &mut App, ui: &mut egui::Ui, artist: &Artist, preview: Option<&Artist>) {
    let locale = app.locale;
    let mut byline = Vec::new();
    if let Some(followers) = &artist.followers {
        byline.push((
            ngettext(
                locale,
                // Translators: {count} is the number of people who follow an artist.
                "{count} follower",
                "{count} followers",
                u32::try_from(followers.total).unwrap_or(u32::MAX),
            )
            .replace("{count}", &util::format_count(followers.total)),
            None,
        ));
    }
    let images = hero_images(
        &artist.images,
        preview.map(|artist| artist.images.as_slice()),
        false,
    );
    hero(
        app,
        ui,
        Hero {
            images,
            liked: false,
            kind: gettext(locale, "Artist"),
            title: &artist.name,
            description: None,
            byline,
            round: true,
        },
    );
}

fn artist_actions(app: &mut App, ui: &mut egui::Ui, artist: &Artist) {
    let palette = app.palette;
    let locale = app.locale;
    let following = app.is_saved(&artist.uri).unwrap_or(false);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 18.0;
        if app.play_pending(&artist.uri) {
            theme::circle_spinner(
                ui,
                56.0,
                palette.accent,
                palette.on_accent,
                &gettext(locale, "Starting…"),
            );
        } else if theme::circle_button(
            ui,
            Icon::PlayFilled,
            56.0,
            palette.accent,
            palette.accent_hover,
            palette.on_accent,
            &gettext(locale, "Play"),
        )
        .clicked()
        {
            app.actions.push(Action::PlayContext {
                uri: artist.uri.clone(),
                offset_uri: None,
                offset_index: None,
            });
        }
        let shuffle = app.playing_context_shuffle();
        if theme::icon_button(
            ui,
            Icon::Shuffle,
            26.0,
            if shuffle {
                palette.accent
            } else {
                palette.secondary
            },
            palette.text,
            &if shuffle {
                gettext(locale, "Shuffle off")
            } else {
                gettext(locale, "Shuffle")
            },
        )
        .clicked()
        {
            app.actions.push(Action::SetShuffle(!shuffle));
        }
        if theme::pill_button(
            ui,
            &palette,
            &if following {
                pgettext(locale, "artist", "Following")
            } else {
                pgettext(locale, "artist", "Follow")
            },
            false,
        )
        .clicked()
        {
            app.actions.push(Action::ToggleSaved(artist.uri.clone()));
        }
        let more = theme::icon_button(
            ui,
            Icon::Ellipsis,
            26.0,
            palette.secondary,
            palette.text,
            &gettext(locale, "More"),
        );
        egui::Popup::menu(&more)
            .frame(widgets::menu_frame(&palette))
            .show(|ui| widgets::context_menu_items(ui, app, &artist.uri, &artist.name, None));
    });
    ui.add_space(20.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(
        id: &str,
        name: &str,
        date: Option<&str>,
        popularity: Option<u8>,
        group: &str,
    ) -> Album {
        Album {
            id: id.into(),
            name: name.into(),
            release_date: date.map(str::to_string),
            popularity,
            album_group: Some(group.into()),
            ..Default::default()
        }
    }

    #[test]
    fn popularity_preview_chooses_the_best_edition_before_limiting_to_five() {
        let albums = vec![
            release("new", "New", Some("2026-10-15"), Some(10), "album"),
            release("old", "Old", Some("2000"), Some(90), "album"),
            release("single", "Single", Some("2024"), Some(80), "single"),
            release("duplicate", "OLD", Some("2026"), Some(50), "album"),
            release("four", "Four", Some("2020"), Some(70), "album"),
            release("five", "Five", Some("2021"), Some(60), "album"),
            release("unknown", "Unknown", Some("2026"), None, "album"),
        ];
        let preview = ordered_releases(&albums, DiscographyFilter::All, true);
        assert_eq!(
            preview
                .iter()
                .take(5)
                .map(|album| album.id.as_str())
                .collect::<Vec<_>>(),
            ["old", "single", "four", "five", "new"]
        );
        assert_eq!(preview.last().unwrap().id, "unknown");
        assert_eq!(
            albums[0].id, "new",
            "sorting must not rewrite the source list"
        );
    }

    #[test]
    fn discography_orders_full_dates_and_partial_dates_with_unknown_dates_last() {
        let albums = vec![
            release("year", "Year", Some("2024"), Some(99), "album"),
            release("day", "Day", Some("2024-08-20"), Some(1), "album"),
            release("month", "Month", Some("2024-08"), Some(80), "single"),
            release("missing", "Missing", None, Some(100), "album"),
            release("invalid", "Invalid", Some("unknown"), Some(100), "album"),
        ];
        let sorted = ordered_releases(&albums, DiscographyFilter::All, false);
        assert_eq!(
            sorted
                .iter()
                .take(3)
                .map(|album| album.id.as_str())
                .collect::<Vec<_>>(),
            ["day", "month", "year"]
        );
        assert_eq!(
            ordered_releases(&albums, DiscographyFilter::Singles, false)[0].id,
            "month"
        );
        assert_eq!(
            ordered_releases(&albums, DiscographyFilter::Albums, false).len(),
            4
        );
        let page = Page::Discography("artist".into());
        assert_eq!(Page::decode(&page.encode()), Some(page));
    }
}
