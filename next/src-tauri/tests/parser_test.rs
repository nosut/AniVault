use anivault_core::engine::parser::parse_filename;

fn assert_parse(input: &str, title: Option<&str>, expected_title: &str, episode: i32) {
    let result = parse_filename(input, title).unwrap();
    assert_eq!(result.episode_number, episode, "episode mismatch for '{input}'");
    let cleaned = result.cleaned_title.to_lowercase();
    assert!(
        cleaned.contains(&expected_title.to_lowercase()),
        "title mismatch for '{input}': expected '{expected_title}' in '{cleaned}'"
    );
}

#[test]
fn parse_standard_dash_separator() {
    assert_parse("Cowboy Bebop - 01.mkv", None, "Cowboy Bebop", 1);
}

#[test]
fn parse_release_group_brackets() {
    assert_parse("[HorribleSubs] Attack on Titan - 12 [1080p].mkv", None, "Attack on Titan", 12);
}

#[test]
fn parse_s01e01_format() {
    assert_parse("Fullmetal Alchemist S01E03.mkv", None, "Fullmetal Alchemist", 3);
}

#[test]
fn parse_ep_prefix() {
    assert_parse("Steins;Gate EP07.mkv", None, "Steins;Gate", 7);
}

#[test]
fn parse_year_before_ep_prefix() {
    // Regression: "Fate 2006 EP01" must extract episode 1, not 2006
    assert_parse("Fate 2006 EP01.mkv", None, "Fate", 1);
}

#[test]
fn parse_episode_prefix_lowercase() {
    assert_parse("Mushishi episode 15.mkv", None, "Mushishi", 15);
}

#[test]
fn parse_hash_prefix() {
    assert_parse("Jujutsu Kaisen - 05 [1080p][HEVC].mkv", None, "Jujutsu Kaisen", 5);
}

#[test]
fn parse_multi_season_s01e01() {
    assert_parse("My Hero Academia S03E10 [1080p].mkv", None, "My Hero Academia", 10);
}

#[test]
fn parse_hyphen_in_title() {
    assert_parse("Spy x Family - 02.mkv", None, "Spy x Family", 2);
}

#[test]
fn parse_semicolon_title() {
    assert_parse("Steins;Gate 0 - 01.mkv", None, "Steins;Gate 0", 1);
}

#[test]
fn parse_square_brackets_group() {
    assert_parse("[Erai-raws] One Piece - 1015 [1080p][HEVC].mkv", None, "One Piece", 1015);
}

#[test]
fn parse_parentheses_quality() {
    assert_parse("Violet Evergarden - 03 (1080p).mkv", None, "Violet Evergarden", 3);
}

#[test]
fn parse_leading_number_skip() {
    assert_parse("01 - Mob Psycho 100 II - 05.mkv", None, "Mob Psycho 100 II", 5);
}

#[test]
fn parse_paren_ep_num() {
    assert_parse("Barakamon (2014) - E01.mkv", None, "Barakamon", 1);
}

#[test]
fn parse_window_title() {
    assert_parse("mpv", Some("Cowboy Bebop - 05"), "Cowboy Bebop", 5);
}

#[test]
fn parse_no_match() {
    assert!(parse_filename("some_random_video.mp4", None).is_none());
}

#[test]
fn parse_episode_zero_rejected() {
    assert!(parse_filename("Show E00.mkv", None).is_none());
}

// ============================================================
// Edge case tests for AniVault filename parser expansion
// ============================================================

// 1. Batch/range patterns — first episode of a range
#[test]
fn parse_batch_range_jujutsu() {
    let result = parse_filename("[SubsPlease] Jujutsu Kaisen - 01-12 [1080p]", None)
        .expect("should parse batch range");
    assert_eq!(result.episode_number, 1, "episode 1 from range '01-12'");
    assert!(
        result.cleaned_title.to_lowercase().contains("jujutsu kaisen"),
        "title should contain 'Jujutsu Kaisen', got '{}'",
        result.cleaned_title
    );
    assert_eq!(
        result.release_group.as_deref(),
        Some("SubsPlease"),
        "release group"
    );
    assert!(result.quality.is_some(), "quality tag should be extracted");
}

#[test]
fn parse_batch_range_steins_gate() {
    let result = parse_filename("Steins;Gate - 01-24 [BD 1080p].mkv", None)
        .expect("should parse batch range");
    assert_eq!(result.episode_number, 1, "episode 1 from range '01-24'");
    assert!(
        result.cleaned_title.to_lowercase().contains("steins;gate"),
        "title should contain 'Steins;Gate', got '{}'",
        result.cleaned_title
    );
}

// 2. Spelled-out season/episode words
#[test]
fn parse_season_episode_spelled() {
    assert_parse(
        "Attack on Titan Season 2 Episode 5 [1080p].mkv",
        None,
        "Attack on Titan",
        5,
    );
}

#[test]
fn parse_second_season_episode_spelled() {
    assert_parse(
        "Mushoku Tensei 2nd Season Episode 12.mkv",
        None,
        "Mushoku Tensei",
        12,
    );
}

#[test]
fn parse_s2_ep_prefix() {
    assert_parse("Title - S2 EP05.mkv", None, "Title", 5);
}

// 3. Secondary brackets and fansub groups
#[test]
fn parse_double_brackets_fff() {
    let result = parse_filename("[[FFF] Fate Zero] - 01 [1080p].mkv", None)
        .expect("should parse double-bracket pattern");
    assert_eq!(result.episode_number, 1, "episode 1");
    assert!(
        result.cleaned_title.to_lowercase().contains("fate zero"),
        "title should contain 'Fate Zero', got '{}'",
        result.cleaned_title
    );
    assert!(result.quality.is_some(), "quality should be present");
}

#[test]
fn parse_tilde_range() {
    let result =
        parse_filename("[Erai-raws] Mushoku Tensei - 01 ~ 23 [1080p][Multiple Subtitle].mkv", None)
            .expect("should parse tilde-range");
    assert_eq!(result.episode_number, 1, "episode 1 from tilde range");
    assert!(
        result.cleaned_title.to_lowercase().contains("mushoku tensei"),
        "title should contain 'Mushoku Tensei', got '{}'",
        result.cleaned_title
    );
    assert_eq!(
        result.release_group.as_deref(),
        Some("Erai-raws"),
        "release group"
    );
    assert!(result.quality.is_some(), "quality tag should be extracted");
}

// 4. Resolution / dimensions
#[test]
fn parse_resolution_1920x1080() {
    assert_parse("Title - 01 [1920x1080].mkv", None, "Title", 1);
}

#[test]
fn parse_resolution_1280x720() {
    assert_parse("Title - 01 [1280x720 Hi10p].mkv", None, "Title", 1);
}

// 5. Decimal quality tags (e.g. 720p.10bit)
#[test]
fn parse_decimal_quality() {
    assert_parse("Title [720p.10bit][SubGroup] - 01.mkv", None, "Title", 1);
}

// 6. Release group AFTER quality bracket
#[test]
fn parse_group_after_quality() {
    assert_parse("Title - 01 [1080p][Kamikaze].mkv", None, "Title", 1);
}

// 7. BD/DVD volume notation — should not crash, gracefully returns None
#[test]
fn parse_vol1_bd() {
    let result = parse_filename("Title - Vol.1 [BD 1080p].mkv", None);
    assert!(
        result.is_none(),
        "expected None for 'Vol.1' pattern, got: {:?}",
        result
    );
}

#[test]
fn parse_bd_vol2() {
    let result = parse_filename("Title - BD Vol.2 [1080p].mkv", None);
    assert!(
        result.is_none(),
        "expected None for 'BD Vol.2' pattern, got: {:?}",
        result
    );
}

// 8. Multi-digit episodes (up to 2000)
#[test]
fn parse_four_digit_episode() {
    assert_parse("One Piece - 1000.mkv", None, "One Piece", 1000);
}

#[test]
fn parse_four_digit_episode_brackets() {
    assert_parse("Detective Conan - 1085 [1080p].mkv", None, "Detective Conan", 1085);
}

// 9. Titles that start with or contain numbers
#[test]
fn parse_title_starts_with_number() {
    let result = parse_filename("86 Eighty Six - 01 [1080p].mkv", None)
        .expect("should parse title starting with number");
    assert_eq!(result.episode_number, 1, "episode 1, not 86");
    assert!(
        result.cleaned_title.to_lowercase().contains("86 eighty six"),
        "title should contain '86 Eighty Six', got '{}'",
        result.cleaned_title
    );
}

#[test]
fn parse_title_with_number_hyphen() {
    let result = parse_filename("3-gatsu no Lion - 01.mkv", None)
        .expect("should parse title with numeric prefix");
    assert_eq!(result.episode_number, 1, "episode 1, not 3");
    assert!(
        result.cleaned_title.to_lowercase().contains("3-gatsu no lion"),
        "title should contain '3-gatsu no Lion', got '{}'",
        result.cleaned_title
    );
}

// 9b. The show title ends at the episode marker — whatever follows is the
// episode's own title, which is noise when matching against the library.
#[test]
fn episode_title_after_the_marker_is_dropped() {
    // mpv reports the mkv's embedded title: "<show> S01E07 <episode title>",
    // with none of the " - " separators the filename on disk has.
    let result = parse_filename(
        "No Longer Allowed in Another World S01E07 Will You Sentence Me to Death Again? - mpv",
        None,
    )
    .expect("should parse an mpv window title");
    assert_eq!(result.episode_number, 7);
    assert_eq!(
        result.cleaned_title, "No Longer Allowed in Another World",
        "the episode title must not survive into the search query"
    );
}

#[test]
fn episode_title_after_a_dash_separated_marker_is_dropped() {
    let result = parse_filename(
        "2.5 Dimensional Seduction - S01E03 - Lili x Miri.mkv",
        None,
    )
    .expect("should parse a dash-separated filename");
    assert_eq!(result.episode_number, 3);
    assert_eq!(result.cleaned_title, "2.5 Dimensional Seduction");
}

#[test]
fn episode_title_after_a_dash_number_marker_is_dropped() {
    let result = parse_filename("Cowboy Bebop - 05 - Ballad of Fallen Angels.mkv", None)
        .expect("should parse a dash-number filename");
    assert_eq!(result.episode_number, 5);
    assert_eq!(result.cleaned_title, "Cowboy Bebop");
}

// 10. No episode number (movie / special) — returns None gracefully
#[test]
fn parse_movie_no_episode() {
    let result = parse_filename("[SubGroup] Your Name [1080p].mkv", None);
    assert!(
        result.is_none(),
        "expected None for movie without episode, got: {:?}",
        result
    );
}

// 11. Season number — kept alongside the episode so the matcher can tell a
// season-2 file from the base-season entry it would otherwise title-match.
#[test]
fn parse_keeps_the_season_from_an_s02e05_marker() {
    let result = parse_filename("The Apothecary Diaries - S02E05 - The Moon Fairy.mkv", None)
        .expect("should parse a season marker");
    assert_eq!(result.episode_number, 5);
    assert_eq!(result.season_number, Some(2));
}

#[test]
fn parse_keeps_the_season_from_a_cross_format_marker() {
    let result =
        parse_filename("Bungou Stray Dogs 3x11.mkv", None).expect("should parse a cross format");
    assert_eq!(result.episode_number, 11);
    assert_eq!(result.season_number, Some(3));
}

#[test]
fn parse_reports_no_season_without_a_marker() {
    let result = parse_filename("Cowboy Bebop - 05.mkv", None).expect("should parse");
    assert_eq!(result.season_number, None);
}
