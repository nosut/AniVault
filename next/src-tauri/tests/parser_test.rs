use anivault_core::engine::recognition::parser::parse_filename;

#[test]
fn parse_standard_subgroup_release() {
    let result = parse_filename(r"D:\Anime\[SubsPlease] Spy x Family - 17.mkv");
    assert_eq!(result.title, "Spy x Family");
    assert_eq!(result.episode, Some(17));
    assert_eq!(result.season, None);
    assert!(result.confidence > 0.9);
}

#[test]
fn parse_alt_separator_episode() {
    let result = parse_filename("Frieren - S01E14.mkv");
    assert_eq!(result.title, "Frieren");
    assert_eq!(result.season, Some(1));
    assert_eq!(result.episode, Some(14));
}

#[test]
fn parse_era_with_resolution_and_codec() {
    let result = parse_filename("[Erai-raws] Jujutsu Kaisen 2nd Season - 05 [1080p][HEVC].mkv");
    assert_eq!(result.title, "Jujutsu Kaisen 2nd Season");
    assert_eq!(result.episode, Some(5));
}

#[test]
fn parse_simple_title_episode() {
    let result = parse_filename("Cowboy Bebop - 03.mkv");
    assert_eq!(result.title, "Cowboy Bebop");
    assert_eq!(result.episode, Some(3));
}

#[test]
fn parse_episode_hash_format() {
    let result = parse_filename("Attack on Titan #25.mkv");
    assert_eq!(result.title, "Attack on Titan");
    assert_eq!(result.episode, Some(25));
}

#[test]
fn parse_no_episode_number() {
    let result = parse_filename("Mushishi.mkv");
    assert_eq!(result.title, "Mushishi");
    assert_eq!(result.episode, None);
}

#[test]
fn parse_dots_as_spaces() {
    let result = parse_filename("One.Piece.1088.mkv");
    assert_eq!(result.title, "One Piece");
    assert_eq!(result.episode, Some(1088));
}

#[test]
fn parse_underscores() {
    let result = parse_filename("Kusuriya_no_Hitorigoto_-_24.mkv");
    assert_eq!(result.title, "Kusuriya no Hitorigoto");
    assert_eq!(result.episode, Some(24));
}

#[test]
fn parse_multiple_brackets() {
    let result = parse_filename("[HorribleSubs] Kimetsu no Yaiba [1080p] - 11.mkv");
    assert_eq!(result.title, "Kimetsu no Yaiba");
    assert_eq!(result.episode, Some(11));
}

#[test]
fn parse_parentheses_episode() {
    let result = parse_filename("Bocchi the Rock! (06).mkv");
    assert_eq!(result.title, "Bocchi the Rock!");
    assert_eq!(result.episode, Some(6));
}

#[test]
fn parse_season_s_format() {
    let result = parse_filename("Oshi no Ko S2 - 03.mkv");
    assert_eq!(result.title, "Oshi no Ko");
    assert_eq!(result.season, Some(2));
    assert_eq!(result.episode, Some(3));
}

#[test]
fn parse_season_word_format() {
    let result = parse_filename("Mushoku Tensei Season 2 - 12.mkv");
    assert_eq!(result.title, "Mushoku Tensei");
    assert_eq!(result.season, Some(2));
    assert_eq!(result.episode, Some(12));
}

#[test]
fn parse_episode_prefix() {
    let result = parse_filename("Vinland Saga Ep 07.mkv");
    assert_eq!(result.title, "Vinland Saga");
    assert_eq!(result.episode, Some(7));
}

#[test]
fn parse_trailing_year_not_episode() {
    let result = parse_filename("Summer 2024.mkv");
    assert_eq!(result.title, "Summer 2024");
    assert_eq!(result.episode, None);
}

#[test]
fn parse_nested_brackets_and_slashes() {
    let result = parse_filename(r"D:\Anime\Fall 2024\[SubsPlease] Dandadan - 12 (1080p) [54B2E3C0].mkv");
    assert_eq!(result.title, "Dandadan");
    assert_eq!(result.episode, Some(12));
}
