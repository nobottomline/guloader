use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Manga {
    pub id: String,
    pub title: String,
    pub site: String,
    pub url: String,
    pub description: Option<String>,
    pub cover_url: Option<String>,
    pub status: MangaStatus,
    pub chapter_count: i32,
    pub last_updated: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Chapter {
    pub id: String,
    pub manga_id: String,
    pub manga_title: String,
    pub title: String,
    pub number: f64,
    pub url: String,
    pub page_count: i32,
    pub file_size_bytes: Option<i64>,
    pub status: ChapterStatus,
    pub downloaded_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ChapterPage {
    pub id: String,
    pub chapter_id: String,
    pub page_number: i32,
    pub image_url: String,
    pub local_path: Option<String>,
    pub file_size_bytes: Option<i64>,
    pub downloaded_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ScanLog {
    pub id: String,
    pub manga_id: String,
    pub site: String,
    pub status: ScanStatus,
    pub chapters_found: i32,
    pub chapters_new: i32,
    pub error_message: Option<String>,
    pub duration_ms: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "manga_status", rename_all = "lowercase")]
pub enum MangaStatus {
    Active,
    Paused,
    Completed,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "chapter_status", rename_all = "lowercase")]
pub enum ChapterStatus {
    Pending,
    Downloading,
    Downloaded,
    Failed,
    Deleted,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "scan_status", rename_all = "lowercase")]
pub enum ScanStatus {
    Success,
    Partial,
    Failed,
}

impl Manga {
}

impl Chapter {
    pub fn new(manga_id: String, manga_title: String, title: String, number: f64, url: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            manga_id,
            manga_title,
            title,
            number,
            url,
            page_count: 0,
            file_size_bytes: None,
            status: ChapterStatus::Pending,
            downloaded_at: None,
            created_at: now,
            updated_at: now,
        }
    }
}

impl ChapterPage {
    pub fn new(chapter_id: String, page_number: i32, image_url: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            chapter_id,
            page_number,
            image_url,
            local_path: None,
            file_size_bytes: None,
            downloaded_at: None,
            created_at: Utc::now(),
        }
    }
}

impl ScanLog {
    pub fn new(manga_id: String, site: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            manga_id,
            site,
            status: ScanStatus::Success,
            chapters_found: 0,
            chapters_new: 0,
            error_message: None,
            duration_ms: 0,
            created_at: Utc::now(),
        }
    }
}

impl std::fmt::Display for MangaStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MangaStatus::Active => write!(f, "Active"),
            MangaStatus::Paused => write!(f, "Paused"),
            MangaStatus::Completed => write!(f, "Completed"),
            MangaStatus::Error => write!(f, "Error"),
        }
    }
}
