use regex::Regex;
use std::sync::LazyLock;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ParsedFilename {
    pub cleaned_title: String,
    pub episode_number: i32,
    /// Season carried by an `S02E05` / `2x05` marker, when the name has one.
    /// `None` for the many names that number episodes without a season.
    pub season_number: Option<i32>,
    pub release_group: Option<String>,
    pub quality: Option<String>,
    pub raw: String,
}

static QUALITY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[\s*(?:1080p|720p|480p|2160p|4K|8K|SD|HD)\s*\]|\(\s*(?:1080p|720p|480p|2160p)\s*\)")
        .unwrap()
});

static CODEC_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[\s*(?:x264|x265|HEVC|AVC|H264|H265|AV1|VP9)\s*\]|\(\s*(?:x264|x265|HEVC)\s*\)")
        .unwrap()
});

static AUDIO_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[\s*(?:AAC|FLAC|OPUS|MP3|DTS|AC3|EAC3|TrueHD|Vorbis)(?:\s+?[0-9.]+[kK])?\s*\]|\(\s*(?:AAC|FLAC)\s*\)")
        .unwrap()
});

static RELEASE_GROUP_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\[([^\]]+)\]").unwrap()
});

static EPISODE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:^|\s|_|-)(?:E(?:P)?)(\d{1,4})(?:\s|$|\.|\[|\(|_|-)").unwrap()
});

static S01E01_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)[Ss](\d{1,2})[Ee](\d{1,4})").unwrap()
});

// "1x01" / "12x05" cross format (season x episode). Season capped at 2 digits and
// episode requires 2+ digits, so it won't collide with resolutions (1920x1080),
// aspect ratios (16x9), or titles like "3x3 Eyes".
static SEASON_X_EP_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(\d{1,2})x(\d{2,4})\b").unwrap()
});

// Season declared inside a show *title*: "2nd Season", "Season 2", "Part 2".
static TITLE_SEASON_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:(\d{1,2})(?:st|nd|rd|th)\s+season|seasons?\s+(\d{1,2})|part\s+(\d{1,2}))\b")
        .unwrap()
});

static EPISODE_WORD_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\bepisode\s+(\d{1,4})\b").unwrap()
});

static DASH_MULTI_NUM_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\s-\s+(\d{1,4})(?:v\d)?(?:\s|$|\.|\[|\(|_|-|\[\s)").unwrap()
});

static YEAR_PAREN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\((?:19|20)\d{2}\)").unwrap()
});

// Trailing " - mpv" / " - VLC media player" etc. that players append to the
// window title.
static PLAYER_SUFFIX_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\s*-\s*(?:mpv|vlc(?:\s+media\s+player)?|potplayer|mpc-?(?:hc|be)?|smplayer|kmplayer|gom\s*player|windows media player)\s*$").unwrap()
});

/// Match one episode pattern and split the text at it: the show title is whatever
/// precedes the marker, and everything after it is the episode's own title.
///
/// Both shapes in the wild read that way — "Show - S01E07 - Episode Title.mkv" on
/// disk, and "Show S01E07 Episode Title" from a player reporting the file's
/// embedded title with no separators at all. Removing just the marker used to
/// leave the episode title glued to the show name, which then drags a handful of
/// common words ("will", "you", "death", "again") into the library search.
///
/// `episode_group` is which capture holds the episode number, `season_group` the
/// season when the pattern has one. Returns `None` when the pattern does not
/// match or the number is out of range. When nothing precedes the marker there is
/// no show name to recover, so the remainder is kept rather than yielding an
/// empty title.
fn episode_and_title(
    cleaned: &str,
    re: &Regex,
    episode_group: usize,
    season_group: Option<usize>,
) -> Option<(i32, Option<i32>, String)> {
    let caps = re.captures(cleaned)?;
    let n = caps.get(episode_group)?.as_str().parse::<i32>().ok()?;
    if n <= 0 || n > 2000 {
        return None;
    }
    let season = season_group
        .and_then(|g| caps.get(g))
        .and_then(|m| m.as_str().parse::<i32>().ok())
        .filter(|s| *s > 0 && *s <= 50);

    let marker = caps.get(0)?;
    let before = cleaned[..marker.start()]
        .trim()
        .trim_end_matches(['-', '_', '.', ' '])
        .trim();
    let title = if before.is_empty() {
        cleaned[marker.end()..].trim().to_string()
    } else {
        before.to_string()
    };
    Some((n, season, title))
}

pub fn parse_filename(input: &str, window_title: Option<&str>) -> Option<ParsedFilename> {
    let source_text = window_title.unwrap_or(input);

    if source_text.is_empty() {
        return None;
    }

    let mut cleaned = source_text.to_string();

    // Strip file extensions first (before any pattern matching)
    cleaned = cleaned
        .replace(".mkv", "")
        .replace(".mp4", "")
        .replace(".avi", "")
        .replace(".mov", "")
        .replace(".wmv", "");

    // Strip a trailing player-name suffix that media players add to window titles.
    cleaned = PLAYER_SUFFIX_RE.replace(&cleaned, "").to_string();

    // Extract release group from leading brackets
    let release_group = RELEASE_GROUP_RE
        .captures(&cleaned)
        .map(|c| c[1].to_string());
    cleaned = RELEASE_GROUP_RE.replace(&cleaned, "").to_string();

    // Strip quality tags
    let quality = QUALITY_RE.find(&cleaned).map(|m| m.as_str().to_string());
    cleaned = QUALITY_RE.replace_all(&cleaned, "").to_string();

    // Strip codec tags
    cleaned = CODEC_RE.replace_all(&cleaned, "").to_string();

    // Strip audio tags
    cleaned = AUDIO_RE.replace_all(&cleaned, "").to_string();

    // Try S01E01 style first (season kept: it is what tells a sequel entry apart
    // from the base-season one the show title alone always matches)
    let mut episode: Option<i32> = None;
    let mut season: Option<i32> = None;
    if let Some((n, s, title)) = episode_and_title(&cleaned, &S01E01_RE, 2, Some(1)) {
        episode = Some(n);
        season = s;
        cleaned = title;
    }

    // Try "1x01" cross format (season x episode)
    if episode.is_none() {
        if let Some((n, s, title)) = episode_and_title(&cleaned, &SEASON_X_EP_RE, 2, Some(1)) {
            episode = Some(n);
            season = s;
            cleaned = title;
        }
    }

    // Try "Episode 01" spelled out
    if episode.is_none() {
        if let Some((n, _, title)) = episode_and_title(&cleaned, &EPISODE_WORD_RE, 1, None) {
            episode = Some(n);
            cleaned = title;
        }
    }

    // Try " - 01" dash-number pattern
    if episode.is_none() {
        if let Some((n, _, title)) = episode_and_title(&cleaned, &DASH_MULTI_NUM_RE, 1, None) {
            episode = Some(n);
            cleaned = title;
        }
    }

    // Try bare EP01 / E01 patterns
    if episode.is_none() {
        if let Some((n, _, title)) = episode_and_title(&cleaned, &EPISODE_RE, 1, None) {
            episode = Some(n);
            cleaned = title;
        }
    }

    let episode_number = episode?;

    // Normalize cleaned title: strip brackets, extra whitespace
    cleaned = cleaned.replace('[', " ").replace(']', " ");
    cleaned = YEAR_PAREN_RE.replace_all(&cleaned, "").to_string();
    cleaned = cleaned.replace('(', " ").replace(')', " ");
    cleaned = cleaned.replace('_', " ");
    cleaned = cleaned
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    cleaned = cleaned.trim().to_string();

    if cleaned.is_empty() {
        return None;
    }

    Some(ParsedFilename {
        cleaned_title: cleaned,
        episode_number,
        season_number: season,
        release_group,
        quality,
        raw: input.to_string(),
    })
}

/// The season a *filename* declares through its `S02E05` / `2x05` marker, if any.
/// Used to check a mapped file against the season being played, without paying
/// for a full parse of a name we already know is a real path.
pub fn season_in_filename(name: &str) -> Option<i32> {
    let season = match S01E01_RE.captures(name) {
        Some(caps) => caps.get(1).map(|m| m.as_str().to_string()),
        None => SEASON_X_EP_RE
            .captures(name)
            .and_then(|caps| caps.get(1).map(|m| m.as_str().to_string())),
    }?;
    season.parse::<i32>().ok().filter(|s| *s > 0 && *s <= 50)
}

/// The season a *show title* names — "2nd Season", "Season 2", "Part 2". `None`
/// when the title carries no season at all, which is how AniList writes both a
/// first season and plenty of differently-named sequels ("… R2", "Final Season"),
/// so absence must never be read as "season 1".
pub fn season_in_title(title: &str) -> Option<i32> {
    TITLE_SEASON_RE.captures(title).and_then(|caps| {
        (1..=caps.len() - 1)
            .filter_map(|i| caps.get(i))
            .find_map(|m| m.as_str().parse::<i32>().ok())
            .filter(|s| *s > 0 && *s <= 50)
    })
}
