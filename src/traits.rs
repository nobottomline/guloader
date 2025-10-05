use crate::models::{Manga, Chapter, ChapterPage};
use crate::config::SiteConfig;
use crate::error::Result;

/// Trait for manga scanners that discover new chapters
#[async_trait::async_trait]
pub trait MangaScanner: Send + Sync {
    /// Scan a specific manga for new chapters
    async fn scan_manga(&self, config: &SiteConfig, manga: &Manga) -> Result<Vec<Chapter>>;
}

/// Trait for chapter downloaders that download chapter images
#[async_trait::async_trait]
pub trait ChapterDownloader: Send + Sync {
    /// Download a specific chapter
    async fn download_chapter(&self, config: &SiteConfig, chapter: &Chapter) -> Result<Vec<ChapterPage>>;
    
    /// Get all image URLs for a chapter
    async fn get_chapter_images(&self, config: &SiteConfig, chapter_url: &str, chapter: &Chapter) -> Result<Vec<ChapterPage>>;
    
    /// Download a single image
    async fn download_image(&self, image_url: &str, local_path: &str) -> Result<()>;
}
