use regex::Regex;
use std::sync::LazyLock;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ParsedFilename {
    pub cleaned_title: String,
    pub episode_number: i32,
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

static EPISODE_WORD_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\bepisode\s+(\d{1,4})\b").unwrap()
});

static DASH_MULTI_NUM_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\s-\s+(\d{1,4})(?:v\d)?(?:\s|$|\.|\[|\(|_|-|\[\s)").unwrap()
});

static YEAR_PAREN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\((?:19|20)\d{2}\)").unwrap()
});

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

    // Try S01E01 style first (ignore season, extract episode)
    let mut episode: Option<i32> = None;
    if let Some(caps) = S01E01_RE.captures(&cleaned) {
        if let Ok(n) = caps[2].parse::<i32>() {
            if n > 0 && n <= 2000 {
                episode = Some(n);
                cleaned = S01E01_RE.replace(&cleaned, "").to_string();
            }
        }
    }

    // Try "Episode 01" spelled out
    if episode.is_none() {
        if let Some(caps) = EPISODE_WORD_RE.captures(&cleaned) {
            if let Ok(n) = caps[1].parse::<i32>() {
                if n > 0 && n <= 2000 {
                    episode = Some(n);
                    cleaned = EPISODE_WORD_RE.replace(&cleaned, "").to_string();
                }
            }
        }
    }

    // Try " - 01" dash-number pattern
    if episode.is_none() {
        if let Some(caps) = DASH_MULTI_NUM_RE.captures(&cleaned) {
            if let Ok(n) = caps[1].parse::<i32>() {
                if n > 0 && n <= 2000 {
                    episode = Some(n);
                    cleaned = DASH_MULTI_NUM_RE.replace(&cleaned, "").to_string();
                }
            }
        }
    }

    // Try bare EP01 / E01 patterns
    if episode.is_none() {
        if let Some(caps) = EPISODE_RE.captures(&cleaned) {
            if let Ok(n) = caps[1].parse::<i32>() {
                if n > 0 && n <= 2000 {
                    episode = Some(n);
                    cleaned = EPISODE_RE.replace(&cleaned, "").to_string();
                }
            }
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

    // Collapse double-dash artifacts from " - S01E01 - EpisodeTitle" pattern removal
    // e.g., "2.5 Dimensional Seduction - - Title" -> "2.5 Dimensional Seduction"
    if let Some(pos) = cleaned.find(" - - ") {
        cleaned = cleaned[..pos].to_string();
    }

    if cleaned.is_empty() {
        return None;
    }

    Some(ParsedFilename {
        cleaned_title: cleaned,
        episode_number,
        release_group,
        quality,
        raw: input.to_string(),
    })
}
