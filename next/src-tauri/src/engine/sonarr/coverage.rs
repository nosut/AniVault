//! Which Watching/Planning entries a Sonarr series covers. Pure: callers pass
//! in the list entries, the live Sonarr series, the existing mappings, and the
//! user's coverage links.

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use regex::Regex;

use crate::engine::matcher::normalize_title;
use crate::engine::sonarr::client::{SonarrImportList, SonarrSeriesRaw};
use crate::engine::storage::{CoverageCandidateRow, CoverageLinkDb};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverageVia {
    Mapping,
    Link,
    Title,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum CoverageState {
    Covered { sonarr_id: i64, via: CoverageVia },
    Missing,
    Ignored,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CoverageRow {
    pub anime_id: i64,
    pub title: String,
    pub image_url: Option<String>,
    pub list_status: String,
    #[serde(flatten)]
    pub state: CoverageState,
}

/// Trailing season markers on an already-normalized title ("season 3",
/// "2nd season", "part 2", "第2期", "第3クール", "iv", ...). Repeated, so
/// "season 3 part 2" and "第4期 第3クール" lose both markers.
pub fn strip_season_markers(normalized: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(
            r"(?x)\s*(?:
                (?:season|part|cour)\s+\d+
              | \d+(?:st|nd|rd|th)\s+(?:season|part|cour)
              | 第[0-9一二三四五六七八九十]+(?:期|幕|クール|部)
              | [0-9]+期
              | \b(?:ii|iii|iv|v)
            )\s*$",
        )
        .expect("season marker regex")
    });
    let mut s = normalized.trim().to_string();
    loop {
        let next = re.replace(&s, "").trim().to_string();
        if next == s || next.is_empty() {
            return s;
        }
        s = next;
    }
}

fn title_strings(titles_json: &str) -> Vec<String> {
    let v: serde_json::Value = serde_json::from_str(titles_json).unwrap_or_default();
    let mut out = Vec::new();
    for key in ["english", "english_derived", "romaji", "japanese"] {
        if let Some(t) = v[key].as_str() {
            out.push(t.to_string());
        }
    }
    if let Some(arr) = v["synonyms"].as_array() {
        out.extend(arr.iter().filter_map(|s| s.as_str().map(str::to_string)));
    }
    out
}

/// Normalized match keys for a title: the title itself and its season-stripped form.
fn keys_for(title: &str, into: &mut HashSet<String>) {
    let n = normalize_title(title);
    if n.is_empty() {
        return;
    }
    into.insert(strip_season_markers(&n));
    into.insert(n);
}

/// The title shown for an entry: English, then derived English, then romaji.
pub fn display_title(titles_json: &str) -> String {
    let v: serde_json::Value = serde_json::from_str(titles_json).unwrap_or_default();
    ["english", "english_derived", "romaji"]
        .iter()
        .filter_map(|k| v[*k].as_str())
        .find(|t| !t.trim().is_empty())
        .unwrap_or("")
        .to_string()
}

/// Most search terms tried per show. Each one is a Sonarr round trip.
pub const MAX_LOOKUP_TERMS: usize = 6;

/// Search terms for Sonarr's lookup, best first. Sonarr searches TVDB's
/// English titles and misses long subtitled ones, so English titles (official,
/// then derived) come first, then synonyms, then romaji. Each title is followed
/// by its part before a colon or dash ("Magical Buffs"), which Sonarr often
/// finds when the full title fails. No blanks or duplicates; at most
/// `MAX_LOOKUP_TERMS`.
pub fn lookup_terms(titles_json: &str) -> Vec<String> {
    let v: serde_json::Value = serde_json::from_str(titles_json).unwrap_or_default();
    let mut sources: Vec<&str> = Vec::new();
    for key in ["english", "english_derived"] {
        if let Some(t) = v[key].as_str() {
            sources.push(t);
        }
    }
    if let Some(arr) = v["synonyms"].as_array() {
        sources.extend(arr.iter().filter_map(|s| s.as_str()));
    }
    if let Some(t) = v["romaji"].as_str() {
        sources.push(t);
    }

    let mut terms: Vec<String> = Vec::new();
    let mut push = |t: &str| {
        let t = t.trim();
        if !t.is_empty() && !terms.iter().any(|x| x.eq_ignore_ascii_case(t)) {
            terms.push(t.to_string());
        }
    };
    for t in sources {
        push(t);
        if let Some(head) = title_head(t) {
            push(head);
        }
    }
    terms.truncate(MAX_LOOKUP_TERMS);
    terms
}

/// The part of a title before its subtitle (":", " - ", "～"), when that part
/// is at least two words.
fn title_head(title: &str) -> Option<&str> {
    let cut = [":", " - ", "～", "~"]
        .iter()
        .filter_map(|sep| title.find(sep))
        .min()?;
    let head = title[..cut].trim();
    (head.split_whitespace().count() >= 2).then_some(head)
}

/// Candidates ordered by how closely their title matches the show's titles
/// (Sonarr's own order breaks ties), capped at `limit`.
pub fn rank_candidates(
    titles_json: &str,
    mut candidates: Vec<crate::engine::sonarr::client::SonarrCandidate>,
    limit: usize,
) -> Vec<crate::engine::sonarr::client::SonarrCandidate> {
    // sort_by_key is stable, so equal scores keep Sonarr's order.
    candidates.sort_by_key(|c| std::cmp::Reverse(crate::engine::matcher::score_titles_json(&c.title, titles_json)));
    candidates.truncate(limit);
    candidates
}

/// Coverage for every non-movie candidate. Rule order: ignored, existing
/// mapping, confirmed link, exact normalized title (season markers stripped).
/// Mappings and links pointing at series no longer in `series` do not count.
pub fn classify(
    candidates: &[CoverageCandidateRow],
    series: &[SonarrSeriesRaw],
    mapped: &[(i64, i64)],
    links: &[CoverageLinkDb],
) -> Vec<CoverageRow> {
    let live: HashSet<i64> = series.iter().map(|s| s.id).collect();
    let mapped: HashMap<i64, i64> = mapped
        .iter()
        .filter(|(_, sid)| live.contains(sid))
        .copied()
        .collect();
    let links: HashMap<i64, &CoverageLinkDb> = links.iter().map(|l| (l.anime_id, l)).collect();

    let mut by_key: HashMap<String, i64> = HashMap::new();
    for s in series {
        let mut keys = HashSet::new();
        keys_for(&s.title, &mut keys);
        for alt in &s.alternate_titles {
            keys_for(&alt.title, &mut keys);
        }
        for k in keys {
            by_key.entry(k).or_insert(s.id);
        }
    }

    candidates
        .iter()
        .filter(|c| c.format.as_deref() != Some("MOVIE"))
        .map(|c| {
            let link = links.get(&c.anime_id);
            let state = if link.is_some_and(|l| l.ignored) {
                CoverageState::Ignored
            } else if let Some(&sid) = mapped.get(&c.anime_id) {
                CoverageState::Covered { sonarr_id: sid, via: CoverageVia::Mapping }
            } else if let Some(sid) = link.and_then(|l| l.sonarr_id).filter(|s| live.contains(s)) {
                CoverageState::Covered { sonarr_id: sid, via: CoverageVia::Link }
            } else {
                let mut keys = HashSet::new();
                for t in title_strings(&c.titles_json) {
                    keys_for(&t, &mut keys);
                }
                match keys.iter().find_map(|k| by_key.get(k)) {
                    Some(&sid) => CoverageState::Covered { sonarr_id: sid, via: CoverageVia::Title },
                    None => CoverageState::Missing,
                }
            };
            CoverageRow {
                anime_id: c.anime_id,
                title: display_title(&c.titles_json),
                image_url: c.image_url.clone(),
                list_status: c.list_status.clone(),
                state,
            }
        })
        .collect()
}

/// `POST /api/v3/series` body for a new series, copying the AniList import
/// list's settings. Errors when the list lacks a root folder or quality profile.
pub fn add_series_body(tvdb_id: i64, title: &str, list: &SonarrImportList) -> anyhow::Result<serde_json::Value> {
    let root = list
        .root_folder_path
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("The AniList import list in Sonarr has no root folder"))?;
    let profile = list
        .quality_profile_id
        .ok_or_else(|| anyhow::anyhow!("The AniList import list in Sonarr has no quality profile"))?;
    Ok(serde_json::json!({
        "tvdbId": tvdb_id,
        "title": title,
        "qualityProfileId": profile,
        "rootFolderPath": root,
        "seriesType": list.series_type.as_deref().unwrap_or("anime"),
        "seasonFolder": list.season_folder.unwrap_or(true),
        "monitored": true,
        "monitorNewItems": list.monitor_new_items.as_deref().unwrap_or("all"),
        "tags": list.tags,
        "addOptions": {
            "monitor": list.should_monitor.as_deref().unwrap_or("all"),
            "searchForMissingEpisodes": true,
            "searchForCutoffUnmetEpisodes": false,
        },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::sonarr::client::{SonarrAlternateTitle, SonarrImportList, SonarrSeriesRaw};
    use crate::engine::storage::{CoverageCandidateRow, CoverageLinkDb};

    fn norm(s: &str) -> String {
        crate::engine::matcher::normalize_title(s)
    }

    #[test]
    fn strips_season_markers() {
        let cases = [
            ("Shangri-La Frontier Season 3", "shangrila frontier"),
            ("Re:Monster 2nd Season", "remonster"),
            ("One Punch Man Season 3 Part 2", "one punch man"),
            ("望まぬ不死の冒険者 第2期", "望まぬ不死の冒険者"),
            ("転生したらスライムだった件 第4期 第3クール", "転生したらスライムだった件"),
            ("ふつつかな悪女ではございますが ～雛宮蝶鼠とりかえ伝～ 第二幕", "ふつつかな悪女ではございますが 雛宮蝶鼠とりかえ伝"),
            ("Overlord IV", "overlord"),
            ("Overgeared", "overgeared"),
        ];
        for (input, want) in cases {
            assert_eq!(strip_season_markers(&norm(input)), want, "{input}");
        }
    }

    fn cand(id: i64, titles: &str) -> CoverageCandidateRow {
        CoverageCandidateRow {
            anime_id: id,
            titles_json: titles.to_string(),
            format: None,
            image_url: None,
            list_status: "watching".into(),
        }
    }

    fn series(id: i64, title: &str, alts: &[&str]) -> SonarrSeriesRaw {
        let mut s: SonarrSeriesRaw = serde_json::from_value(serde_json::json!({
            "id": id, "title": title, "monitored": true, "tags": []
        }))
        .unwrap();
        s.alternate_titles = alts
            .iter()
            .map(|t| SonarrAlternateTitle { title: t.to_string() })
            .collect();
        s
    }

    fn states(rows: &[CoverageRow]) -> Vec<(i64, CoverageState)> {
        rows.iter().map(|r| (r.anime_id, r.state.clone())).collect()
    }

    #[test]
    fn classify_applies_rules_in_order() {
        let cands = vec![
            cand(1, r#"{"romaji":"Anything","english":"Mapped Show"}"#),
            cand(2, r#"{"romaji":"Linked"}"#),
            cand(3, r#"{"romaji":"Shangri-La Frontier","english":"Shangri-La Frontier Season 3"}"#),
            cand(4, r#"{"romaji":"Temppal","english":"Overgeared"}"#),
            cand(5, r#"{"romaji":"Nope"}"#),
        ];
        let live = vec![series(10, "Mapped", &[]), series(20, "Other", &[]), series(30, "Shangri-La Frontier", &[])];
        let mapped = vec![(1, 10)];
        let links = vec![
            CoverageLinkDb { anime_id: 2, sonarr_id: Some(20), ignored: false },
            CoverageLinkDb { anime_id: 5, sonarr_id: None, ignored: true },
        ];
        assert_eq!(
            states(&classify(&cands, &live, &mapped, &links)),
            vec![
                (1, CoverageState::Covered { sonarr_id: 10, via: CoverageVia::Mapping }),
                (2, CoverageState::Covered { sonarr_id: 20, via: CoverageVia::Link }),
                (3, CoverageState::Covered { sonarr_id: 30, via: CoverageVia::Title }),
                (4, CoverageState::Missing),
                (5, CoverageState::Ignored),
            ]
        );
    }

    #[test]
    fn classify_ignores_stale_mapping_and_link() {
        let cands = vec![cand(1, r#"{"romaji":"A"}"#), cand(2, r#"{"romaji":"B"}"#)];
        let links = vec![CoverageLinkDb { anime_id: 2, sonarr_id: Some(99), ignored: false }];
        let rows = classify(&cands, &[series(5, "Unrelated", &[])], &[(1, 98)], &links);
        assert_eq!(states(&rows), vec![(1, CoverageState::Missing), (2, CoverageState::Missing)]);
    }

    #[test]
    fn classify_does_not_match_by_containment() {
        let cands = vec![cand(1, r#"{"romaji":"Re:Monster 2nd Season"}"#)];
        let rows = classify(&cands, &[series(5, "Monster", &[])], &[], &[]);
        assert_eq!(states(&rows), vec![(1, CoverageState::Missing)]);
    }

    #[test]
    fn classify_matches_japanese_alternate_title() {
        let cands = vec![cand(
            1,
            r#"{"romaji":"Nozomanu Fushi no Boukensha 2","english":null,"japanese":"望まぬ不死の冒険者 第2期"}"#,
        )];
        let rows = classify(&cands, &[series(7, "The Unwanted Undead Adventurer", &["望まぬ不死の冒険者"])], &[], &[]);
        assert_eq!(
            states(&rows),
            vec![(1, CoverageState::Covered { sonarr_id: 7, via: CoverageVia::Title })]
        );
    }

    #[test]
    fn classify_skips_movies() {
        let mut movie = cand(1, r#"{"romaji":"Kage no Jitsuryokusha Movie"}"#);
        movie.format = Some("MOVIE".into());
        assert!(classify(&[movie], &[], &[], &[]).is_empty());
    }

    #[test]
    fn display_title_prefers_english_then_derived_then_romaji() {
        assert_eq!(display_title(r#"{"romaji":"R","english":"E"}"#), "E");
        assert_eq!(display_title(r#"{"romaji":"R","english":null,"english_derived":"D"}"#), "D");
        assert_eq!(display_title(r#"{"romaji":"R","english":""}"#), "R");
    }

    #[test]
    fn lookup_terms_put_english_first_and_romaji_last_deduped() {
        let t = lookup_terms(r#"{"romaji":"Temppal","english":"Overgeared","synonyms":["템빨","Overgeared"," "]}"#);
        assert_eq!(t, vec!["Overgeared", "템빨", "Temppal"]);
    }

    // Sonarr's search misses long subtitled titles but finds the part before
    // the colon ("Magical Buffs" -> TVDB 475721, checked against Skyhook).
    #[test]
    fn lookup_terms_add_the_title_before_a_colon() {
        let t = lookup_terms(
            r#"{"romaji":"Zatsuyou Fuyojutsu-shi ga Jibun no Saikyou ni Kizuku Made",
                "english":"Magical Buffs: The Support Caster is Stronger Than He Realized!",
                "synonyms":["Magical Buffs: The Support Caster is Stronger Than He Realized!","ZatsuyoFuyo"]}"#,
        );
        assert_eq!(
            t,
            vec![
                "Magical Buffs: The Support Caster is Stronger Than He Realized!",
                "Magical Buffs",
                "ZatsuyoFuyo",
                "Zatsuyou Fuyojutsu-shi ga Jibun no Saikyou ni Kizuku Made",
            ]
        );
    }

    // With no official English title, the derived one (Sonarr finds the elf
    // show by it) must be searched before the romaji, which finds nothing.
    #[test]
    fn lookup_terms_use_the_derived_english_title_before_romaji() {
        let t = lookup_terms(
            r#"{"romaji":"Game Sekai Tensei <Dankatsu>: Gamer wa [Dungeon Shuukatsu no Susume] wo <Hajime kara> Play Suru",
                "english":null,
                "english_derived":"Reincarnation in the Game World Dan-Katsu: Game Addict Plays \"Encouragement for Job Hunting in Dungeons\" From a \"New Game\"",
                "synonyms":[]}"#,
        );
        assert_eq!(t[1], "Reincarnation in the Game World Dan-Katsu");
        assert!(t[0].starts_with("Reincarnation in the Game World Dan-Katsu:"));
        assert!(t[2].starts_with("Game Sekai Tensei"));
    }

    #[test]
    fn lookup_terms_are_capped() {
        let t = lookup_terms(r#"{"romaji":"R","english":"E","synonyms":["a","b","c","d","e","f","g"]}"#);
        assert_eq!(t.len(), MAX_LOOKUP_TERMS);
    }

    fn candidate(title: &str, tvdb_id: i64) -> crate::engine::sonarr::client::SonarrCandidate {
        crate::engine::sonarr::client::SonarrCandidate {
            tvdb_id,
            title: title.into(),
            year: None,
            season_count: 1,
            poster_url: None,
            overview: None,
            in_sonarr: false,
            sonarr_id: None,
        }
    }

    #[test]
    fn rank_candidates_puts_the_closest_title_first() {
        let titles = r#"{"romaji":"Game Sekai Tensei","english":null,
            "english_derived":"Reincarnation in the Game World Dan-Katsu: Game Addict Plays","synonyms":[]}"#;
        let ranked = rank_candidates(
            titles,
            vec![candidate("Game Shakers", 1), candidate("Shin Megami Tensei: Devil Children", 2), candidate("Reincarnation in the Game World Dan-Katsu", 3)],
            2,
        );
        assert_eq!(ranked.iter().map(|c| c.tvdb_id).collect::<Vec<_>>()[0], 3);
        assert_eq!(ranked.len(), 2);
    }

    #[test]
    fn add_series_body_copies_import_list_settings() {
        let list = SonarrImportList {
            id: 1,
            name: "AniList".into(),
            implementation: "AniListImport".into(),
            root_folder_path: Some("/data/anime".into()),
            quality_profile_id: Some(6),
            series_type: Some("anime".into()),
            season_folder: Some(true),
            should_monitor: Some("all".into()),
            monitor_new_items: Some("all".into()),
            tags: vec![2, 3],
        };
        let body = add_series_body(448176, "Overgeared", &list).unwrap();
        assert_eq!(
            body,
            serde_json::json!({
                "tvdbId": 448176, "title": "Overgeared", "qualityProfileId": 6,
                "rootFolderPath": "/data/anime", "seriesType": "anime", "seasonFolder": true,
                "monitored": true, "monitorNewItems": "all", "tags": [2, 3],
                "addOptions": { "monitor": "all", "searchForMissingEpisodes": true,
                                "searchForCutoffUnmetEpisodes": false }
            })
        );
    }

    #[test]
    fn add_series_body_requires_root_folder_and_quality_profile() {
        let mut list = SonarrImportList {
            id: 1,
            name: "AniList".into(),
            implementation: "AniListImport".into(),
            root_folder_path: None,
            quality_profile_id: Some(6),
            series_type: None,
            season_folder: None,
            should_monitor: None,
            monitor_new_items: None,
            tags: vec![],
        };
        assert!(add_series_body(1, "X", &list).is_err());
        list.root_folder_path = Some("/a".into());
        let body = add_series_body(1, "X", &list).unwrap();
        assert_eq!(body["seriesType"], "anime");
        assert_eq!(body["addOptions"]["monitor"], "all");
    }
}
