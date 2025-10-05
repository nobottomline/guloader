use thiserror::Error;

#[derive(Error, Debug)]
pub enum GuLoaderError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    
    #[error("HTTP request error: {0}")]
    Http(#[from] reqwest::Error),
    
    #[error("Configuration error: {0}")]
    Config(#[from] config::ConfigError),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error("URL parsing error: {0}")]
    Url(#[from] url::ParseError),
    
    #[error("Image processing error: {0}")]
    Image(#[from] image::ImageError),
    
    #[error("ZIP compression error: {0}")]
    Zip(#[from] zip::result::ZipError),
    
    #[error("Scraping error: {0}")]
    Scraping(String),
    
    #[error("Chapter not found: {0}")]
    ChapterNotFound(String),
    
    #[error("Manga not found: {0}")]
    MangaNotFound(String),
    
    #[error("Site not supported: {0}")]
    SiteNotSupported(String),
}

impl GuLoaderError {
    pub fn scraping(msg: impl Into<String>) -> Self {
        Self::Scraping(msg.into())
    }
    
    pub fn chapter_not_found(chapter_id: impl Into<String>) -> Self {
        Self::ChapterNotFound(chapter_id.into())
    }
    
    pub fn manga_not_found(manga_id: impl Into<String>) -> Self {
        Self::MangaNotFound(manga_id.into())
    }
    
    pub fn site_not_supported(site: impl Into<String>) -> Self {
        Self::SiteNotSupported(site.into())
    }
}

pub type Result<T> = std::result::Result<T, GuLoaderError>;