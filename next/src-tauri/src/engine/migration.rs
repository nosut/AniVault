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

        storage.insert_minimal_anime(anime.id, &anime.title).await?;
        if anime.watched_episodes > 0 {
            storage
                .append_watch_history(anime.id, anime.watched_episodes, None, None, 0)
                .await?;
        }
        report.imported_anime += 1;
    }

    Ok(report)
}
