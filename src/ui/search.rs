//! The Search page.

use std::sync::Arc;

use egui::{Align, CornerRadius, Layout, Rect, Sense, Vec2, pos2, vec2};

use crate::api::models::{Artist, ArtistRef, PlayableItem, SearchResults, pick_image};
use crate::app::App;
use crate::i18n::gettext;
use crate::model::{Action, Loadable, Page, RowContext, SearchFilter};
use crate::settings::{LIKED_SONGS_KEY, RecentSearch};
use crate::theme::{self, Icon};

use super::widgets::{self, TrackRow};

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    let palette = app.palette;
    if app.search.committed.is_empty() && app.search.typed_at.is_none() {
        recent(app, ui);
        return;
    }
    ui.add_space(4.0);
    let labels: Vec<_> = SearchFilter::ALL
        .iter()
        .map(|f| (*f, f.label(app.locale)))
        .collect();
    let options: Vec<(SearchFilter, &str)> = labels
        .iter()
        .map(|(filter, label)| (*filter, label.as_ref()))
        .collect();
    if let Some(filter) = widgets::chips(ui, &palette, &options, app.search.filter) {
        app.actions.push(Action::SetSearchFilter(filter));
    }
    ui.add_space(12.0);
    let pending = app.search.catalogue_pending || app.search.playlists_pending;
    if pending {
        widgets::loading_row(ui, &palette, app.locale);
    }
    if let Some(error) = app.search.error.clone() {
        widgets::error_row(ui, app, &error, None);
    }
    let results = match &app.search.results {
        Loadable::Loaded(results) => results.clone(),
        Loadable::Loading | Loadable::NotLoaded => {
            if !pending {
                widgets::loading_row(ui, &palette, app.locale);
            }
            return;
        }
        Loadable::Failed(error) => {
            let error = error.clone();
            if app.search.error.is_none() {
                widgets::error_row(ui, app, &error, None);
            }
            return;
        }
    };
    if results.is_empty() && (pending || app.search.error.is_some()) {
        return;
    }
    if results.is_empty() {
        widgets::empty_state(
            ui,
            &palette,
            Icon::Search,
            // Translators: {query} is the text the user searched for.
            &gettext(app.locale, "No results for “{query}”")
                .replace("{query}", &app.search.committed),
            &gettext(app.locale, "Check the spelling, or try fewer words."),
        );
        return;
    }
    match app.search.filter {
        SearchFilter::All => all(app, ui, &results),
        SearchFilter::Songs => songs(app, ui, &results, usize::MAX),
        SearchFilter::Artists => artists_grid(app, ui, &results),
        SearchFilter::Albums => albums_grid(app, ui, &results),
        SearchFilter::Playlists => playlists_grid(app, ui, &results),
        SearchFilter::Podcasts => shows_grid(app, ui, &results),
        SearchFilter::Episodes => episodes(app, ui, &results, usize::MAX),
    }
}

fn recent(app: &mut App, ui: &mut egui::Ui) {
    let palette = app.palette;
    let locale = app.locale;
    ui.add_space(6.0);
    if app.settings.recent_searches.is_empty() {
        widgets::empty_state(
            ui,
            &palette,
            Icon::Search,
            &gettext(app.locale, "Search Spotify"),
            &gettext(
                app.locale,
                "Find songs, artists, albums, playlists, and podcasts.",
            ),
        );
        return;
    }
    ui.horizontal(|ui| {
        theme::section_title(ui, &palette, &gettext(locale, "Recent searches"));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if theme::soft_button(ui, &palette, None, &gettext(locale, "Clear"), false).clicked() {
                app.actions.push(Action::ClearRecentSearches);
            }
        });
    });
    ui.add_space(10.0);
    // The page is wide and the rows are short, so they flow into as many
    // columns as fit, newest first along each line.
    const GAP: f32 = 12.0;
    const MIN_COLUMN: f32 = 320.0;
    let width = ui.available_width();
    let columns = ((width + GAP) / (MIN_COLUMN + GAP)).floor().clamp(1.0, 3.0) as usize;
    let column = (width - GAP * (columns - 1) as f32) / columns as f32;
    let recents = app.settings.recent_searches.clone();
    for line in recents.chunks(columns) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            for entry in line {
                recent_row(app, ui, entry, column);
            }
        });
        ui.add_space(4.0);
    }
}

/// What a recent result's second line says: its kind, then who made it.
fn recent_subtitle(locale: crate::i18n::Locale, entry: &RecentSearch) -> String {
    let with = |text: std::borrow::Cow<'_, str>, key: &str| text.replace(key, &entry.detail);
    match entry.kind() {
        "track" => format!("{}{}", gettext(locale, "Song • "), entry.detail),
        "artist" => gettext(locale, "Artist").into_owned(),
        "album" => with(gettext(locale, "Album • {artists}"), "{artists}"),
        "playlist" => with(gettext(locale, "Playlist • {owner}"), "{owner}"),
        "show" => with(gettext(locale, "Podcast • {publisher}"), "{publisher}"),
        // Translators: {podcast} is the name of the podcast the episode is from.
        "episode" => with(gettext(locale, "Episode • {podcast}"), "{podcast}"),
        "liked-songs" => gettext(locale, "Playlist").into_owned(),
        _ => entry.detail.clone(),
    }
    .trim_end_matches([' ', '•'])
    .to_string()
}

/// One result the listener opened or played from an earlier search. A song
/// or an episode plays again; anything else opens its page.
fn recent_row(app: &mut App, ui: &mut egui::Ui, entry: &RecentSearch, width: f32) {
    let palette = app.palette;
    let subtitle = recent_subtitle(app.locale, entry);
    let kind = entry.kind();
    let liked = kind == "liked-songs";
    let name = if liked {
        gettext(app.locale, "Liked Songs").into_owned()
    } else {
        entry.name.clone()
    };
    let (rect, response) = ui.allocate_exact_size(vec2(width, 64.0), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            ui.is_enabled(),
            format!("{name}, {subtitle}"),
        )
    });
    // Claimed after the row, so the cross keeps its own click.
    let cross_rect = Rect::from_center_size(
        pos2(rect.right() - 26.0, rect.center().y),
        Vec2::splat(32.0),
    );
    let cross = ui
        .interact(cross_rect, response.id.with("forget"), Sense::click())
        .on_hover_text(gettext(app.locale, "Remove"));
    cross.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            ui.is_enabled(),
            format!("Remove {name}"),
        )
    });
    let context = app.playing_context_uri();
    let playing = app.believed_playing()
        && (app.current_track_uri().as_deref() == Some(entry.uri.as_str())
            || context.as_deref() == Some(entry.uri.as_str())
            || liked && context.is_some_and(|context| context.ends_with(":collection")));
    if ui.is_rect_visible(rect) {
        let hovered = ui.rect_contains_pointer(rect) || response.has_focus() || cross.has_focus();
        if hovered {
            ui.painter().rect_filled(
                rect,
                CornerRadius::same(theme::RADIUS),
                palette.surface_hover,
            );
        }
        let round = kind == "artist";
        let cover = Rect::from_min_size(
            pos2(rect.left() + 8.0, rect.center().y - 24.0),
            Vec2::splat(48.0),
        );
        widgets::paint_cover(
            ui,
            &palette,
            entry.image.as_deref(),
            cover,
            if round { 24.0 } else { 4.0 },
            if round { Icon::User } else { Icon::Music },
            Some(app.backend.art()),
        );
        let text_left = cover.right() + 12.0;
        let text_width = (cross_rect.left() - 8.0 - text_left).max(0.0);
        let title_color = if playing {
            palette.accent
        } else {
            palette.text
        };
        let title =
            widgets::ellipsized(ui, &name, theme::semibold(14.5), title_color, text_width, 1);
        let detail = widgets::ellipsized(
            ui,
            &subtitle,
            theme::regular(13.0),
            palette.secondary,
            text_width,
            1,
        );
        let gap = 3.0;
        let top = rect.center().y - (title.size().y + gap + detail.size().y) / 2.0;
        ui.painter()
            .galley(pos2(text_left, top), title.clone(), title_color);
        ui.painter().galley(
            pos2(text_left, top + title.size().y + gap),
            detail,
            palette.secondary,
        );
        if hovered {
            let color = if cross.hovered() {
                palette.text
            } else {
                palette.secondary
            };
            Icon::X.image(color, 16.0).paint_at(
                ui,
                Rect::from_center_size(cross_rect.center(), Vec2::splat(16.0)),
            );
        } else if playing {
            theme::paint_playing_bars(
                ui,
                Rect::from_center_size(cross_rect.center(), Vec2::splat(16.0)),
                16.0,
                palette.accent,
            );
        }
    }
    if cross.clicked() {
        app.actions.push(Action::ForgetSearch(entry.uri.clone()));
    } else if response.clicked() {
        let uri = entry.uri.clone();
        let action = match kind {
            "track" => Some(Action::PlayUris {
                uris: vec![uri],
                index: 0,
            }),
            "episode" => Some(Action::PlayEpisode {
                uri,
                resume_ms: None,
            }),
            "liked-songs" => Some(Action::Open(Page::LikedSongs)),
            _ => Page::from_uri(&uri).map(Action::Open),
        };
        app.actions.extend(action);
    }
    if !matches!(kind, "track" | "episode" | "liked-songs") {
        egui::Popup::context_menu(&response)
            .id(ui.make_persistent_id(("recent-search-menu", &entry.uri)))
            .frame(widgets::menu_frame(&palette))
            .show(|ui| {
                widgets::context_menu_items(ui, app, &entry.uri, &entry.name, None);
            });
    }
}

fn all(app: &mut App, ui: &mut egui::Ui, results: &SearchResults) {
    let palette = app.palette;
    let locale = app.locale;
    let owned = library_pick(&app.library, locale, &app.search.committed);
    let top = owned.as_ref().map(Owned::top).or_else(|| {
        top_pick(results, &app.search.committed, |uri| {
            app.is_saved(uri) == Some(true)
        })
    });
    let wide = ui.available_width() > 720.0;
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 24.0;
        let top_width = if wide {
            (ui.available_width() * 0.36).clamp(240.0, 380.0)
        } else {
            ui.available_width()
        };
        ui.vertical(|ui| {
            ui.set_width(top_width);
            theme::section_title(ui, &palette, &gettext(locale, "Top result"));
            ui.add_space(4.0);
            if let Some(Top::Artist(artist)) = top {
                top_result(
                    app,
                    ui,
                    pick_image(&artist.images, 640),
                    &artist.name,
                    TopResultSubtitle::Text(&gettext(locale, "Artist")),
                    true,
                    Some(artist.uri.clone()),
                    Page::Artist(artist.id.clone()),
                    |ui, app| {
                        widgets::context_menu_items(ui, app, &artist.uri, &artist.name, None);
                    },
                );
            } else if let Some(Top::Song(track)) = top {
                let page = track
                    .album
                    .as_ref()
                    .map(|album| Page::Album(album.id.clone()))
                    .unwrap_or(Page::Search);
                top_result(
                    app,
                    ui,
                    track.image(640),
                    &track.name,
                    TopResultSubtitle::SongArtists(&track.artists),
                    false,
                    Some(track.uri.clone()),
                    page,
                    |ui, app| {
                        widgets::item_menu(
                            ui,
                            app,
                            &PlayableItem::Track(track.clone()),
                            None,
                            None,
                        );
                    },
                );
            } else if let Some(Top::Album(album)) = top {
                top_result(
                    app,
                    ui,
                    pick_image(&album.images, 640),
                    &album.name,
                    TopResultSubtitle::Text(
                        // Translators: {artists} is the album's artist names.
                        &gettext(locale, "Album • {artists}").replace(
                            "{artists}",
                            &crate::api::models::join_names(
                                album.artists.iter().map(|a| a.name.as_str()),
                            ),
                        ),
                    ),
                    false,
                    Some(album.uri.clone()),
                    Page::Album(album.id.clone()),
                    |ui, app| {
                        widgets::context_menu_items(ui, app, &album.uri, &album.name, None);
                    },
                );
            } else if let Some(Top::Playlist(playlist)) = top {
                top_result(
                    app,
                    ui,
                    pick_image(&playlist.images, 640),
                    &playlist.name,
                    TopResultSubtitle::Text(
                        // Translators: {owner} is the name of the playlist's owner.
                        &gettext(locale, "Playlist • {owner}")
                            .replace("{owner}", playlist.owner_name()),
                    ),
                    false,
                    Some(playlist.uri.clone()),
                    Page::Playlist(playlist.id.clone()),
                    |ui, app| {
                        let owned = app.user_id().is_some_and(|id| playlist.owned_by(id));
                        widgets::context_menu_items(
                            ui,
                            app,
                            &playlist.uri,
                            &playlist.name,
                            owned.then_some(playlist),
                        );
                    },
                );
            } else if let Some(Top::Podcast(show)) = top {
                top_result(
                    app,
                    ui,
                    pick_image(&show.images, 640),
                    &show.name,
                    TopResultSubtitle::Text(
                        // Translators: {publisher} is the podcast's publisher.
                        &gettext(locale, "Podcast • {publisher}")
                            .replace("{publisher}", &show.publisher),
                    ),
                    false,
                    Some(show.uri.clone()),
                    Page::Show(show.id.clone()),
                    |ui, app| {
                        widgets::context_menu_items(ui, app, &show.uri, &show.name, None);
                    },
                );
            } else if let Some(Top::LikedSongs) = top {
                let subtitle = liked_songs_subtitle(app);
                top_result(
                    app,
                    ui,
                    Some(LIKED_SONGS_KEY),
                    &gettext(locale, "Liked Songs"),
                    TopResultSubtitle::Text(&subtitle),
                    false,
                    liked_songs_uri(app),
                    Page::LikedSongs,
                    |_, _| {},
                );
            }
        });
        if wide {
            ui.vertical(|ui| {
                ui.set_width(ui.available_width());
                songs(app, ui, results, 4);
            });
        }
    });
    if !wide {
        ui.add_space(12.0);
        songs(app, ui, results, 4);
    }
    ui.add_space(8.0);
    shelf_artists(app, ui, results);
    shelf_albums(app, ui, results);
    shelf_playlists(app, ui, results);
    shelf_shows(app, ui, results);
    if results
        .episodes
        .as_ref()
        .is_some_and(|page| !page.items.is_empty())
    {
        theme::section_title(ui, &palette, &gettext(locale, "Episodes"));
        ui.add_space(4.0);
        episodes(app, ui, results, 4);
    }
}

/// What the Top result card shows.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Top<'a> {
    Artist(&'a Artist),
    Album(&'a crate::api::models::Album),
    Song(&'a crate::api::models::Track),
    Playlist(&'a crate::api::models::Playlist),
    Podcast(&'a crate::api::models::Show),
    LikedSongs,
}

/// A name reduced to its letters and digits, so "Keep It Like a Secret"
/// and "keep it like a secret!" compare equal.
pub(crate) fn search_key(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// How far down each kind's list a result from the listener's library can
/// still become the Top result; Spotify already ranks each list.
const LIBRARY_REACH: usize = 5;

/// Spotify answers each kind in its own list, with no best match across
/// them, so the Top result is chosen here, the way Spotify's own app ranks:
/// something already in the listener's library near the top of its list,
/// else a name that is exactly what was searched, else the first song. Ties
/// go to an artist, then an album, a song, a playlist, a podcast.
fn top_pick<'a>(
    results: &'a SearchResults,
    query: &str,
    saved: impl Fn(&str) -> bool,
) -> Option<Top<'a>> {
    let key = search_key(query);
    let exact = |name: &str| !key.is_empty() && search_key(name) == key;
    first_top(results, LIBRARY_REACH, |uri, _| saved(uri))
        .or_else(|| first_top(results, usize::MAX, |_, name| exact(name)))
        .or_else(|| fallback_top(results))
}

/// The first result, kind by kind, among each list's first `reach`, that
/// `hit` accepts by URI and name.
fn first_top<'a>(
    results: &'a SearchResults,
    reach: usize,
    hit: impl Fn(&str, &str) -> bool,
) -> Option<Top<'a>> {
    fn items<T>(
        page: &Option<crate::api::models::Page<T>>,
        reach: usize,
    ) -> impl Iterator<Item = &T> {
        page.as_ref()
            .map_or(&[][..], |page| page.items.as_slice())
            .iter()
            .take(reach)
    }
    items(&results.artists, reach)
        .find(|artist| hit(&artist.uri, &artist.name))
        .map(Top::Artist)
        .or_else(|| {
            items(&results.albums, reach)
                .find(|album| hit(&album.uri, &album.name))
                .map(Top::Album)
        })
        .or_else(|| {
            items(&results.tracks, reach)
                .find(|track| hit(&track.uri, &track.name))
                .map(Top::Song)
        })
        .or_else(|| {
            items(&results.playlists, reach)
                .find(|playlist| hit(&playlist.uri, &playlist.name))
                .map(Top::Playlist)
        })
        .or_else(|| {
            items(&results.shows, reach)
                .find(|show| hit(&show.uri, &show.name))
                .map(Top::Podcast)
        })
}

/// With nothing saved or named as searched: the first song, or else the
/// first of whatever came back.
fn fallback_top(results: &SearchResults) -> Option<Top<'_>> {
    results
        .tracks
        .as_ref()
        .and_then(|page| page.items.first())
        .map(Top::Song)
        .or_else(|| first_top(results, 1, |_, _| true))
}

/// Something from the listener's library, which a search may not return.
enum Owned {
    Artist(Artist),
    Album(crate::api::models::Album),
    Playlist(crate::api::models::Playlist),
    Podcast(crate::api::models::Show),
    LikedSongs,
}

impl Owned {
    fn top(&self) -> Top<'_> {
        match self {
            Self::Artist(artist) => Top::Artist(artist),
            Self::Album(album) => Top::Album(album),
            Self::Playlist(playlist) => Top::Playlist(playlist),
            Self::Podcast(show) => Top::Podcast(show),
            Self::LikedSongs => Top::LikedSongs,
        }
    }
}

/// Spotify's own app looks in the listener's library as they type: "dia"
/// finds a saved album called Diary that the Web API search leaves out, and
/// "fire" finds a saved Firewater even when the search returns another
/// release of it. A followed artist, then a saved album, playlist or
/// podcast whose name is what was typed, or else starts with it, wins.
fn library_pick(
    library: &crate::model::Library,
    locale: crate::i18n::Locale,
    query: &str,
) -> Option<Owned> {
    let key = search_key(query);
    // One letter starts the name of half the library.
    if key.chars().count() < 2 {
        return None;
    }
    let playlists = library.playlists.get().map_or(&[][..], Vec::as_slice);
    let liked = gettext(locale, "Liked Songs");
    let find = |hit: &dyn Fn(&str) -> bool| {
        library
            .artists
            .items
            .iter()
            .find(|artist| hit(&artist.name))
            .cloned()
            .map(Owned::Artist)
            .or_else(|| {
                library
                    .albums
                    .items
                    .iter()
                    .map(|saved| &saved.album)
                    .find(|album| hit(&album.name))
                    .cloned()
                    .map(Owned::Album)
            })
            .or_else(|| {
                // As in Spotify, Liked Songs answers from three letters on.
                (key.chars().count() >= 3 && (hit(&liked) || hit("Liked Songs")))
                    .then_some(Owned::LikedSongs)
            })
            .or_else(|| {
                playlists
                    .iter()
                    .find(|playlist| hit(&playlist.name))
                    .cloned()
                    .map(Owned::Playlist)
            })
            .or_else(|| {
                library
                    .shows
                    .items
                    .iter()
                    .map(|saved| &saved.show)
                    .find(|show| hit(&show.name))
                    .cloned()
                    .map(Owned::Podcast)
            })
    };
    find(&|name| search_key(name) == key)
        .or_else(|| find(&|name| search_key(name).starts_with(&key)))
}

enum TopResultSubtitle<'a> {
    Text(&'a str),
    SongArtists(&'a [ArtistRef]),
}

#[allow(clippy::too_many_arguments)]
fn top_result(
    app: &mut App,
    ui: &mut egui::Ui,
    image: Option<&str>,
    title: &str,
    subtitle: TopResultSubtitle<'_>,
    round: bool,
    play_uri: Option<String>,
    page: Page,
    menu: impl FnOnce(&mut egui::Ui, &mut App),
) {
    let palette = app.palette;
    let mut subtitle_clicked = false;
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), 232.0), Sense::click());
    if ui.is_rect_visible(rect) {
        let hovered = ui.rect_contains_pointer(rect);
        let fill = if hovered {
            palette.surface_hover
        } else {
            palette.surface
        };
        ui.painter()
            .rect_filled(rect, CornerRadius::same(theme::RADIUS), fill);
        let image_rect = Rect::from_min_size(rect.min + vec2(20.0, 20.0), Vec2::splat(96.0));
        widgets::paint_shadow(ui, &palette, image_rect, if round { 48.0 } else { 6.0 });
        widgets::paint_cover(
            ui,
            &palette,
            image,
            image_rect,
            if round { 48.0 } else { 6.0 },
            if round { Icon::User } else { Icon::Music },
            Some(app.backend.art()),
        );
        let text_clip = Rect::from_min_max(
            pos2(rect.left() + 20.0, image_rect.bottom() + 12.0),
            pos2(rect.right() - 20.0, rect.bottom()),
        );
        let painter = ui.painter().with_clip_rect(text_clip);
        crate::bidi::paint_line(
            &painter,
            text_clip.left(),
            text_clip.right(),
            text_clip.top() + 16.0,
            title,
            theme::bold(26.0),
            palette.text,
        );
        match subtitle {
            TopResultSubtitle::SongArtists(artists) => {
                let subtitle_rect = Rect::from_min_max(
                    pos2(text_clip.left(), text_clip.top() + 36.0),
                    pos2(text_clip.right(), text_clip.top() + 56.0),
                );
                let mut child = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(subtitle_rect)
                        .layout(Layout::left_to_right(Align::Center)),
                );
                child.set_clip_rect(subtitle_rect.intersect(ui.clip_rect()));
                child.spacing_mut().item_spacing.x = 0.0;
                theme::text(
                    &mut child,
                    // Translators: Precedes the song's artist names, which follow as links.
                    gettext(app.locale, "Song • ").as_ref(),
                    theme::regular(13.5),
                    palette.secondary,
                );
                subtitle_clicked = widgets::artist_links(
                    &mut child,
                    app,
                    artists,
                    theme::regular(13.5),
                    palette.secondary,
                );
            }
            TopResultSubtitle::Text(subtitle) => {
                crate::bidi::paint_line(
                    &painter,
                    text_clip.left(),
                    text_clip.right(),
                    text_clip.top() + 46.0,
                    subtitle,
                    theme::regular(13.5),
                    palette.secondary,
                );
            }
        }
        if hovered && let Some(uri) = &play_uri {
            let button = Rect::from_center_size(
                pos2(rect.right() - 44.0, rect.bottom() - 44.0),
                Vec2::splat(48.0),
            );
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(button)
                    .layout(Layout::centered_and_justified(egui::Direction::LeftToRight)),
            );
            if theme::circle_button(
                &mut child,
                Icon::PlayFilled,
                48.0,
                palette.accent,
                palette.accent_hover,
                palette.on_accent,
                &gettext(app.locale, "Play"),
            )
            .clicked()
            {
                if uri.starts_with("spotify:track:") {
                    app.actions.push(Action::PlayUris {
                        uris: vec![uri.clone()],
                        index: 0,
                    });
                } else {
                    app.actions.push(Action::PlayContext {
                        uri: uri.clone(),
                        offset_uri: None,
                        offset_index: None,
                    });
                }
            }
        }
    }
    if response.clicked() && !subtitle_clicked && page != Page::Search {
        app.actions.push(Action::Open(page));
    }
    egui::Popup::context_menu(&response)
        .frame(widgets::menu_frame(&palette))
        .show(|ui| menu(ui, app));
}

fn songs(app: &mut App, ui: &mut egui::Ui, results: &SearchResults, limit: usize) {
    let palette = app.palette;
    let Some(page) = &results.tracks else {
        return;
    };
    if page.items.is_empty() {
        return;
    }
    theme::section_title(ui, &palette, &gettext(app.locale, "Songs"));
    ui.add_space(4.0);
    let uris: Arc<[String]> = page
        .items
        .iter()
        .map(|track| track.uri.clone())
        .collect::<Vec<_>>()
        .into();
    let context = RowContext::Uris(Arc::clone(&uris));
    let items: Vec<PlayableItem> = page
        .items
        .iter()
        .cloned()
        .map(PlayableItem::Track)
        .collect();
    for (index, item) in items.iter().take(limit).enumerate() {
        widgets::track_row(
            ui,
            app,
            TrackRow {
                index,
                number: None,
                item,
                context: &context,
                show_cover: true,
                show_album: limit == usize::MAX,
                added_at: None,
                added_by: None,
                show_added_by: false,
                compact: limit != usize::MAX,
                thin: false,
                shift: 0.0,
                picked: false,
                picked_songs: &[],
            },
        );
    }
}

fn artist_card(app: &mut App, ui: &mut egui::Ui, artist: &Artist) {
    let playing_here =
        app.playing_context_uri().as_deref() == Some(artist.uri.as_str()) && app.believed_playing();
    let card = widgets::card(
        ui,
        app,
        pick_image(&artist.images, 640),
        &artist.name,
        &gettext(app.locale, "Artist"),
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
        .id(ui.make_persistent_id(("search-artist-menu", &artist.uri)))
        .frame(widgets::menu_frame(&app.palette))
        .show(|ui| {
            widgets::context_menu_items(ui, app, &artist.uri, &artist.name, None);
        });
}

fn shelf_artists(app: &mut App, ui: &mut egui::Ui, results: &SearchResults) {
    let palette = app.palette;
    let Some(page) = &results.artists else { return };
    if page.items.is_empty() {
        return;
    }
    widgets::shelf(
        ui,
        &palette,
        "search-artists",
        &gettext(app.locale, "Artists"),
        |ui| {
            for artist in &page.items {
                artist_card(app, ui, artist);
            }
        },
    );
}

fn artists_grid(app: &mut App, ui: &mut egui::Ui, results: &SearchResults) {
    let Some(page) = &results.artists else { return };
    widgets::grid(ui, |ui| {
        for artist in &page.items {
            artist_card(app, ui, artist);
        }
    });
}

fn album_card(app: &mut App, ui: &mut egui::Ui, album: &crate::api::models::Album) {
    let subtitle = format!(
        "{} • {}",
        album.year().unwrap_or(""),
        crate::api::models::join_names(album.artists.iter().map(|a| a.name.as_str()))
    );
    let playing_here =
        app.playing_context_uri().as_deref() == Some(album.uri.as_str()) && app.believed_playing();
    let card = widgets::card(
        ui,
        app,
        pick_image(&album.images, 640),
        &album.name,
        subtitle.trim_start_matches(" • "),
        widgets::CardCover::square(playing_here),
    );
    if card.play {
        if playing_here {
            app.actions.push(Action::TogglePlay);
        } else {
            app.actions.push(Action::PlayContext {
                uri: album.uri.clone(),
                offset_uri: None,
                offset_index: None,
            });
        }
    }
    if card.clicked {
        app.actions
            .push(Action::Open(Page::Album(album.id.clone())));
    }
    egui::Popup::context_menu(&card.response)
        .id(ui.make_persistent_id(("search-album-menu", &album.uri)))
        .frame(widgets::menu_frame(&app.palette))
        .show(|ui| {
            widgets::context_menu_items(ui, app, &album.uri, &album.name, None);
        });
}

fn shelf_albums(app: &mut App, ui: &mut egui::Ui, results: &SearchResults) {
    let palette = app.palette;
    let Some(page) = &results.albums else { return };
    if page.items.is_empty() {
        return;
    }
    widgets::shelf(
        ui,
        &palette,
        "search-albums",
        &gettext(app.locale, "Albums"),
        |ui| {
            for album in &page.items {
                album_card(app, ui, album);
            }
        },
    );
}

fn albums_grid(app: &mut App, ui: &mut egui::Ui, results: &SearchResults) {
    let Some(page) = &results.albums else { return };
    widgets::grid(ui, |ui| {
        for album in &page.items {
            album_card(app, ui, album);
        }
    });
}

fn playlist_card(app: &mut App, ui: &mut egui::Ui, playlist: &crate::api::models::Playlist) {
    let playing_here = app.playing_context_uri().as_deref() == Some(playlist.uri.as_str())
        && app.believed_playing();
    let card = widgets::card(
        ui,
        app,
        pick_image(&playlist.images, 640),
        &playlist.name,
        // Translators: {owner} is the name of the playlist's owner.
        &gettext(app.locale, "By {owner}").replace("{owner}", playlist.owner_name()),
        widgets::CardCover::square(playing_here),
    );
    if card.play {
        if playing_here {
            app.actions.push(Action::TogglePlay);
        } else {
            app.actions.push(Action::PlayContext {
                uri: playlist.uri.clone(),
                offset_uri: None,
                offset_index: None,
            });
        }
    }
    if card.clicked {
        app.actions
            .push(Action::Open(Page::Playlist(playlist.id.clone())));
    }
    egui::Popup::context_menu(&card.response)
        .id(ui.make_persistent_id(("search-playlist-menu", &playlist.uri)))
        .frame(widgets::menu_frame(&app.palette))
        .show(|ui| {
            let owned = app.user_id().is_some_and(|id| playlist.owned_by(id));
            widgets::context_menu_items(
                ui,
                app,
                &playlist.uri,
                &playlist.name,
                owned.then_some(playlist),
            );
        });
}

/// Liked Songs plays the account's collection; it has no URI of its own.
fn liked_songs_uri(app: &App) -> Option<String> {
    app.user
        .as_ref()
        .map(|user| format!("spotify:user:{}:collection", user.id))
}

fn liked_songs_subtitle(app: &App) -> String {
    let owner = app
        .user
        .as_ref()
        .map(|user| user.display_name.clone().unwrap_or_else(|| user.id.clone()))
        .unwrap_or_default();
    // Translators: {owner} is the name of the playlist's owner.
    gettext(app.locale, "Playlist • {owner}")
        .replace("{owner}", &owner)
        .trim_end_matches([' ', '•'])
        .to_string()
}

fn liked_songs_card(app: &mut App, ui: &mut egui::Ui) {
    let playing_here = app.believed_playing()
        && app
            .playing_context_uri()
            .is_some_and(|context| context.ends_with(":collection"));
    let subtitle = liked_songs_subtitle(app);
    let card = widgets::card(
        ui,
        app,
        Some(LIKED_SONGS_KEY),
        &gettext(app.locale, "Liked Songs"),
        &subtitle,
        widgets::CardCover::square(playing_here),
    );
    if card.play {
        if playing_here {
            app.actions.push(Action::TogglePlay);
        } else if let Some(uri) = liked_songs_uri(app) {
            app.actions.push(Action::PlayContext {
                uri,
                offset_uri: None,
                offset_index: None,
            });
        }
    }
    if card.clicked {
        app.actions.push(Action::Open(Page::LikedSongs));
    }
}

/// The playlists Search shows. The Web API search never returns a private
/// playlist or Liked Songs, so the listener's own whose names hold the
/// search come first, then Spotify's results, each once. The place returned
/// is where Liked Songs goes among them, when its name starts with the
/// search: first from three letters on, as in Spotify, and after the
/// listener's own playlists for two.
fn playlists_shown(
    app: &App,
    results: &SearchResults,
) -> (Option<usize>, Vec<crate::api::models::Playlist>) {
    let key = search_key(&app.search.committed);
    let letters = key.chars().count();
    let mut shown = Vec::new();
    let mut liked = None;
    if letters >= 2 {
        if let Some(playlists) = app.library.playlists.get() {
            shown.extend(
                playlists
                    .iter()
                    .filter(|playlist| search_key(&playlist.name).contains(&key))
                    .cloned(),
            );
        }
        let named = [gettext(app.locale, "Liked Songs").as_ref(), "Liked Songs"]
            .iter()
            .any(|name| search_key(name).starts_with(&key));
        if named {
            liked = Some(if letters >= 3 { 0 } else { shown.len() });
        }
    }
    for playlist in results.playlists.iter().flat_map(|page| &page.items) {
        if !shown.iter().any(|held| held.uri == playlist.uri) {
            shown.push(playlist.clone());
        }
    }
    (liked, shown)
}

/// Draws the playlist cards with Liked Songs at its place among them.
fn playlist_cards(
    app: &mut App,
    ui: &mut egui::Ui,
    liked: Option<usize>,
    playlists: &[crate::api::models::Playlist],
) {
    for index in 0..=playlists.len() {
        if liked == Some(index) {
            liked_songs_card(app, ui);
        }
        if let Some(playlist) = playlists.get(index) {
            playlist_card(app, ui, playlist);
        }
    }
}

fn shelf_playlists(app: &mut App, ui: &mut egui::Ui, results: &SearchResults) {
    let palette = app.palette;
    let (liked, playlists) = playlists_shown(app, results);
    if liked.is_none() && playlists.is_empty() {
        return;
    }
    widgets::shelf(
        ui,
        &palette,
        "search-playlists",
        &gettext(app.locale, "Playlists"),
        |ui| playlist_cards(app, ui, liked, &playlists),
    );
}

fn playlists_grid(app: &mut App, ui: &mut egui::Ui, results: &SearchResults) {
    let (liked, playlists) = playlists_shown(app, results);
    widgets::grid(ui, |ui| playlist_cards(app, ui, liked, &playlists));
}

fn show_card(app: &mut App, ui: &mut egui::Ui, show: &crate::api::models::Show) {
    let card = widgets::card(
        ui,
        app,
        pick_image(&show.images, 640),
        &show.name,
        &show.publisher,
        widgets::CardCover::default(),
    );
    if card.clicked {
        app.actions.push(Action::Open(Page::Show(show.id.clone())));
    }
    egui::Popup::context_menu(&card.response)
        .id(ui.make_persistent_id(("search-show-menu", &show.uri)))
        .frame(widgets::menu_frame(&app.palette))
        .show(|ui| {
            widgets::context_menu_items(ui, app, &show.uri, &show.name, None);
        });
}

fn shelf_shows(app: &mut App, ui: &mut egui::Ui, results: &SearchResults) {
    let palette = app.palette;
    let Some(page) = &results.shows else { return };
    if page.items.is_empty() {
        return;
    }
    widgets::shelf(
        ui,
        &palette,
        "search-shows",
        &gettext(app.locale, "Podcasts"),
        |ui| {
            for show in &page.items {
                show_card(app, ui, show);
            }
        },
    );
}

fn shows_grid(app: &mut App, ui: &mut egui::Ui, results: &SearchResults) {
    let Some(page) = &results.shows else { return };
    widgets::grid(ui, |ui| {
        for show in &page.items {
            show_card(app, ui, show);
        }
    });
}

fn episodes(app: &mut App, ui: &mut egui::Ui, results: &SearchResults, limit: usize) {
    let Some(page) = &results.episodes else {
        return;
    };
    for episode in page.items.iter().take(limit) {
        super::show::episode_row(app, ui, episode, None);
    }
}

#[allow(dead_code)]
fn align_right(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    ui.with_layout(Layout::right_to_left(Align::Center), add);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::models::{Album, Page as Results, Track};

    fn results() -> SearchResults {
        let track = |name: &str| Track {
            name: name.into(),
            ..Track::default()
        };
        let artist = |name: &str| Artist {
            name: name.into(),
            ..Artist::default()
        };
        SearchResults {
            tracks: Some(Results {
                items: vec![track("Broken Chairs"), track("Carry the Zero")],
                ..Default::default()
            }),
            artists: Some(Results {
                items: vec![artist("keepsecrets"), artist("Built to Spill")],
                ..Default::default()
            }),
            albums: Some(Results {
                items: vec![
                    Album {
                        name: "There's Nothing Wrong with Love".into(),
                        ..Album::default()
                    },
                    Album {
                        name: "Keep It Like a Secret".into(),
                        uri: "spotify:album:secret".into(),
                        ..Album::default()
                    },
                ],
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    fn name(top: Option<Top<'_>>) -> &str {
        match top.expect("a top result") {
            Top::Artist(artist) => &artist.name,
            Top::Album(album) => &album.name,
            Top::Song(track) => &track.name,
            Top::Playlist(playlist) => &playlist.name,
            Top::Podcast(show) => &show.name,
            Top::LikedSongs => "Liked Songs",
        }
    }

    #[test]
    fn a_result_named_as_searched_is_the_top_result_whatever_its_kind() {
        let results = results();
        assert_eq!(
            name(top_pick(&results, "keep it like a secret", |_| false)),
            "Keep It Like a Secret"
        );
        assert_eq!(
            name(top_pick(&results, "Built To Spill!", |_| false)),
            "Built to Spill"
        );
        assert_eq!(
            name(top_pick(&results, "carry the zero", |_| false)),
            "Carry the Zero"
        );
    }

    /// Spotify's own app puts what the listener saved first: "fire" finds
    /// a saved album called Firewater before an artist called Fire.
    #[test]
    fn a_saved_result_near_the_top_of_its_list_wins_over_a_matching_name() {
        let results = results();
        let saved = |uri: &str| uri == "spotify:album:secret";
        assert_eq!(
            name(top_pick(&results, "keepsecrets", saved)),
            "Keep It Like a Secret"
        );
        assert_eq!(
            name(top_pick(&results, "keepsecrets", |_| false)),
            "keepsecrets"
        );
    }

    /// As in Spotify, Liked Songs is the Top result from three letters on.
    #[test]
    fn liked_songs_is_the_top_result_from_three_letters() {
        let library = crate::model::Library::default();
        let top = |query: &str| {
            library_pick(&library, crate::i18n::Locale::English, query)
                .is_some_and(|owned| matches!(owned, Owned::LikedSongs))
        };
        assert!(!top("li"));
        assert!(top("lik"));
        assert!(top("Liked songs"));
        assert!(!top("songs"), "only the start of its name counts");
    }

    #[test]
    fn the_library_is_searched_by_the_start_of_a_name() {
        use crate::api::models::SavedAlbum;
        let mut library = crate::model::Library::default();
        for name in ["Firewater", "Diary (Remastered and Expanded)", "Dia"] {
            library.albums.items.push(SavedAlbum {
                added_at: None,
                album: Album {
                    name: name.into(),
                    ..Album::default()
                },
            });
        }
        let found = |query: &str| {
            library_pick(&library, crate::i18n::Locale::English, query)
                .map(|owned| name(Some(owned.top())).to_string())
        };
        assert_eq!(found("fire").as_deref(), Some("Firewater"));
        assert_eq!(
            found("diar").as_deref(),
            Some("Diary (Remastered and Expanded)")
        );
        // A name that is exactly the search beats one that only starts with it.
        assert_eq!(found("dia").as_deref(), Some("Dia"));
        assert_eq!(found("f"), None, "one letter is not enough");
        assert_eq!(found("water"), None, "only the start of a name counts");
    }

    #[test]
    fn without_a_matching_name_the_first_song_is_the_top_result() {
        let mut results = results();
        assert_eq!(name(top_pick(&results, "keep", |_| false)), "Broken Chairs");
        results.tracks = None;
        assert_eq!(name(top_pick(&results, "keep", |_| false)), "keepsecrets");
    }
}
