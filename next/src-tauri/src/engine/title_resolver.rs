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

/// A trailing season marker together with any separator introducing it:
/// `3rd Season`, `Season 2`, `(The) Final Season`, each optionally followed by a
/// split-cour `Part N`, or a bare `Part N`.
///
/// A `-` only counts as a separator when whitespace precedes it. Without that
/// guard the closing dash of a bracketed subtitle gets eaten, turning
/// "TSUKIMICHI -Moonlit Fantasy- Season 2" into "TSUKIMICHI -Moonlit Fantasy".
static SEASON_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(?:\s*:|\s+[\-–])?\s*(?:(?:\d+(?:st|nd|rd|th)\s+season|season\s+\d+|(?:the\s+)?final\s+season)(?:\s*[:\-–]?\s*part\s+(?:\d+|[ivx]+))?|part\s+(?:\d+|[ivx]+))\s*$",
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
    WORD_RE.find_iter(title).filter(|m| is_romaji_word(m.as_str())).count()
}

fn is_romaji_word(word: &str) -> bool {
    let w = word.to_lowercase();
    let base = w.trim_matches(|c| c == '-' || c == '\'');
    if base.is_empty() {
        return false;
    }
    if ROMAJI_WORDS.contains(&base) {
        return true;
    }
    base.len() >= 5 && ROMAJI_MORPH.is_match(base)
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

/// The season number a title advertises, if any. Delegates to the matcher's
/// parser so recognition and derived titles never disagree about a season.
pub fn season_number(title: &str) -> Option<u32> {
    crate::engine::parser::season_in_title(title).map(|s| s as u32)
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

/// English function words. Their presence is what separates a translated title
/// ("Now That We Draw") from an abbreviation ("BTR2") or a romanised
/// alternative ("Otasukebu"), all of which AniList files under `synonyms`.
/// "to" is also a Japanese particle; in a synonym, already known not to be the
/// romaji, it counts as English.
const ENGLISH_WORDS: &[&str] = &[
    "the", "to", "a", "an", "of", "and", "or", "in", "on", "at", "for", "with", "without", "from",
    "into", "about", "after", "is", "am", "are", "was", "were", "be", "been", "i", "i'm", "me",
    "my", "we", "our", "you", "your", "he's", "his", "she", "her", "they", "their", "it", "its",
    "this", "that", "who", "what", "when", "how", "why", "not", "can't", "don't", "will", "so",
    "but", "if", "as", "by", "every", "all",
];

fn is_english_word(word: &str) -> bool {
    ENGLISH_WORDS.contains(&word.to_lowercase().trim_matches('-'))
}

fn english_score(title: &str) -> usize {
    WORD_RE.find_iter(title).filter(|m| is_english_word(m.as_str())).count()
}

/// Pick the English title AniList filed under `synonyms` when it left
/// `title.english` empty — "Kakunaru Ue wa" lists "Now That We Draw" there.
///
/// A synonym qualifies only when it reads unmistakably as English: at least two
/// English function words, more of them than romaji signals, and more than the
/// romaji title itself carries (so "Bocchi the Rock!" is never swapped for a
/// rewording of itself). Abbreviations, romanised alternatives and Japanese
/// script all fail that test, and so does a synonym that merely extends the
/// romaji with a subtitle ("Lamune ~ The Glass Jar that Reflects the Sea"),
/// whose romaji is already the English name. The first qualifying synonym wins,
/// which is the order AniList lists them in.
pub fn english_synonym(romaji: &str, synonyms: &[String]) -> Option<String> {
    let romaji_english = english_score(romaji);
    let romaji_lower = romaji.trim().to_lowercase();
    synonyms
        .iter()
        .map(|s| s.trim())
        .find(|s| {
            if !romaji_lower.is_empty() && s.to_lowercase().starts_with(&romaji_lower) {
                return false;
            }
            let english = english_score(s);
            let romaji = WORD_RE
                .find_iter(s)
                .filter(|m| is_romaji_word(m.as_str()) && !is_english_word(m.as_str()))
                .count();
            english >= 2 && english > romaji && english > romaji_english
        })
        .map(String::from)
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
    fn season_number_agrees_with_the_matcher() {
        use crate::engine::parser::season_in_title;
        for t in [
            "Tensei shitara Slime Datta Ken Season 2 Part 2",
            "Mushoku Tensei II: Isekai Ittara Honki Dasu",
            "Mushoku Tensei: Isekai Ittara Honki Dasu Part 2",
            "Sousou no Frieren 3rd Season",
            "Lupin III: Part 6",
        ] {
            assert_eq!(
                season_number(t),
                season_in_title(t).map(|s| s as u32),
                "disagree on {t}"
            );
        }
        assert_eq!(season_number("Tensei shitara Slime Datta Ken Season 2 Part 2"), Some(2));
    }

    #[test]
    fn strips_split_cour_and_final_season_markers() {
        assert_eq!(
            strip_season("That Time I Got Reincarnated as a Slime Season 2 Part 2"),
            "That Time I Got Reincarnated as a Slime"
        );
        assert_eq!(
            strip_season("Mushoku Tensei: Jobless Reincarnation Part 2"),
            "Mushoku Tensei: Jobless Reincarnation"
        );
        assert_eq!(strip_season("Attack on Titan Final Season"), "Attack on Titan");
        assert_eq!(
            strip_season("Attack on Titan: The Final Season Part 2"),
            "Attack on Titan"
        );
        assert_eq!(strip_season("Bocchi the Rock! 2nd Season Part 2"), "Bocchi the Rock!");
        // A title that merely contains the word stays intact.
        assert_eq!(strip_season("Part-Time Heroes"), "Part-Time Heroes");
    }

    #[test]
    fn derives_from_a_split_cour_prequel_without_doubling_markers() {
        assert_eq!(
            derive_from_relation(
                "Tensei shitara Slime Datta Ken 3rd Season",
                "That Time I Got Reincarnated as a Slime Season 2 Part 2"
            ),
            Some("That Time I Got Reincarnated as a Slime Season 3".into())
        );
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

    fn syn(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn picks_the_english_title_anilist_filed_as_a_synonym() {
        // Real Plan to Watch rows whose `title.english` is null.
        let cases = [
            ("Kakunaru Ue wa", syn(&["Now That We Draw"]), "Now That We Draw"),
            (
                "Isshiki-san wa Koi wo Shiritai.",
                syn(&["Isshiki-san Wants to Know About Love."]),
                "Isshiki-san Wants to Know About Love.",
            ),
            (
                "Mayonaka Heart Tune 2nd Season",
                syn(&["Tune In to the Midnight Heart Season 2", "Mayochu 2"]),
                "Tune In to the Midnight Heart Season 2",
            ),
            (
                "Megane, Tokidoki, Yankee-kun",
                syn(&["Otasukebu", "Glasses With a Chance of Delinquent"]),
                "Glasses With a Chance of Delinquent",
            ),
            (
                "Kaketa Tsuki no Mercedes",
                syn(&[
                    "Mercedes and the Waning Moon: The Dungeoneering Feats of a Discarded Vampire Aristocrat",
                    "欠けた月のメルセデス",
                    "Kaketa Tsuki no Mercedes: Kyuuketsuki no Kizoku ni Tensei Shitakedo Sute Rare Sounanode Dungeon wo Seiha Suru",
                ]),
                "Mercedes and the Waning Moon: The Dungeoneering Feats of a Discarded Vampire Aristocrat",
            ),
            (
                "Doukyo Shiteiru Kensei no Onna Shishou ga Kawaisugite Mainichi Shiawase desu",
                syn(&["My Female Sword Master, Who I Live with, Is So Cute That I’m Happy Every Day"]),
                "My Female Sword Master, Who I Live with, Is So Cute That I’m Happy Every Day",
            ),
            (
                "Futsutsuka na Akujo de wa Gozaimasu ga: Suuguu Chouso Torikae Den Part 2",
                syn(&["Though I Am an Inept Villainess Part 2"]),
                "Though I Am an Inept Villainess Part 2",
            ),
        ];
        for (romaji, synonyms, want) in cases {
            assert_eq!(english_synonym(romaji, &synonyms).as_deref(), Some(want), "{romaji}");
        }
    }

    #[test]
    fn ignores_abbreviations_and_romanised_synonyms() {
        assert_eq!(english_synonym("Bocchi the Rock! 2nd Season", &syn(&["BTR2"])), None);
        assert_eq!(english_synonym("Mayonaka Heart Tune", &syn(&["Mayochu"])), None);
        assert_eq!(
            english_synonym(
                "Kaketa Tsuki no Mercedes",
                &syn(&["Kaketa Tsuki no Mercedes: Kyuuketsuki no Kizoku ni Tensei Shitakedo Sute Rare Sounanode Dungeon wo Seiha Suru"])
            ),
            None
        );
        assert_eq!(english_synonym("Re:Monster 2nd Season", &[]), None);
    }

    #[test]
    fn never_swaps_an_english_romaji_for_a_rewording() {
        assert_eq!(
            english_synonym("The Girl Who Leapt Through Time", &syn(&["The Girl Who Leaped Through Time"])),
            None
        );
        // Real row: the romaji is the English name and the synonym only adds a
        // subtitle to it.
        assert_eq!(
            english_synonym("Lamune", &syn(&["Lamune ~ The Glass Jar that Reflects the Sea Lemonade"])),
            None
        );
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
