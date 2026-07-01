use taiga_next::engine::parser::parse_filename;

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
