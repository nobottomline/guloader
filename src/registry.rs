use crate::traits::{MangaScanner, ChapterDownloader, CatalogChecker};
use crate::models::Chapter;
use crate::config::Config;
use crate::database::Database;
use crate::error::Result;
use tracing::info;
use crate::scanners::eros::ErosScanner;
use crate::scanners::madara::MadaraScanner;
use crate::downloaders::eros::ErosDownloader;
use crate::downloaders::madara::MadaraDownloader;
use crate::checkers::eros::ErosCatalogChecker;

/// Registry for managing scanners
pub struct ScannerRegistry {
    scanners: std::collections::HashMap<String, Box<dyn MangaScanner>>,
}

impl ScannerRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            scanners: std::collections::HashMap::new(),
        };
        
        // Register built-in scanners
        registry.register_scanner("eros", Box::new(ErosScanner::new()));
        registry.register_scanner("madara", Box::new(MadaraScanner::new()));
        registry.register_scanner("thunder", Box::new(MadaraScanner::new())); // Thunderscans использует тот же парсер
        
        registry
    }
    
    pub fn register_scanner(&mut self, name: &str, scanner: Box<dyn MangaScanner>) {
        self.scanners.insert(name.to_string(), scanner);
    }
    
    pub fn get_scanner(&self, name: &str) -> Option<&dyn MangaScanner> {
        self.scanners.get(name).map(|s| s.as_ref())
    }
    
    pub async fn scan_manga(&self, config: &Config, db: &Database, manga_id: &str) -> Result<()> {
        let manga = db.get_manga_by_id(manga_id)
            .await?
            .ok_or_else(|| crate::error::GuLoaderError::manga_not_found(manga_id))?;
        
        let site_config = config.get_site_config(&manga.site)
            .ok_or_else(|| crate::error::GuLoaderError::site_not_supported(&manga.site))?;
        
        let scanner = self.get_scanner(&site_config.scanner_type)
            .ok_or_else(|| crate::error::GuLoaderError::site_not_supported(&site_config.scanner_type))?;
        
        let start_time = std::time::Instant::now();
        let mut scan_log = crate::models::ScanLog::new(manga_id.to_string(), manga.site.clone());
        
        match scanner.scan_manga(site_config, &manga).await {
            Ok(chapters) => {
                let chapters_count = chapters.len();
                let mut new_chapters = 0;
                
                for chapter in chapters {
                    // Check if chapter already exists
                    let existing_chapters = db.get_chapters_by_manga_id(&manga.id).await?;
                    let chapter_exists = existing_chapters.iter()
                        .any(|c| (c.number - chapter.number).abs() < 0.01);
                    
                    if !chapter_exists {
                        db.create_chapter(&chapter).await?;
                        new_chapters += 1;
                    }
                }
                
                scan_log.chapters_found = chapters_count as i32;
                scan_log.chapters_new = new_chapters;
                scan_log.status = crate::models::ScanStatus::Success;
                
                // Update manga chapter count
                let total_chapters = db.get_chapters_by_manga_id(&manga.id).await?.len() as i32;
                let mut updated_manga = manga.clone();
                updated_manga.chapter_count = total_chapters;
                updated_manga.last_updated = chrono::Utc::now();
                updated_manga.updated_at = chrono::Utc::now();
                db.update_manga(&updated_manga).await?;
            }
            Err(e) => {
                scan_log.status = crate::models::ScanStatus::Failed;
                scan_log.error_message = Some(e.to_string());
            }
        }
        
        scan_log.duration_ms = start_time.elapsed().as_millis() as i64;
        db.create_scan_log(&scan_log).await?;
        
        Ok(())
    }
    
    pub async fn scan_all_manga(&self, config: &Config, db: &Database) -> Result<()> {
        let manga_list = db.get_all_manga().await?;
        
        for manga in manga_list {
            if manga.status == crate::models::MangaStatus::Active {
                if let Err(e) = self.scan_manga(config, db, &manga.id).await {
                    tracing::error!("Failed to scan manga {}: {}", manga.title, e);
                }
            }
        }
        
        Ok(())
    }
}

/// Registry for managing downloaders
pub struct DownloaderRegistry {
    downloaders: std::collections::HashMap<String, Box<dyn ChapterDownloader>>,
}

/// Registry for catalog checkers
pub struct CatalogRegistry {
    checkers: std::collections::HashMap<String, Box<dyn CatalogChecker>>, 
}

impl CatalogRegistry {
    pub fn new() -> Self {
        let mut registry = Self { checkers: std::collections::HashMap::new() };
        registry.register_checker("eros", Box::new(ErosCatalogChecker::new()));
        registry
    }

    pub fn register_checker(&mut self, name: &str, checker: Box<dyn CatalogChecker>) {
        self.checkers.insert(name.to_string(), checker);
    }

    pub fn get_checker(&self, name: &str) -> Option<&dyn CatalogChecker> {
        self.checkers.get(name).map(|c| c.as_ref())
    }
}

impl DownloaderRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            downloaders: std::collections::HashMap::new(),
        };
        
        // Register built-in downloaders
        registry.register_downloader("eros", Box::new(ErosDownloader::new()));
        registry.register_downloader("madara", Box::new(MadaraDownloader::new()));
        registry.register_downloader("thunder", Box::new(MadaraDownloader::new())); // Thunderscans использует тот же загрузчик
        
        registry
    }
    
    pub fn register_downloader(&mut self, name: &str, downloader: Box<dyn ChapterDownloader>) {
        self.downloaders.insert(name.to_string(), downloader);
    }
    
    pub fn get_downloader(&self, name: &str) -> Option<&dyn ChapterDownloader> {
        self.downloaders.get(name).map(|d| d.as_ref())
    }
    
    pub async fn download_chapter(&self, config: &Config, db: &Database, chapter_url: &str) -> Result<()> {
        // Find chapter by URL
        let chapters = sqlx::query_as::<_, Chapter>(
            "SELECT * FROM chapters WHERE url = ?"
        )
        .bind(chapter_url)
        .fetch_all(&db.pool)
        .await?;
        
        let chapter = chapters.first()
            .ok_or_else(|| crate::error::GuLoaderError::chapter_not_found(chapter_url))?;
        
        let manga = db.get_manga_by_id(&chapter.manga_id)
            .await?
            .ok_or_else(|| crate::error::GuLoaderError::manga_not_found(&chapter.manga_id))?;
        
        let site_config = config.get_site_config(&manga.site)
            .ok_or_else(|| crate::error::GuLoaderError::site_not_supported(&manga.site))?;
        
        let downloader = self.get_downloader(&site_config.downloader_type)
            .ok_or_else(|| crate::error::GuLoaderError::site_not_supported(&site_config.downloader_type))?;
        
        // Update chapter status to downloading
        let mut updated_chapter = chapter.clone();
        updated_chapter.status = crate::models::ChapterStatus::Downloading;
        updated_chapter.updated_at = chrono::Utc::now();
        db.update_chapter(&updated_chapter).await?;
        
        match downloader.download_chapter(site_config, chapter).await {
            Ok(pages) => {
                let pages_count = pages.len();
                // Save pages to database
                for page in pages {
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
                    .execute(&db.pool)
                    .await?;
                }
                
                // Update chapter status to downloaded
                updated_chapter.status = crate::models::ChapterStatus::Downloaded;
                updated_chapter.page_count = pages_count as i32;
                updated_chapter.downloaded_at = Some(chrono::Utc::now());
                updated_chapter.updated_at = chrono::Utc::now();
                db.update_chapter(&updated_chapter).await?;
            }
            Err(e) => {
                // Update chapter status to failed
                updated_chapter.status = crate::models::ChapterStatus::Failed;
                updated_chapter.updated_at = chrono::Utc::now();
                db.update_chapter(&updated_chapter).await?;
                
                return Err(e);
            }
        }
        
        Ok(())
    }
    
    pub async fn download_chapter_to_scans(&self, config: &Config, db: &Database, chapter: &Chapter) -> Result<()> {
        // Get site config
        let manga = db.get_manga_by_id(&chapter.manga_id).await?
            .ok_or_else(|| crate::error::GuLoaderError::manga_not_found(&chapter.manga_id))?;
        
        let site_config = config.get_site_config(&manga.site)
            .ok_or_else(|| crate::error::GuLoaderError::site_not_supported(&manga.site))?;
        
        // Get downloader
        let downloader = self.get_downloader(&site_config.downloader_type)
            .ok_or_else(|| crate::error::GuLoaderError::site_not_supported(&site_config.downloader_type))?;
        
        // Update chapter status to downloading
        let mut updated_chapter = chapter.clone();
        updated_chapter.status = crate::models::ChapterStatus::Downloading;
        updated_chapter.updated_at = chrono::Utc::now();
        db.update_chapter(&updated_chapter).await?;
        
        // Create storage manager for scans
        let storage = crate::storage::StorageManager::new(&config.storage);
        
        match downloader.download_chapter(site_config, chapter).await {
            Ok(pages) => {
                let pages_count = pages.len();
                
                // Download pages to scans directory
                let mut downloaded_pages = Vec::new();
                for (index, page) in pages.iter().enumerate() {
                    let local_path = storage.get_scans_chapter_page_path(&chapter.manga_title, chapter, index + 1).await?;
                    
                    // Download image
                    let response = reqwest::get(&page.image_url).await?;
                    let bytes = response.bytes().await?;
                    tokio::fs::write(&local_path, bytes).await?;
                    
                    let mut downloaded_page = page.clone();
                    downloaded_page.local_path = Some(local_path.to_string_lossy().to_string());
                    downloaded_page.downloaded_at = Some(chrono::Utc::now());
                    
                    // Get file size
                    if let Ok(metadata) = tokio::fs::metadata(&local_path).await {
                        downloaded_page.file_size_bytes = Some(metadata.len() as i64);
                    }
                    
                    downloaded_pages.push(downloaded_page);
                }
                
                // Create ZIP archive in scans directory
                storage.create_scans_chapter_zip(&chapter.manga_title, chapter).await?;
                
                // Save pages to database
                for page in downloaded_pages {
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
                    .execute(&db.pool)
                    .await?;
                }
                
                // Update chapter status to downloaded
                let mut final_chapter = chapter.clone();
                final_chapter.status = crate::models::ChapterStatus::Downloaded;
                final_chapter.page_count = pages_count as i32;
                final_chapter.downloaded_at = Some(chrono::Utc::now());
                final_chapter.updated_at = chrono::Utc::now();
                db.update_chapter(&final_chapter).await?;
                
                info!("Successfully downloaded {} pages for chapter: {} to scans directory", pages_count, chapter.title);
            }
            Err(e) => {
                // Update chapter status back to pending on error
                let mut error_chapter = chapter.clone();
                error_chapter.status = crate::models::ChapterStatus::Pending;
                error_chapter.updated_at = chrono::Utc::now();
                db.update_chapter(&error_chapter).await?;
                
                return Err(e);
            }
        }
        
        Ok(())
    }
}
