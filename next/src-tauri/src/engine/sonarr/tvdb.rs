//! A TVDB series typed in by hand: an ID, `tvdb:<id>`, or a thetvdb.com link.
//! Sonarr's name search misses some series (new ones especially), but finds
//! any series by `tvdb:<id>`.

use std::sync::LazyLock;

use regex::Regex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TvdbRef {
    Id(i64),
    /// The name part of a `thetvdb.com/series/<slug>` link. Only TVDB's page
    /// knows its ID.
    Slug(String),
}

static QUERY_ID: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[?&]id=(\d+)").unwrap());
static SERIES_PATH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)/series/([A-Za-z0-9_-]+)").unwrap());

/// Read what the user pasted. `None` when it is neither an ID nor a
/// thetvdb.com series link.
pub fn parse_tvdb_ref(input: &str) -> Option<TvdbRef> {
    let s = input.trim();
    let bare = s.get(..5).filter(|p| p.eq_ignore_ascii_case("tvdb:")).map_or(s, |_| s[5..].trim());
    if let Ok(id) = bare.parse::<i64>() {
        return (id > 0).then_some(TvdbRef::Id(id));
    }
    if !s.to_ascii_lowercase().contains("thetvdb.com") {
        return None;
    }
    if let Some(c) = QUERY_ID.captures(s) {
        return c[1].parse().ok().map(TvdbRef::Id);
    }
    let part = &SERIES_PATH.captures(s)?[1];
    match part.parse::<i64>() {
        Ok(id) => Some(TvdbRef::Id(id)),
        Err(_) => Some(TvdbRef::Slug(part.to_string())),
    }
}

static INFO_ID: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"TheTVDB\.com Series ID\s*</strong>\s*<span>\s*(\d+)").unwrap());
static PERMISSION_ID: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"data-permission="series-(\d+)-"#).unwrap());

/// The series ID on a thetvdb.com series page: the "TheTVDB.com Series ID"
/// row, or the ID in its edit-permission markers if that row moves.
pub fn series_id_from_page(html: &str) -> Option<i64> {
    INFO_ID
        .captures(html)
        .or_else(|| PERMISSION_ID.captures(html))
        .and_then(|c| c[1].parse().ok())
}

/// Fetch `thetvdb.com/series/<slug>` and read the series ID off it.
pub async fn resolve_slug(slug: &str) -> anyhow::Result<i64> {
    let client = reqwest::Client::builder()
        .user_agent(concat!("AniVault/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(15))
        .build()?;
    let resp = client.get(format!("https://thetvdb.com/series/{slug}")).send().await?;
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        anyhow::bail!("TheTVDB has no series at that link ({slug})");
    }
    let html = resp.error_for_status()?.text().await?;
    series_id_from_page(&html)
        .ok_or_else(|| anyhow::anyhow!("TheTVDB page for {slug} didn't show a series ID"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_ids_in_every_form() {
        for input in [
            "480598",
            " tvdb:480598 ",
            "TVDB: 480598",
            "https://thetvdb.com/series/480598",
            "https://thetvdb.com/dereferrer/series/480598",
            "https://thetvdb.com/?tab=series&id=480598",
            "thetvdb.com/index.php?tab=series&id=480598&lid=7",
        ] {
            assert_eq!(parse_tvdb_ref(input), Some(TvdbRef::Id(480598)), "{input}");
        }
    }

    #[test]
    fn reads_slug_links() {
        for input in [
            "https://thetvdb.com/series/shinnoseijo-anime",
            "https://www.thetvdb.com/series/shinnoseijo-anime/seasons/official/1",
            "thetvdb.com/series/shinnoseijo-anime#seasons",
        ] {
            assert_eq!(parse_tvdb_ref(input), Some(TvdbRef::Slug("shinnoseijo-anime".into())), "{input}");
        }
    }

    #[test]
    fn rejects_anything_else() {
        for input in ["", "  ", "0", "-5", "tvdb:", "Frieren", "https://anilist.co/anime/154587", "https://thetvdb.com/"] {
            assert_eq!(parse_tvdb_ref(input), None, "{input}");
        }
    }

    #[test]
    fn finds_the_id_on_a_series_page() {
        let page = r#"<li class="list-group-item clearfix">
                            <strong>TheTVDB.com Series ID</strong>
                            <span>480598</span>
                        </li>"#;
        assert_eq!(series_id_from_page(page), Some(480598));
        let moved = r#"<li><a href="/series/480598/edit" data-permission="series-480598-edit">Edit Series</a></li>"#;
        assert_eq!(series_id_from_page(moved), Some(480598));
        assert_eq!(series_id_from_page("<html>Not found</html>"), None);
    }
}
