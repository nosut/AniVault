use crate::engine::models::ParseResult;

pub fn parse_filename(path: &str) -> ParseResult {
    let stem = file_stem(path);
    let without_tags = strip_square_tags(stem);
    let without_metadata_parens = strip_metadata_parentheses(&without_tags);
    let normalized = normalize_separators(&without_metadata_parens);
    let (mut title, season, episode) = extract_fields(&normalized);
    title = cleanup_title(&title);

    ParseResult {
        title,
        season,
        episode,
        confidence: if episode.is_some() { 0.95 } else { 0.5 },
    }
}

fn file_stem(path: &str) -> &str {
    let name = path.rsplit(['\\', '/']).next().unwrap_or(path);
    name.rsplit_once('.').map_or(name, |(stem, _)| stem)
}

fn strip_square_tags(input: &str) -> String {
    let re = regex_lite::Regex::new(r"\[[^\]]*\]").expect("valid square tag regex");
    re.replace_all(input, " ").to_string()
}

fn strip_metadata_parentheses(input: &str) -> String {
    let re = regex_lite::Regex::new(r"\(([^)]*)\)").expect("valid paren regex");
    re.replace_all(input, |caps: &regex_lite::Captures<'_>| {
        let inner = caps.get(1).map(|m| m.as_str().trim()).unwrap_or_default();
        if inner.chars().all(|c| c.is_ascii_digit()) {
            caps.get(0).map(|m| m.as_str()).unwrap_or_default().to_string()
        } else {
            " ".to_string()
        }
    })
    .to_string()
}

fn normalize_separators(input: &str) -> String {
    let mut value = input.replace('_', " ").replace('.', " ");
    value = value.replace('–', "-").replace('—', "-");
    collapse_spaces(&value)
}

fn extract_fields(input: &str) -> (String, Option<i32>, Option<i32>) {
    let mut text = input.to_string();
    let mut season = None;
    let mut episode = None;

    let sxe = regex_lite::Regex::new(r"(?i)\bS(\d{1,2})E(\d{1,4})\b").expect("valid sxe regex");
    text = sxe
        .replace(&text, |caps: &regex_lite::Captures<'_>| {
            season = caps.get(1).and_then(|m| m.as_str().parse().ok());
            episode = caps.get(2).and_then(|m| m.as_str().parse().ok());
            " "
        })
        .to_string();

    let season_re = regex_lite::Regex::new(r"(?i)\b(?:Season|S)\s*(\d{1,2})\b").expect("valid season regex");
    text = season_re
        .replace(&text, |caps: &regex_lite::Captures<'_>| {
            if season.is_none() {
                season = caps.get(1).and_then(|m| m.as_str().parse().ok());
            }
            " "
        })
        .to_string();

    let ep_prefix = regex_lite::Regex::new(r"(?i)\b(?:Ep|Episode)\s*(\d{1,4})\b").expect("valid ep regex");
    text = ep_prefix
        .replace(&text, |caps: &regex_lite::Captures<'_>| {
            if episode.is_none() {
                episode = caps.get(1).and_then(|m| m.as_str().parse().ok());
            }
            " "
        })
        .to_string();

    let hash_re = regex_lite::Regex::new(r"#\s*(\d{1,4})\b").expect("valid hash regex");
    text = hash_re
        .replace(&text, |caps: &regex_lite::Captures<'_>| {
            if episode.is_none() {
                episode = caps.get(1).and_then(|m| m.as_str().parse().ok());
            }
            " "
        })
        .to_string();

    let paren_ep = regex_lite::Regex::new(r"\(\s*(\d{1,4})\s*\)").expect("valid paren ep regex");
    text = paren_ep
        .replace(&text, |caps: &regex_lite::Captures<'_>| {
            if episode.is_none() {
                episode = caps.get(1).and_then(|m| m.as_str().parse().ok());
            }
            " "
        })
        .to_string();

    let trailing = regex_lite::Regex::new(r"(?:^|\s|-)(\d{1,4})\s*$").expect("valid trailing regex");
    text = trailing
        .replace(&text, |caps: &regex_lite::Captures<'_>| {
            if episode.is_none() {
                let num = caps.get(1).and_then(|m| m.as_str().parse::<i32>().ok());
                if let Some(n) = num {
                    if !(1900..=2099).contains(&n) {
                        episode = Some(n);
                        return " ".to_string();
                    }
                }
            }
            caps.get(0).map(|m| m.as_str()).unwrap_or_default().to_string()
        })
        .to_string();

    (text, season, episode)
}

fn cleanup_title(input: &str) -> String {
    let dimensions = regex_lite::Regex::new(r"(?i)\b(?:480p|720p|1080p|2160p|4k|8k|hevc|x264|x265|av1|aac|flac|web-dl|webrip|bluray|bdrip)\b")
        .expect("valid metadata regex");
    let without_meta = dimensions.replace_all(input, " ");
    let hash = regex_lite::Regex::new(r"\b[A-Fa-f0-9]{8}\b").expect("valid hash regex");
    collapse_spaces(&hash.replace_all(&without_meta, " "))
        .trim_matches(['-', ' '])
        .to_string()
}

fn collapse_spaces(input: &str) -> String {
    let spaces = regex_lite::Regex::new(r"\s+").expect("valid spaces regex");
    spaces.replace_all(input.trim(), " ").to_string()
}
