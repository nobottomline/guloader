use crate::traits::ChapterDownloader;
use crate::models::{Chapter, ChapterPage};
use crate::config::SiteConfig;
use crate::error::{Result, GuLoaderError};
use crate::utils::HttpClient;
use crate::storage::StorageManager;
use scraper::{Html, Selector};
use tracing::{info, debug, error};
use futures::future::join_all;

pub struct MadaraDownloader {
    http_client: HttpClient,
}

impl MadaraDownloader {
    pub fn new() -> Self {
        Self {
            http_client: HttpClient::new(),
        }
    }
}

#[async_trait::async_trait]
impl ChapterDownloader for MadaraDownloader {
    async fn download_chapter(&self, config: &SiteConfig, chapter: &Chapter) -> Result<Vec<ChapterPage>> {
        info!("[MADARA/THUNDER DOWNLOADER] Downloading chapter: {} from Madara/Thunder", chapter.title);
        info!("[MADARA/THUNDER DOWNLOADER] Getting chapter images from: {}", chapter.url);
        
        let html = self.http_client.get(&chapter.url, config).await?;
        debug!("[MADARA/THUNDER DOWNLOADER] HTML length: {}", html.len());
        
        let pages = self.parse_images(&html, chapter)?;
        
        if pages.is_empty() {
            return Err(GuLoaderError::Scraping("No images found in chapter".to_string()));
        }
        
        info!("[MADARA/THUNDER DOWNLOADER] Found {} pages for chapter: {}", pages.len(), chapter.title);
        
        // Создаем StorageManager для загрузки файлов
        let storage_config = crate::config::StorageConfig {
            base_path: "./downloads".to_string(),
            scans_path: "./scans".to_string(),
            max_size_gb: 50,
            thumbnail_size: 200,
            compression: false,
        };
        let storage = StorageManager::new(&storage_config);
        
        // Загружаем изображения параллельно
        let download_tasks: Vec<_> = pages.iter().enumerate().map(|(index, page)| {
            let page_clone = page.clone();
            let chapter_clone = chapter.clone();
            let storage_config_clone = storage_config.clone();
            let http_client_clone = self.http_client.clone();
            
            tokio::spawn(async move {
                let storage = StorageManager::new(&storage_config_clone);
                let page_path = storage.get_chapter_page_path(&chapter_clone.manga_title, &chapter_clone, index + 1).await?;
                
                info!("[MADARA/THUNDER DOWNLOADER] Downloading image: {} to {:?}", 
                      page_clone.image_url, page_path);
                
                let response = http_client_clone.get_raw(&page_clone.image_url).await?;
                let image_data = response.bytes().await?;
                
                tokio::fs::write(&page_path, image_data).await?;
                
                info!("[MADARA/THUNDER DOWNLOADER] Successfully downloaded image to: {:?}", page_path);
                Ok::<(), crate::error::GuLoaderError>(())
            })
        }).collect();
        
        // Ждем завершения всех загрузок
        let results = join_all(download_tasks).await;
        
        // Проверяем результаты
        for result in results {
            if let Err(e) = result {
                error!("[MADARA/THUNDER DOWNLOADER] Failed to download image: {}", e);
                return Err(GuLoaderError::Io(std::io::Error::other(
                    format!("Failed to download image: {}", e)
                )));
            }
        }
        
        info!("[MADARA/THUNDER DOWNLOADER] Downloaded {} pages for chapter: {}", pages.len(), chapter.title);
        
        // Создаем ZIP архив
        info!("[MADARA/THUNDER DOWNLOADER] Creating ZIP archive...");
        let zip_path = storage.create_chapter_zip(&chapter.manga_title, chapter).await?;
        info!("[MADARA/THUNDER DOWNLOADER] Created ZIP archive: {:?}", zip_path);
        
        Ok(pages)
    }
    
    async fn get_chapter_images(&self, config: &SiteConfig, chapter_url: &str, chapter: &Chapter) -> Result<Vec<ChapterPage>> {
        info!("[MADARA/THUNDER DOWNLOADER] Getting chapter images from: {}", chapter_url);
        
        let html = self.http_client.get(chapter_url, config).await?;
        self.parse_images(&html, chapter)
    }
    
    async fn download_image(&self, image_url: &str, local_path: &str) -> Result<()> {
        info!("[MADARA/THUNDER DOWNLOADER] Downloading image: {} to {}", image_url, local_path);
        
        let response = self.http_client.get_raw(image_url).await?;
        let image_data = response.bytes().await?;
        
        tokio::fs::write(local_path, image_data).await?;
        
        info!("[MADARA/THUNDER DOWNLOADER] Successfully downloaded image to: {}", local_path);
        Ok(())
    }
}

impl MadaraDownloader {
    fn parse_images(&self, html: &str, chapter: &Chapter) -> Result<Vec<ChapterPage>> {
        debug!("[MADARA/THUNDER DOWNLOADER] Parsing images from HTML");
        
        let document = Html::parse_document(html);
        
        // Ищем изображения в различных возможных селекторах
        let mut image_urls = Vec::new();
        
        // Попробуем разные селекторы для изображений
        let selectors = [
            ".reading-content img",
            ".entry-content img", 
            ".chapter-content img",
            ".wp-manga-chapter-img",
            ".page-break img",
            "img[data-src]",
            "img[src*='wp-content']"
        ];
        
        for selector_str in &selectors {
            let selector = Selector::parse(selector_str).unwrap();
            for element in document.select(&selector) {
                // Сначала пробуем data-src (для lazy loading)
                let mut image_url = element.value().attr("data-src").unwrap_or("");
                
                // Если data-src пустой, пробуем src
                if image_url.is_empty() {
                    image_url = element.value().attr("src").unwrap_or("");
                }
                
                if !image_url.is_empty() && self.is_valid_image_url(image_url) {
                    image_urls.push(image_url.to_string());
                }
            }
        }
        
        // Удаляем дубликаты и сортируем
        image_urls.sort();
        image_urls.dedup();
        
        debug!("[MADARA/THUNDER DOWNLOADER] Found {} unique image URLs", image_urls.len());
        
        if image_urls.is_empty() {
            return Err(GuLoaderError::Scraping("No valid image URLs found".to_string()));
        }
        
        // Создаем объекты ChapterPage
        let pages: Vec<ChapterPage> = image_urls.into_iter().enumerate().map(|(index, url)| {
            ChapterPage {
                id: uuid::Uuid::new_v4().to_string(),
                chapter_id: chapter.id.clone(), // Используем ID главы
                page_number: (index + 1) as i32,
                image_url: url,
                local_path: None,
                file_size_bytes: None,
                downloaded_at: None,
                created_at: chrono::Utc::now(),
            }
        }).collect();
        
        info!("[MADARA/THUNDER DOWNLOADER] Successfully parsed {} pages", pages.len());
        Ok(pages)
    }
    
    fn is_valid_image_url(&self, url: &str) -> bool {
        // Проверяем, что это валидный URL изображения
        url.starts_with("http") && 
        (url.contains(".jpg") || url.contains(".jpeg") || url.contains(".png") || url.contains(".webp")) &&
        !url.contains("avatar") &&
        !url.contains("logo") &&
        !url.contains("icon")
    }
}