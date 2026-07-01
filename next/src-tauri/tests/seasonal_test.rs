use anivault_core::engine::anilist::parse_seasonal_response;

#[test]
fn parses_seasonal_anime() {
    let json = r#"{
        "data": {
            "Page": {
                "media": [
                    {
                        "id": 1,
                        "title": { "romaji": "Spring Show", "english": null },
                        "coverImage": { "large": "https://example.com/spring.jpg" },
                        "episodes": 12,
                        "status": "RELEASING",
                        "season": "SPRING",
                        "seasonYear": 2026,
                        "format": "TV"
                    },
                    {
                        "id": 2,
                        "title": { "romaji": "Summer Movie", "english": "Summer Film" },
                        "coverImage": { "large": null },
                        "episodes": 1,
                        "status": "FINISHED",
                        "season": "SUMMER",
                        "seasonYear": 2025,
                        "format": "MOVIE"
                    }
                ]
            }
        }
    }"#;

    let results = parse_seasonal_response(json).unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].title, "Spring Show");
    assert_eq!(results[0].status, "RELEASING");
    assert_eq!(results[0].episodes, Some(12));
    assert_eq!(results[1].title, "Summer Movie");
    assert_eq!(results[1].english_title.as_deref(), Some("Summer Film"));
    assert_eq!(results[1].format, "MOVIE");
}

#[test]
fn parses_empty_seasonal() {
    let results = parse_seasonal_response(r#"{"data":{"Page":{"media":[]}}}"#).unwrap();
    assert!(results.is_empty());
}
