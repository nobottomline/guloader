use crate::models::*;
use crate::error::Result;
use sqlx::SqlitePool;
use tracing::info;

pub struct Database {
    pub pool: SqlitePool,
}

impl Database {
    pub async fn new(database_url: &str) -> Result<Self> {
        info!("Connecting to database: {}", database_url);
        
        // Extract directory path from database URL
        if let Some(path) = database_url.strip_prefix("sqlite:") {
            let path = std::path::Path::new(path);
            if let Some(parent) = path.parent() {
                if !parent.as_os_str().is_empty() {
                    info!("Creating directory: {:?}", parent);
                    std::fs::create_dir_all(parent)?;
                }
            }
        }
        
        let pool = SqlitePool::connect(database_url).await?;
        info!("Database connected successfully");
        Ok(Self { pool })
    }
    
    pub async fn init(&self) -> Result<()> {
        info!("Initializing database schema...");
        
        // Create manga table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS manga (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                site TEXT NOT NULL,
                url TEXT NOT NULL,
                description TEXT,
                cover_url TEXT,
                status TEXT NOT NULL DEFAULT 'active',
                chapter_count INTEGER NOT NULL DEFAULT 0,
                last_updated DATETIME NOT NULL,
                created_at DATETIME NOT NULL,
                updated_at DATETIME NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;
        
        // Create chapters table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS chapters (
                id TEXT PRIMARY KEY,
                manga_id TEXT NOT NULL,
                manga_title TEXT NOT NULL,
                title TEXT NOT NULL,
                number REAL NOT NULL,
                url TEXT NOT NULL,
                page_count INTEGER NOT NULL DEFAULT 0,
                file_size_bytes INTEGER,
                status TEXT NOT NULL DEFAULT 'pending',
                downloaded_at DATETIME,
                created_at DATETIME NOT NULL,
                updated_at DATETIME NOT NULL,
                FOREIGN KEY (manga_id) REFERENCES manga (id),
                UNIQUE(manga_id, number)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;
        
        // Create chapter_pages table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS chapter_pages (
                id TEXT PRIMARY KEY,
                chapter_id TEXT NOT NULL,
                page_number INTEGER NOT NULL,
                image_url TEXT NOT NULL,
                local_path TEXT,
                file_size_bytes INTEGER,
                downloaded_at DATETIME,
                created_at DATETIME NOT NULL,
                FOREIGN KEY (chapter_id) REFERENCES chapters (id),
                UNIQUE(chapter_id, page_number)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;
        
        // Create scan_logs table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS scan_logs (
                id TEXT PRIMARY KEY,
                manga_id TEXT NOT NULL,
                site TEXT NOT NULL,
                status TEXT NOT NULL,
                chapters_found INTEGER NOT NULL DEFAULT 0,
                chapters_new INTEGER NOT NULL DEFAULT 0,
                error_message TEXT,
                duration_ms INTEGER NOT NULL DEFAULT 0,
                created_at DATETIME NOT NULL,
                FOREIGN KEY (manga_id) REFERENCES manga (id)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;
        
        // Create indexes
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_manga_site ON manga (site)")
            .execute(&self.pool)
            .await?;
        
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_chapters_manga_id ON chapters (manga_id)")
            .execute(&self.pool)
            .await?;
        
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_chapters_number ON chapters (manga_id, number)")
            .execute(&self.pool)
            .await?;
        
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_chapter_pages_chapter_id ON chapter_pages (chapter_id)")
            .execute(&self.pool)
            .await?;
        
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_scan_logs_manga_id ON scan_logs (manga_id)")
            .execute(&self.pool)
            .await?;
        
        info!("Database schema initialized successfully");
        Ok(())
    }
    
    // Manga operations
    pub async fn create_manga(&self, manga: &Manga) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO manga (id, title, site, url, description, cover_url, status, chapter_count, last_updated, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&manga.id)
        .bind(&manga.title)
        .bind(&manga.site)
        .bind(&manga.url)
        .bind(&manga.description)
        .bind(&manga.cover_url)
        .bind(&manga.status)
        .bind(manga.chapter_count)
        .bind(manga.last_updated)
        .bind(manga.created_at)
        .bind(manga.updated_at)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    pub async fn get_manga_by_id(&self, id: &str) -> Result<Option<Manga>> {
        let manga = sqlx::query_as::<_, Manga>(
            "SELECT * FROM manga WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        
        Ok(manga)
    }
    
    pub async fn get_manga_by_url(&self, url: &str) -> Result<Option<Manga>> {
        let manga = sqlx::query_as::<_, Manga>(
            "SELECT * FROM manga WHERE url = ?"
        )
        .bind(url)
        .fetch_optional(&self.pool)
        .await?;
        
        Ok(manga)
    }
    
    pub async fn get_all_manga(&self) -> Result<Vec<Manga>> {
        let manga_list = sqlx::query_as::<_, Manga>(
            "SELECT * FROM manga ORDER BY last_updated DESC"
        )
        .fetch_all(&self.pool)
        .await?;
        
        Ok(manga_list)
    }
    
    pub async fn update_manga(&self, manga: &Manga) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE manga 
            SET title = ?, site = ?, url = ?, description = ?, cover_url = ?, status = ?, chapter_count = ?, last_updated = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(&manga.title)
        .bind(&manga.site)
        .bind(&manga.url)
        .bind(&manga.description)
        .bind(&manga.cover_url)
        .bind(&manga.status)
        .bind(manga.chapter_count)
        .bind(manga.last_updated)
        .bind(manga.updated_at)
        .bind(&manga.id)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    // Chapter operations
    pub async fn create_or_get_chapter(&self, chapter: &Chapter) -> Result<Chapter> {
        let result = sqlx::query(
            r#"
            INSERT OR IGNORE INTO chapters (id, manga_id, manga_title, title, number, url, page_count, file_size_bytes, status, downloaded_at, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&chapter.id)
        .bind(&chapter.manga_id)
        .bind(&chapter.manga_title)
        .bind(&chapter.title)
        .bind(chapter.number)
        .bind(&chapter.url)
        .bind(chapter.page_count)
        .bind(chapter.file_size_bytes)
        .bind(&chapter.status)
        .bind(chapter.downloaded_at)
        .bind(chapter.created_at)
        .bind(chapter.updated_at)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            // Уже существует — вернем существующую запись
            if let Some(existing) = self.get_chapter_by_manga_and_number(&chapter.manga_id, chapter.number).await? {
                return Ok(existing);
            }
        }
        Ok(chapter.clone())
    }
    
    pub async fn get_chapters_by_manga_id(&self, manga_id: &str) -> Result<Vec<Chapter>> {
        let chapters = sqlx::query_as::<_, Chapter>(
            "SELECT * FROM chapters WHERE manga_id = ? ORDER BY number DESC"
        )
        .bind(manga_id)
        .fetch_all(&self.pool)
        .await?;
        
        Ok(chapters)
    }
    
    pub async fn get_chapter_by_manga_and_number(&self, manga_id: &str, number: f64) -> Result<Option<Chapter>> {
        let chapter = sqlx::query_as::<_, Chapter>(
                "SELECT * FROM chapters WHERE manga_id = ? AND ABS(number - ?) < 0.001 LIMIT 1"
            )
            .bind(manga_id)
            .bind(number)
            .fetch_optional(&self.pool)
            .await?;
        Ok(chapter)
    }
    
    pub async fn update_chapter(&self, chapter: &Chapter) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE chapters 
            SET title = ?, number = ?, url = ?, page_count = ?, file_size_bytes = ?, status = ?, downloaded_at = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(&chapter.title)
        .bind(chapter.number)
        .bind(&chapter.url)
        .bind(chapter.page_count)
        .bind(chapter.file_size_bytes)
        .bind(&chapter.status)
        .bind(chapter.downloaded_at)
        .bind(chapter.updated_at)
        .bind(&chapter.id)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    pub async fn get_old_chapters(&self, cutoff_date: chrono::DateTime<chrono::Utc>) -> Result<Vec<Chapter>> {
        let chapters = sqlx::query_as::<_, Chapter>(
            "SELECT * FROM chapters WHERE downloaded_at < ? AND status = 'downloaded'"
        )
        .bind(cutoff_date)
        .fetch_all(&self.pool)
        .await?;
        
        Ok(chapters)
    }
    
    pub async fn mark_chapter_deleted(&self, chapter_id: &str) -> Result<()> {
        sqlx::query(
            "UPDATE chapters SET status = 'deleted' WHERE id = ?"
        )
        .bind(chapter_id)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    // Chapter page operations
    #[allow(dead_code)]
    pub async fn create_chapter_page(&self, page: &ChapterPage) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO chapter_pages (id, chapter_id, page_number, image_url, local_path, file_size_bytes, downloaded_at, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&page.id)
        .bind(&page.chapter_id)
        .bind(page.page_number)
        .bind(&page.image_url)
        .bind(&page.local_path)
        .bind(page.file_size_bytes)
        .bind(page.downloaded_at)
        .bind(page.created_at)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    #[allow(dead_code)]
    pub async fn get_chapter_pages(&self, chapter_id: &str) -> Result<Vec<ChapterPage>> {
        let pages = sqlx::query_as::<_, ChapterPage>(
            "SELECT * FROM chapter_pages WHERE chapter_id = ? ORDER BY page_number"
        )
        .bind(chapter_id)
        .fetch_all(&self.pool)
        .await?;
        
        Ok(pages)
    }
    
    #[allow(dead_code)]
    pub async fn delete_chapter_pages(&self, chapter_id: &str) -> Result<()> {
        sqlx::query(
            "DELETE FROM chapter_pages WHERE chapter_id = ?"
        )
        .bind(chapter_id)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    // Scan log operations
    pub async fn create_scan_log(&self, scan_log: &ScanLog) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO scan_logs (id, manga_id, site, status, chapters_found, chapters_new, error_message, duration_ms, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&scan_log.id)
        .bind(&scan_log.manga_id)
        .bind(&scan_log.site)
        .bind(&scan_log.status)
        .bind(scan_log.chapters_found)
        .bind(scan_log.chapters_new)
        .bind(&scan_log.error_message)
        .bind(scan_log.duration_ms)
        .bind(scan_log.created_at)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
}
