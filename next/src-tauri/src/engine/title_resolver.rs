//! Derives a readable English display title for anime that AniList has no
//! `title.english` for.
//!
//! AniList leaves `english` null on a large share of sequel seasons: the
//! franchise has a well-known English title recorded on season one, and it
//! simply hasn't been backfilled onto season two or three. The result is a
//! library that shows "Sousou no Frieren 3rd Season" next to "Frieren: Beyond
//! Journey's End Season 2".
//!
//! This module closes that gap without inventing anything. It never translates
//! — every value it produces is an English title AniList already published on a
//! *related* entry, with the season marker carried across. Two rules keep it
//! from damaging rows that were already fine:
//!
//! 1. [`looks_unresolved`] gates the whole pass. Titles like "Madlax", "Noir"
//!    or "Yakitate!! Japan" have no English title because the romaji *is* the
//!    English name; deriving anything for those makes the library worse.
//! 2. [`derive_from_relation`] only fires when the entry carries an explicit
//!    season number. Without one there is no safe way to tell a numbered sequel
//!    from a side story or a film with its own distinct title.
//!
//! Results are written to `titles_json.english_derived`, never to `english`, so
//! AniList stays authoritative and a later sync silently corrects any guess.

use regex::Regex;
use std::sync::LazyLock;

/// Japanese function words (particles, copulas, common verb endings) plus the
/// stock vocabulary of light-novel titles. A romanised title almost always
/// contains at least two; an English one almost never does.
const ROMAJI_WORDS: &[&str] = &[
    // particles and conjunctions
    "no", "ga", "wa", "wo", "ni", "de", "to", "mo", "ka", "ya", "na", "yo", "ne", "he", "kara",
    "made", "yori", "dake", "sae", "koso", "nado", "hodo", "shika", "nara", "node", "noni",
    "kedo", "demo", "nanoni", "nano", "datta", "dakedo", "toshite", "nite",
    // verbs and copulas
    "suru", "shita", "shite", "sareta", "sarete", "saseru", "naru", "natta", "naritai",
    "naritakute", "nai", "desu", "masu", "aru", "iru", "ita", "itta", "kuru", "kita", "miru",
    "mita", "ikiru", "tsukuru", "tsukuriagero", "shiritai", "shitakedo", "hajimemashita",
    "oidasareta", "michibiku", "suberu", "nariagaru", "tsuujinai", "akogarete", "notte",
    "shimatta", "fuyasu", "mawashite", "tensei", "tenseishita",
    // stock title nouns
    "isekai", "sekai", "saikyou", "saikou", "shoujo", "shounen", "mahou", "maou", "yuusha",
    "kizoku", "reijou", "ore", "boku", "watashi", "kimi", "anata", "kare", "kanojo",
    "majutsushi", "majutsu", "boukensha", "ansatsusha", "chiyushi", "yome", "tsuma",
    "douchuu", "jitsuryokusha", "kokoro", "yabai", "yatsu", "koi", "ai", "yume", "tabi",
    "shokugyou", "binbou", "kiyou", "moto", "kouho", "mattari", "fushi", "nozomanu",
    "tonari", "bosotto", "dereru", "tokidoki", "subete", "kanchigai", "gyaku", "shitu",
    "hen", "ki", "dai", "ban", "hajime",
];

/// Hepburn long-vowel and geminate clusters. These are pervasive in romanised
/// Japanese and vanishingly rare inside English words, so they catch romaji the
/// word list misses ("Saishuu", "Shinkakusha", "Hanaukyou").
static ROMAJI_MORPH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(ou[a-z]|uu|kyou|shuu|chuu|ryuu|jou|tsu|ssh|kk|tt[aeiou]n|ou$|sha$)").unwrap()
});

static WORD_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[A-Za-z][A-Za-z'-]*").unwrap());

/// `3rd Season` / `Season 2` / `2nd season` — the season marker AniList appends
/// to romaji sequel titles, together with any separator introducing it.
///
/// A `-` only counts as a separator when whitespace precedes it. Without that
/// guard the closing dash of a bracketed subtitle gets eaten, turning
/// "TSUKIMICHI -Moonlit Fantasy- Season 2" into "TSUKIMICHI -Moonlit Fantasy".
static SEASON_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(?:\s*:|\s+[\-–])?\s*(?:(\d+)(?:st|nd|rd|th)\s+season|season\s+(\d+))\s*$",
    )
    .unwrap()
});

/// A parenthesised format qualifier — "(Special)", "(OVA)", "(Movie)". A
/// related entry carrying one is a spin-off rather than the franchise's main
/// line, and its title must not become a numbered season's display name.
static FORMAT_QUALIFIER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\((?:special|specials|ova|ona|movie|tv|film)\)").unwrap());

/// A trailing roman numeral, as in "Hyouken no Majutsushi ga Sekai wo Suberu II".
static ROMAN_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+(II|III|IV|V)$").unwrap());

/// How strongly a title reads as romanised Japanese. Counts a word-list hit or a
/// romaji morphology hit once per word.
fn romaji_score(title: &str) -> usize {
    WORD_RE
        .find_iter(title)
        .filter(|m| {
            let w = m.as_str().to_lowercase();
            let base = w.trim_matches(|c| c == '-' || c == '\'');
            if base.is_empty() {
                return false;
            }
            if ROMAJI_WORDS.contains(&base) {
                return true;
            }
            base.len() >= 5 && ROMAJI_MORPH.is_match(base)
        })
        .count()
}

/// Does this title still read as untranslated Japanese?
///
/// Deliberately conservative: it requires two independent signals, so it would
/// rather decline to help than rewrite a title that was already correct. Titles
/// whose romaji *is* the released English name ("Madlax", "Charlotte", "Final
/// Approach", "Dandadan 3rd Season") score below the threshold and are left
/// alone.
pub fn looks_unresolved(title: &str) -> bool {
    romaji_score(title) >= 2
}

/// The season number a title advertises, if any.
pub fn season_number(title: &str) -> Option<u32> {
    if let Some(c) = SEASON_RE.captures(title) {
        let n = c.get(1).or_else(|| c.get(2))?;
        return n.as_str().parse().ok();
    }
    ROMAN_RE.captures(title).map(|c| match &c[1] {
        "II" => 2,
        "III" => 3,
        "IV" => 4,
        _ => 5,
    })
}

/// The title with any trailing season marker removed, so a base name can be
/// recombined with a different season number.
pub fn strip_season(title: &str) -> String {
    let s = SEASON_RE.replace(title, "");
    let s = ROMAN_RE.replace(&s, "");
    s.trim().to_string()
}

/// Build a display title for `romaji` from the English title of a related entry
/// (its prequel or parent).
///
/// Returns `None` unless `romaji` carries an explicit season number. That
/// restriction is the safety property: a numbered sequel reliably shares its
/// franchise's English name, whereas an unnumbered side story, OVA or film
/// frequently has a title of its own that must not be overwritten.
pub fn derive_from_relation(romaji: &str, relation_english: &str) -> Option<String> {
    let season = season_number(romaji)?;
    // A special or OVA sits in the relation graph alongside the real seasons but
    // frequently carries an episode-specific title. Borrowing that would label
    // season two after a one-off bonus episode.
    if FORMAT_QUALIFIER_RE.is_match(relation_english) {
        return None;
    }
    let base = strip_season(relation_english);
    if base.is_empty() {
        return None;
    }
    Some(format!("{base} Season {season}"))
}

/// Relation types whose English title a sequel may inherit. `SEQUEL` is
/// excluded on purpose — a later season's title should never flow backwards
/// onto an earlier entry.
pub fn is_inheritable_relation(relation_type: &str) -> bool {
    matches!(relation_type, "PREQUEL" | "PARENT")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Titles whose romaji is already the name people know them by. Deriving
    /// anything for these makes the library worse, so the gate must reject
    /// every one. These are real rows from a 645-anime library.
    const LEAVE_ALONE: &[&str] = &[
        "Yakitate!! Japan",
        "SHUFFLE!",
        "Final Approach",
        "HUNTER×HUNTER: Greed Island",
        "Whistle!",
        "Rizelmine",
        "Yumeria",
        "Madlax",
        "Noir",
        "Happy World!",
        "GIRLS Bravo: second season",
        "I My Me! Strawberry Eggs",
        "Lamune",
        "Rec",
        "Initial D BATTLE STAGE",
        "Slayers Great",
        "Saber Marionette J Again",
        "Happiness!",
        "Gift: eternal rainbow",
        "Nodame Cantabile",
        "Shuffle! Memories",
        "Sekirei",
        "Princess Lover!",
        "Amagami SS+ plus",
        "Initial D Fifth Stage",
        "Charlotte",
        "DARK MACHINE: The Animation",
        "Dandadan 3rd Season",
        "Re:Monster 2nd Season",
        "Shangri-La Frontier 3rd Season",
    ];

    /// Titles that genuinely still read as Japanese.
    const SHOULD_DERIVE: &[&str] = &[
        "Sousou no Frieren 3rd Season",
        "Boku no Kokoro no Yabai Yatsu 3rd Season",
        "Mahou Shoujo ni Akogarete 2nd Season",
        "Nozomanu Fushi no Boukensha 2nd Season",
        "Tensei Kizoku, Kantei Skill de Nariagaru 3rd Season",
        "Tsuki ga Michibiku Isekai Douchuu 3rd Season",
        "Kage no Jitsuryokusha ni Naritakute!: Zankyou-hen",
        "Hyouken no Majutsushi ga Sekai wo Suberu II",
        "Yuusha Party wo Oidasareta Kiyou Binbou 2nd Season",
        "Sekai Saikou no Ansatsusha, Isekai Kizoku ni Tensei suru 2nd Season",
        "Tokidoki Bosotto Rossiya-go de Dereru Tonari no Alya-san Season 2",
        "Kuroiwa Medaka ni Watashi no Kawaii ga Tsuujinai 2nd Season",
        "Isekai Maou to Shoukan Shoujo no Dorei Majutsu ULT",
        "Sumomomo Momomo: Chijou Saikyou no Yome",
        "MASHLE: Sanma Taisou Shinkakusha Saishuu Shiken-hen",
        "Tenkaichi: Nihon Saikyou Bugeisha Ketteisen",
        "Sekai Saikyou no Majo, Hajimemashita",
        "Lv2 Kara Cheat datta Moto Yuusha Kouho no Mattari Isekai Life 2nd Season",
    ];

    #[test]
    fn gate_never_touches_titles_that_are_already_correct() {
        for t in LEAVE_ALONE {
            assert!(
                !looks_unresolved(t),
                "would have damaged an already-correct title: {t}"
            );
        }
    }

    #[test]
    fn gate_catches_untranslated_romaji() {
        for t in SHOULD_DERIVE {
            assert!(looks_unresolved(t), "failed to flag romaji title: {t}");
        }
    }

    #[test]
    fn reads_season_numbers_in_every_form_anilist_uses() {
        assert_eq!(season_number("Sousou no Frieren 3rd Season"), Some(3));
        assert_eq!(season_number("Alya-san Season 2"), Some(2));
        assert_eq!(season_number("Mahou Shoujo ni Akogarete 2nd Season"), Some(2));
        assert_eq!(season_number("Hyouken no Majutsushi ga Sekai wo Suberu II"), Some(2));
        assert_eq!(season_number("Sousou no Frieren"), None);
    }

    #[test]
    fn strips_the_season_marker_from_a_base_name() {
        assert_eq!(
            strip_season("Frieren: Beyond Journey's End Season 2"),
            "Frieren: Beyond Journey's End"
        );
        assert_eq!(strip_season("The Dangers in My Heart Season 2"), "The Dangers in My Heart");
        assert_eq!(strip_season("Gushing Over Magical Girls"), "Gushing Over Magical Girls");
        assert_eq!(strip_season("MASHLE - Season 2"), "MASHLE");
        assert_eq!(strip_season("Overlord: Season 2"), "Overlord");
    }

    #[test]
    fn keeps_the_closing_dash_of_a_bracketed_subtitle() {
        // Regression: a greedy separator rule ate the trailing dash and produced
        // "TSUKIMICHI -Moonlit Fantasy Season 3".
        assert_eq!(
            derive_from_relation(
                "Tsuki ga Michibiku Isekai Douchuu 3rd Season",
                "TSUKIMICHI -Moonlit Fantasy- Season 2"
            ),
            Some("TSUKIMICHI -Moonlit Fantasy- Season 3".into())
        );
    }

    #[test]
    fn refuses_to_inherit_a_specials_title() {
        // Observed on a real library row: the related entry was a bonus episode
        // whose title would have mislabelled the whole season.
        assert_eq!(
            derive_from_relation(
                "Isekai de Cheat Skill wo Te ni Shita Ore wa 2nd Season",
                "I Got a Cheat Skill in Another World (Special) - The Legendary Dragon Awakens"
            ),
            None
        );
        assert_eq!(derive_from_relation("Foo 2nd Season", "Some Show (OVA)"), None);
        assert_eq!(derive_from_relation("Foo 2nd Season", "Some Show (Movie)"), None);
    }

    #[test]
    fn carries_the_season_number_onto_the_franchise_title() {
        assert_eq!(
            derive_from_relation(
                "Sousou no Frieren 3rd Season",
                "Frieren: Beyond Journey's End Season 2"
            ),
            Some("Frieren: Beyond Journey's End Season 3".into())
        );
        assert_eq!(
            derive_from_relation("Mahou Shoujo ni Akogarete 2nd Season", "Gushing Over Magical Girls"),
            Some("Gushing Over Magical Girls Season 2".into())
        );
        assert_eq!(
            derive_from_relation(
                "Hyouken no Majutsushi ga Sekai wo Suberu II",
                "The Iceblade Sorcerer Shall Rule the World"
            ),
            Some("The Iceblade Sorcerer Shall Rule the World Season 2".into())
        );
    }

    #[test]
    fn refuses_to_derive_without_an_explicit_season_number() {
        // A side story or film may carry a title of its own; inheriting the
        // franchise name here would silently mislabel it.
        assert_eq!(
            derive_from_relation("Kage no Jitsuryokusha ni Naritakute!: Zankyou-hen", "The Eminence in Shadow"),
            None
        );
        assert_eq!(derive_from_relation("Sumomomo Momomo: Chijou Saikyou no Yome", "Whatever"), None);
    }

    #[test]
    fn refuses_to_derive_from_an_empty_relation_title() {
        assert_eq!(derive_from_relation("Sousou no Frieren 3rd Season", ""), None);
        assert_eq!(derive_from_relation("Sousou no Frieren 3rd Season", "Season 2"), None);
    }

    #[test]
    fn only_earlier_entries_are_inheritable() {
        assert!(is_inheritable_relation("PREQUEL"));
        assert!(is_inheritable_relation("PARENT"));
        assert!(!is_inheritable_relation("SEQUEL"));
        assert!(!is_inheritable_relation("SIDE_STORY"));
        assert!(!is_inheritable_relation("CHARACTER"));
    }
}
