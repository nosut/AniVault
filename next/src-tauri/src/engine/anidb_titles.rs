//! Looks up official English titles in AniDB's public title dump.
//!
//! Fills the gap franchise inheritance cannot reach: a *first* entry with no
//! English title on AniList has no prequel to borrow a name from, so the only
//! way to give it a readable title is to find one somewhere else.
//!
//! AniDB is the right somewhere else because its titles are **language-tagged**
//! — `<aid>|<type>|<language>|<title>`, where type 4 is an official title and
//! the language is an explicit ISO code. That means no guessing about which of
//! a pile of alternative titles happens to be the English one, which is exactly
//! the failure mode that made the unlabeled community datasets unusable here.
//!
//! Entries are matched by normalised romaji title rather than by ID. AniDB IDs
//! aren't in AniList's data, and the only public crosswalk is a 62 MB weekly
//! dataset carrying share-alike obligations; matching on the title costs a
//! 1.4 MB download instead. A romaji title that matches more than one AniDB
//! entry is skipped rather than guessed at.
//!
//! # Rate limit
//!
//! AniDB bans clients that request the dump more than once per day. Every path
//! through [`load_or_refresh`] records the attempt *before* making it, so a
//! failed download cannot turn into a retry loop.

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::engine::storage::Storage;

const DUMP_URL: &str = "https://anidb.net/api/anime-titles.dat.gz";
const CACHE_FILE: &str = "anidb-titles.dat";
/// AniDB's hard limit is one request per day. Nothing here may lower it.
const MIN_REFRESH_SECS: i64 = 24 * 60 * 60;
const LAST_ATTEMPT_KEY: &str = "anidb_titles_last_attempt";

/// Build the key two romanisations of the same Japanese title should share.
///
/// AniList and AniDB romanise independently, and their disagreements are
/// systematic rather than random: word division ("Sumomomo Momomo" vs "Sumomo
/// mo Momo mo", "Maoujou" vs "Maou jou"), the direct-object particle spelled
/// `wo` or `o`, and long vowels written `ou`/`oo`/`uu` or bare. Comparing on
/// the literal spelling misses well over half the entries that both databases
/// actually hold.
///
/// So the key folds all three away: particle `wo` → `o`, then whitespace is
/// removed entirely, then long vowels are collapsed. Two titles that survive to
/// the same key are still compared for *exact* equality — this widens what
/// counts as the same spelling, it does not introduce fuzzy matching. Measured
/// against 490 library rows where AniList supplies the English title, the key
/// agrees with AniList 96.9% of the time, and only 0.33% of AniDB's keys are
/// ambiguous (those are rejected outright by [`AniDbTitles::english_for`]).
fn match_key(title: &str) -> String {
    let base = title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>();
    let folded: Vec<&str> = base
        .split_whitespace()
        .map(|w| if w == "wo" { "o" } else { w })
        .collect();
    folded
        .concat()
        .replace("ou", "o")
        .replace("uu", "u")
        .replace("oo", "o")
}

/// Keys shorter than this are too collision-prone to trust.
const MIN_KEY_LEN: usize = 6;

/// AniDB files theatrical releases under a "Gekijouban" ("movie version")
/// prefix that AniList omits, so the film's entry never matches on the literal
/// title. Indexing a prefix-stripped alias as well recovers those.
///
/// Conflating a film with its TV series is prevented by the ambiguity guard
/// rather than by this function: where the film has no distinguishing subtitle,
/// its stripped alias collides with the series and both are rejected.
fn strip_movie_prefix(key: &str) -> Option<&str> {
    for p in ["gekijouban", "gekijoban", "gekijōban"] {
        if let Some(rest) = key.strip_prefix(p) {
            if rest.len() >= MIN_KEY_LEN {
                return Some(rest);
            }
        }
    }
    None
}

/// AniDB's dump encodes apostrophes as backticks, so raw values read as
/// "The World`s Strongest Witch".
fn clean(title: &str) -> String {
    title.replace('`', "'").trim().to_string()
}

/// An indexed AniDB title dump: normalised title → anime id, and anime id →
/// best English title.
#[derive(Debug, Default)]
pub struct AniDbTitles {
    by_title: HashMap<String, Vec<u32>>,
    english: HashMap<u32, String>,
}

impl AniDbTitles {
    /// Index a dump. Lines are `<aid>|<type>|<language>|<title>`; comments start
    /// with `#`.
    ///
    /// Type 4 (official) beats type 1 (main) beats types 2/3 (synonym, short),
    /// so a licensor-confirmed title always wins over a community-supplied one.
    pub fn parse(dat: &str) -> Self {
        let mut by_title: HashMap<String, Vec<u32>> = HashMap::new();
        let mut best: HashMap<u32, (u8, String)> = HashMap::new();

        for line in dat.lines() {
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            let mut parts = line.splitn(4, '|');
            let (Some(aid), Some(ty), Some(lang), Some(title)) =
                (parts.next(), parts.next(), parts.next(), parts.next())
            else {
                continue;
            };
            let Ok(aid) = aid.parse::<u32>() else { continue };
            let title = clean(title);
            if title.is_empty() {
                continue;
            }

            // Type 3 is a short title — "KnY", "GIF", "S! M". These are
            // abbreviations, never display names, on either side of the lookup.
            if lang == "en" && ty != "3" {
                let rank = match ty {
                    "4" => 3u8, // official title for a language
                    "1" => 2,   // primary title
                    _ => 1,     // synonym
                };
                match best.get(&aid) {
                    Some((existing, _)) if *existing >= rank => {}
                    _ => {
                        best.insert(aid, (rank, title.clone()));
                    }
                }
            }

            // Romaji and Japanese titles are what an AniList entry is matched
            // against. Short titles (type 3) are abbreviations like "GIF" and
            // would collide wildly, so they are not indexed.
            if matches!(lang, "x-jat" | "ja") && ty != "3" {
                let key = match_key(&title);
                if key.len() >= MIN_KEY_LEN {
                    let alias = strip_movie_prefix(&key).map(str::to_string);
                    for k in std::iter::once(key).chain(alias) {
                        let ids = by_title.entry(k).or_default();
                        if !ids.contains(&aid) {
                            ids.push(aid);
                        }
                    }
                }
            }
        }

        AniDbTitles {
            by_title,
            english: best.into_iter().map(|(k, (_, t))| (k, t)).collect(),
        }
    }

    /// The English title AniDB records for `romaji`, or `None` when there is no
    /// match, no English title, or — deliberately — more than one candidate
    /// entry. An ambiguous match is a guess, and guesses are what this whole
    /// design avoids.
    pub fn english_for(&self, romaji: &str) -> Option<&str> {
        let key = match_key(romaji);
        if key.len() < MIN_KEY_LEN {
            return None;
        }
        let ids = self.by_title.get(&key)?;
        let [aid] = ids[..] else { return None };
        self.english.get(&aid).map(String::as_str)
    }

    pub fn is_empty(&self) -> bool {
        self.english.is_empty()
    }
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn cache_path(data_dir: &Path) -> PathBuf {
    data_dir.join(CACHE_FILE)
}

/// Age of the cached dump in seconds, or `None` if there isn't one.
fn cache_age(path: &Path) -> Option<i64> {
    let meta = std::fs::metadata(path).ok()?;
    let modified = meta.modified().ok()?;
    let secs = modified.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64;
    Some(now_secs() - secs)
}

async fn download(url: &str) -> anyhow::Result<String> {
    // AniDB blocks generic clients; identify the app explicitly.
    let client = reqwest::Client::builder()
        .user_agent(concat!("AniVault/", env!("CARGO_PKG_VERSION")))
        .build()?;
    let bytes = client.get(url).send().await?.error_for_status()?.bytes().await?;
    let mut out = String::new();
    flate2::read::GzDecoder::new(&bytes[..]).read_to_string(&mut out)?;
    Ok(out)
}

/// Return an indexed dump, downloading a fresh copy at most once per day.
///
/// Falls back to a stale cache whenever a download is not allowed or fails, and
/// returns `None` only when there is nothing usable at all — in which case the
/// caller simply derives no titles from AniDB this cycle.
pub async fn load_or_refresh(storage: &Storage, data_dir: &Path) -> Option<AniDbTitles> {
    let path = cache_path(data_dir);
    let age = cache_age(&path);

    if let Some(age) = age {
        if age < MIN_REFRESH_SECS {
            return std::fs::read_to_string(&path).ok().map(|d| AniDbTitles::parse(&d));
        }
    }

    // Record the attempt before making it: AniDB counts requests, not successes,
    // and a retry loop here gets the user's IP banned.
    let last_attempt = storage
        .get_setting(LAST_ATTEMPT_KEY)
        .await
        .ok()
        .flatten()
        .and_then(|v| v.trim_matches('"').parse::<i64>().ok())
        .unwrap_or(0);
    let now = now_secs();
    if now - last_attempt < MIN_REFRESH_SECS {
        return std::fs::read_to_string(&path).ok().map(|d| AniDbTitles::parse(&d));
    }
    let _ = storage.set_setting(LAST_ATTEMPT_KEY, &now.to_string(), now).await;

    match download(DUMP_URL).await {
        Ok(dat) => {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Err(e) = std::fs::write(&path, &dat) {
                tracing::warn!("could not cache AniDB title dump: {e}");
            }
            tracing::info!(bytes = dat.len(), "refreshed AniDB title dump");
            Some(AniDbTitles::parse(&dat))
        }
        Err(e) => {
            tracing::warn!("AniDB title dump download failed: {e}");
            std::fs::read_to_string(&path).ok().map(|d| AniDbTitles::parse(&d))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DUMP: &str = "\
# created: Sat Aug 22 03:00:02 2026
# <aid>|<type>|<language>|<title>
100|1|x-jat|Sekai Saikyou no Majo, Hajimemashita
100|2|en|The World`s Strongest Witch
200|1|x-jat|Pen to Wappa to Jijitsukon
200|4|en|A Pen, Handcuffs, and a Common-Law Marriage
200|2|en|A Pen
300|1|x-jat|Kimetsu no Yaiba
300|3|en|KnY
400|1|x-jat|Duplicate Title
500|1|x-jat|Duplicate Title
400|4|en|First Claimant
500|4|en|Second Claimant
600|1|x-jat|No English Here
";

    #[test]
    fn finds_an_english_title_by_romaji() {
        let t = AniDbTitles::parse(DUMP);
        assert_eq!(
            t.english_for("Sekai Saikyou no Majo, Hajimemashita"),
            Some("The World's Strongest Witch"),
            "backtick apostrophes must be normalised"
        );
    }

    #[test]
    fn an_official_title_outranks_a_synonym() {
        let t = AniDbTitles::parse(DUMP);
        assert_eq!(
            t.english_for("Pen to Wappa to Jijitsukon"),
            Some("A Pen, Handcuffs, and a Common-Law Marriage")
        );
    }

    #[test]
    fn matching_ignores_punctuation_and_case() {
        let t = AniDbTitles::parse(DUMP);
        assert_eq!(
            t.english_for("sekai saikyou no majo hajimemashita"),
            Some("The World's Strongest Witch")
        );
    }

    /// Every pair below is one real AniList title and AniDB's own differing
    /// romanisation of the same show. All of these were missed before the key
    /// folded word division, the `wo`/`o` particle and long vowels.
    #[test]
    fn matches_across_romanisation_differences() {
        const PAIRS: &[(&str, &str)] = &[
            // word division
            ("Sumomomo Momomo: Chijou Saikyou no Yome", "Sumomo mo Momo mo: Chijou Saikyou no Yome"),
            ("Sudachi no Maoujou", "Sudachi no Maou-jou"),
            (
                "Yowaki MAX Reijou Nano ni, Ratsuwan Konyakusha-sama no Kake",
                "Yowaki Max Reijou na no ni, Ratsuwan Kon'yakusha-sama no Kake",
            ),
            // the direct-object particle
            ("Gacha wo Mawashite Nakama wo Fuyasu", "Gacha o Mawashite Nakama o Fuyasu"),
            ("Isshiki-san wa Koi wo Shiritai.", "Isshiki-san wa Koi o Shiritai"),
            // long vowels
            ("Hyouken no Majutsushi ga Sekai wo Suberu", "Hyoken no Majutsushi ga Sekai o Suberu"),
        ];
        for (anilist, anidb) in PAIRS {
            assert_eq!(
                match_key(anilist),
                match_key(anidb),
                "should have matched:\n  AniList: {anilist}\n  AniDB  : {anidb}"
            );
        }
    }

    /// Widening the key must not merge genuinely different entries. A "Special"
    /// is not its parent season — matching those was the concrete regression
    /// that ruled out fuzzy matching.
    #[test]
    fn distinct_shows_keep_distinct_keys() {
        const DISTINCT: &[(&str, &str)] = &[
            (
                "Arifureta Shokugyou de Sekai Saikyou 2nd season Special",
                "Arifureta Shokugyou de Sekai Saikyou 2nd season",
            ),
            ("Shangri-La Frontier 3rd Season", "Shangri-La Frontier"),
            ("初音 Mix", "Hatsune Mix"),
            ("Fate/stay night", "Fate/Zero"),
        ];
        for (a, b) in DISTINCT {
            assert_ne!(match_key(a), match_key(b), "must not conflate {a:?} with {b:?}");
        }
    }

    #[test]
    fn finds_a_film_anidb_files_under_its_movie_prefix() {
        let dump = "\
700|1|x-jat|Gekijouban Kage no Jitsuryokusha ni Naritakute! Zankyou-hen
700|4|en|The Eminence in Shadow: Lost Echoes
";
        let t = AniDbTitles::parse(dump);
        assert_eq!(
            t.english_for("Kage no Jitsuryokusha ni Naritakute!: Zankyou-hen"),
            Some("The Eminence in Shadow: Lost Echoes")
        );
    }

    #[test]
    fn a_film_without_its_own_subtitle_stays_ambiguous_with_the_series() {
        // Stripping the prefix makes the film's alias collide with the TV
        // series; neither may win that tie.
        let dump = "\
800|1|x-jat|Kimagure Orange Road
800|4|en|TV Series Title
801|1|x-jat|Gekijouban Kimagure Orange Road
801|4|en|Movie Title
";
        let t = AniDbTitles::parse(dump);
        assert_eq!(t.english_for("Kimagure Orange Road"), None);
    }

    #[test]
    fn a_key_too_short_to_be_trusted_is_refused() {
        // "Rec" and friends collide with everything once spaces are gone.
        let t = AniDbTitles::parse("1|1|x-jat|Rec\n1|4|en|Something\n");
        assert_eq!(t.english_for("Rec"), None);
    }

    #[test]
    fn an_ambiguous_title_is_skipped_rather_than_guessed() {
        let t = AniDbTitles::parse(DUMP);
        assert_eq!(t.english_for("Duplicate Title"), None);
    }

    #[test]
    fn returns_nothing_when_there_is_no_english_title() {
        let t = AniDbTitles::parse(DUMP);
        assert_eq!(t.english_for("No English Here"), None);
        assert_eq!(t.english_for("Kimetsu no Yaiba"), None, "a short title is not a title");
        assert_eq!(t.english_for("Something Absent"), None);
    }

    #[test]
    fn a_comment_only_dump_indexes_nothing() {
        assert!(AniDbTitles::parse("# just a header\n").is_empty());
        assert!(AniDbTitles::parse("").is_empty());
    }

    #[test]
    fn malformed_lines_are_skipped_without_panicking() {
        let t = AniDbTitles::parse(
            "garbage\n|||\nabc|1|x-jat|Title\n1|1|x-jat|Kimagure Orange Road\n1|4|en|Fine\n",
        );
        assert_eq!(t.english_for("Kimagure Orange Road"), Some("Fine"));
    }
}
