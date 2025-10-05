use crate::config::StorageConfig;
use crate::models::Chapter;
use crate::error::Result;
use std::path::{Path, PathBuf};
use std::io::Write;
use tracing::info;
use zip::write::FileOptions;

pub struct StorageManager {
    config: StorageConfig,
}

impl StorageManager {
    pub fn new(config: &StorageConfig) -> Self {
        Self {
            config: config.clone(),
        }
    }
    
    pub async fn get_manga_path(&self, manga_title: &str) -> Result<PathBuf> {
        let sanitized_title = self.sanitize_filename(manga_title);
        let path = Path::new(&self.config.base_path).join(sanitized_title);
        
        // Create directory if it doesn't exist
        tokio::fs::create_dir_all(&path).await?;
        
        Ok(path)
    }
    
    // Methods for scans directory (automatic downloads)
    pub async fn get_scans_manga_path(&self, manga_title: &str) -> Result<PathBuf> {
        let sanitized_title = self.sanitize_filename(manga_title);
        let path = Path::new(&self.config.scans_path).join(sanitized_title);
        
        // Create directory if it doesn't exist
        tokio::fs::create_dir_all(&path).await?;
        
        Ok(path)
    }
    
    pub async fn get_scans_chapter_path(&self, manga_title: &str, chapter: &Chapter) -> Result<PathBuf> {
        let manga_path = self.get_scans_manga_path(manga_title).await?;
        let chapter_number = chapter.number as i32;
        let path = manga_path.join(format!("{}", chapter_number));
        
        // Create directory if it doesn't exist
        tokio::fs::create_dir_all(&path).await?;
        
        Ok(path)
    }
    
    pub async fn get_scans_chapter_pages_path(&self, manga_title: &str, chapter: &Chapter) -> Result<PathBuf> {
        let chapter_path = self.get_scans_chapter_path(manga_title, chapter).await?;
        let pages_path = chapter_path.join("pages");
        
        // Create directory if it doesn't exist
        tokio::fs::create_dir_all(&pages_path).await?;
        
        Ok(pages_path)
    }
    
    pub async fn get_scans_chapter_page_path(&self, manga_title: &str, chapter: &Chapter, page_number: usize) -> Result<PathBuf> {
        let pages_path = self.get_scans_chapter_pages_path(manga_title, chapter).await?;
        let filename = format!("page_{:03}.jpg", page_number);
        Ok(pages_path.join(filename))
    }
    
    pub async fn create_scans_chapter_zip(&self, manga_title: &str, chapter: &Chapter) -> Result<PathBuf> {
        let chapter_path = self.get_scans_chapter_path(manga_title, chapter).await?;
        let pages_path = self.get_scans_chapter_pages_path(manga_title, chapter).await?;
        let zip_path = chapter_path.join(format!("Chapter_{}.zip", chapter.number));
        
        info!("Creating scans ZIP archive: {:?}", zip_path);
        
        let file = std::fs::File::create(&zip_path)?;
        let mut zip = zip::ZipWriter::new(file);
        
        // Add all page files to ZIP
        let mut entries = tokio::fs::read_dir(&pages_path).await?;
        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_file() {
                let file_path = entry.path();
                let file_name = file_path.file_name().unwrap().to_string_lossy();
                
                zip.start_file(file_name, FileOptions::default())?;
                
                let file_content = tokio::fs::read(&file_path).await?;
                zip.write_all(&file_content)?;
            }
        }
        
        zip.finish()?;
        info!("Created scans ZIP archive: {:?}", zip_path);
        Ok(zip_path)
    }
    
    pub async fn get_chapter_path(&self, manga_title: &str, chapter: &Chapter) -> Result<PathBuf> {
        let manga_path = self.get_manga_path(manga_title).await?;
        let chapter_number = chapter.number as i32;
        let path = manga_path.join(format!("{}", chapter_number));
        
        // Create directory if it doesn't exist
        tokio::fs::create_dir_all(&path).await?;
        
        Ok(path)
    }
    
    pub async fn get_chapter_pages_path(&self, manga_title: &str, chapter: &Chapter) -> Result<PathBuf> {
        let chapter_path = self.get_chapter_path(manga_title, chapter).await?;
        let pages_path = chapter_path.join("pages");
        
        // Create directory if it doesn't exist
        tokio::fs::create_dir_all(&pages_path).await?;
        
        Ok(pages_path)
    }
    
    pub async fn get_chapter_page_path(&self, manga_title: &str, chapter: &Chapter, page_number: usize) -> Result<PathBuf> {
        let pages_path = self.get_chapter_pages_path(manga_title, chapter).await?;
        let filename = format!("page_{:03}.jpg", page_number);
        Ok(pages_path.join(filename))
    }
    
    pub async fn remove_chapter(&self, manga_title: &str, chapter: &Chapter) -> Result<()> {
        let chapter_path = self.get_chapter_path(manga_title, chapter).await?;
        
        if chapter_path.exists() {
            tokio::fs::remove_dir_all(&chapter_path).await?;
            info!("Removed chapter directory: {:?}", chapter_path);
        }
        
        Ok(())
    }
    
    fn sanitize_filename(&self, filename: &str) -> String {
        filename
            .chars()
            .map(|c| match c {
                '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
                c if c.is_control() => '_',
                c => c,
            })
            .collect::<String>()
            .trim_matches('.')
            .trim_matches(' ')
            .to_string()
    }
    
    pub async fn create_chapter_zip(&self, manga_title: &str, chapter: &Chapter) -> Result<PathBuf> {
        let chapter_path = self.get_chapter_path(manga_title, chapter).await?;
        let pages_path = self.get_chapter_pages_path(manga_title, chapter).await?;
        let zip_path = chapter_path.join(format!("Chapter_{}.zip", chapter.number));
        
        info!("Creating ZIP archive: {:?}", zip_path);
        
        let file = std::fs::File::create(&zip_path)?;
        let mut zip = zip::ZipWriter::new(file);
        
        // Add all page files to ZIP
        let mut entries = tokio::fs::read_dir(&pages_path).await?;
        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_file() {
                let file_path = entry.path();
                let file_name = file_path.file_name().unwrap().to_string_lossy();
                
                zip.start_file(file_name, FileOptions::default())?;
                
                let file_content = tokio::fs::read(&file_path).await?;
                zip.write_all(&file_content)?;
            }
        }
        
        zip.finish()?;
        info!("Created ZIP archive: {:?}", zip_path);
        Ok(zip_path)
    }
}
