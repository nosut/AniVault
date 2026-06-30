use crate::engine::storage::Storage;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TaigaSnapshot {
    pub anime: Vec<TaigaAnime>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TaigaAnime {
    pub id: i64,
    pub title: String,
    pub watched_episodes: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MigrationWarning {
    pub source: String,
    pub source_id: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub struct MigrationReport {
    pub imported_anime: usize,
    pub skipped_records: usize,
    pub warnings: Vec<MigrationWarning>,
}

pub async fn import_taiga_snapshot(storage: &Storage, snapshot: TaigaSnapshot) -> anyhow::Result<MigrationReport> {
    let mut report = MigrationReport::default();
    let mut valid_anime = Vec::new();

    for anime in snapshot.anime {
        if anime.id <= 0 || anime.title.trim().is_empty() {
            report.skipped_records += 1;
            report.warnings.push(MigrationWarning {
                source: "taiga_anime".to_string(),
                source_id: anime.id.to_string(),
                message: "Skipped anime with invalid id or blank title".to_string(),
            });
            continue;
        }

        valid_anime.push(anime);
        report.imported_anime += 1;
    }

    let mut transaction = storage.pool().begin().await?;
    for anime in valid_anime {
        let titles_json = serde_json::json!({
            "romaji": anime.title,
            "english": null,
            "japanese": null,
            "synonyms": []
        })
        .to_string();

        sqlx::query(
            "INSERT OR REPLACE INTO anime (id, titles_json, last_modified) VALUES (?1, ?2, 0)",
        )
        .bind(anime.id)
        .bind(titles_json)
        .execute(&mut *transaction)
        .await?;

        if anime.watched_episodes > 0 {
            sqlx::query(
                "INSERT INTO watch_history (anime_id, episode, file_path, player, watched_at, source)
                 SELECT ?1, ?2, NULL, NULL, 0, 'taiga_v2'
                 WHERE NOT EXISTS (
                   SELECT 1 FROM watch_history WHERE anime_id = ?1 AND episode = ?2 AND source = 'taiga_v2'
                 )",
            )
            .bind(anime.id)
            .bind(anime.watched_episodes)
            .execute(&mut *transaction)
            .await?;
        }
    }
    transaction.commit().await?;

    Ok(report)
}
