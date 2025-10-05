use crate::traits::ChapterDownloader;
use crate::models::{Chapter, ChapterPage};
use crate::config::SiteConfig;
use crate::error::{Result, GuLoaderError};
use crate::utils::HttpClient;
use crate::storage::StorageManager;
use std::path::Path;
use tracing::{info, warn};
use indicatif::{ProgressBar, ProgressStyle};
use regex::Regex;

#[derive(Clone)]
pub struct ErosDownloader {
    http_client: HttpClient,
}

impl ErosDownloader {
    pub fn new() -> Self {
        Self {
            http_client: HttpClient::new(),
        }
    }
}

#[async_trait::async_trait]
impl ChapterDownloader for ErosDownloader {
    async fn download_chapter(&self, config: &SiteConfig, chapter: &Chapter) -> Result<Vec<ChapterPage>> {
        info!("[EROS DOWNLOADER] Downloading chapter: {} from {}", chapter.title, config.name);
        
        let pages = self.get_chapter_images(config, &chapter.url, chapter).await?;
        
        let storage = StorageManager::new(&crate::config::StorageConfig {
            base_path: "./downloads".to_string(),
            scans_path: "./scans".to_string(),
            max_size_gb: 50,
            compression: true,
            thumbnail_size: 200,
        });
        
        let mut downloaded_pages = Vec::new();
        let pb = ProgressBar::new(pages.len() as u64);
        pb.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("#>-"));
        
        // Create download tasks for parallel execution
        let mut download_tasks = Vec::new();
        let _pages_clone = pages.clone(); // Clone pages for later use
        for (index, page) in pages.into_iter().enumerate() {
            let local_path = storage.get_chapter_page_path(&chapter.manga_title, chapter, index + 1).await?;
            let downloader = self.clone();
            let image_url = page.image_url.clone();
            let page_number = index + 1;
            
            let task = tokio::spawn(async move {
                let result = downloader.download_image(&image_url, &local_path.to_string_lossy()).await;
                (page_number, result, local_path, page)
            });
            
            download_tasks.push(task);
        }
        
        // Execute all downloads in parallel
        let mut completed_downloads = 0;
        for task in download_tasks {
            match task.await {
                Ok((page_number, result, local_path, mut page)) => {
                    pb.set_message(format!("Downloading page {}", page_number));
                    
                    match result {
                        Ok(()) => {
                            page.local_path = Some(local_path.to_string_lossy().to_string());
                            page.downloaded_at = Some(chrono::Utc::now());
                            
                            // Get file size
                            if let Ok(metadata) = tokio::fs::metadata(&local_path).await {
                                page.file_size_bytes = Some(metadata.len() as i64);
                            }
                            
                            downloaded_pages.push(page);
                        }
                        Err(e) => {
                            warn!("Failed to download image for page {}: {}", page_number, e);
                        }
                    }
                    
                    completed_downloads += 1;
                    pb.set_position(completed_downloads);
                }
                Err(e) => {
                    warn!("Task failed: {}", e);
                }
            }
        }
        
        pb.finish_with_message("Download completed");
        info!("[EROS DOWNLOADER] Downloaded {} pages for chapter: {}", downloaded_pages.len(), chapter.title);
        
        // Create ZIP and PDF files
        if !downloaded_pages.is_empty() {
            info!("[EROS DOWNLOADER] Creating ZIP archive...");
            
            // Create ZIP archive
            if let Err(e) = storage.create_chapter_zip(&chapter.manga_title, chapter).await {
                warn!("[EROS DOWNLOADER] Failed to create ZIP: {}", e);
            }
        }
        
        Ok(downloaded_pages)
    }
    
    async fn get_chapter_images(&self, config: &SiteConfig, chapter_url: &str, chapter: &Chapter) -> Result<Vec<ChapterPage>> {
        info!("[EROS DOWNLOADER] Getting chapter images from: {}", chapter_url);
        
        let response = self.http_client.get(chapter_url, config).await?;
        let html = &response;
        
        info!("[EROS DOWNLOADER] HTML length: {}", html.len());
        info!("[EROS DOWNLOADER] Contains ts_reader.run: {}", html.contains("ts_reader.run"));
        info!("[EROS DOWNLOADER] Contains images: {}", html.contains("\"images\""));
        info!("[EROS DOWNLOADER] Contains erosscans.xyz: {}", html.contains("erosscans.xyz"));
        
        let mut pages = Vec::new();
        
        // Ищем JavaScript блок с ts_reader.run
        let ts_reader_start = html.find("ts_reader.run(")
            .ok_or_else(|| GuLoaderError::scraping("Не удалось найти ts_reader.run в HTML"))?;
        
        let after_start = &html[ts_reader_start..];
        let ts_reader_end = after_start.find(");")
            .ok_or_else(|| GuLoaderError::scraping("Не удалось найти конец ts_reader.run"))?;
        
        let ts_reader_content = &after_start[..ts_reader_end];
        info!("[EROS DOWNLOADER] Found ts_reader.run content, length: {}", ts_reader_content.len());
        
        // Ищем массив images в JSON
        let images_start = ts_reader_content.find("\"images\":[")
            .ok_or_else(|| GuLoaderError::scraping("Не удалось найти массив images"))?;
        
        let after_images_start = &ts_reader_content[images_start..];
        
        // Ищем конец массива, учитывая возможные вложенные структуры
        let mut bracket_count = 0;
        let mut images_end = None;
        
        for (index, char) in after_images_start.char_indices() {
            match char {
                '[' => bracket_count += 1,
                ']' => {
                    bracket_count -= 1;
                    if bracket_count == 0 {
                        images_end = Some(index + 1);
                        break;
                    }
                }
                _ => {}
            }
        }
        
        let images_end = images_end
            .ok_or_else(|| GuLoaderError::scraping("Не удалось найти конец массива images"))?;
        
        let images_array = &after_images_start[..images_end];
        info!("[EROS DOWNLOADER] Found images array: {}...", &images_array[..images_array.len().min(200)]);
        info!("[EROS DOWNLOADER] Images array length: {}", images_array.len());
        
        // Извлекаем URL изображений из массива
        let mut image_urls = Vec::new();
        
        // Используем regex для поиска экранированных URL
        let pattern = r#"https:\\\\\\/[^\\"]+\.(webp|jpg|jpeg|png)"#;
        let regex = match Regex::new(pattern) {
            Ok(regex) => regex,
            Err(e) => {
                warn!("[EROS DOWNLOADER] Failed to create regex: {}", e);
                // Fallback to simple method
                return self.parse_images_fallback(images_array, chapter);
            }
        };
        
        for mat in regex.find_iter(images_array) {
            let url = mat.as_str().replace("\\", "");
            info!("[EROS DOWNLOADER] Regex URL: {}", url);
            image_urls.push(url);
        }
        
        // Если regex не сработал, используем fallback метод
        if image_urls.is_empty() {
            info!("[EROS DOWNLOADER] Regex didn't work, using fallback method");
            return self.parse_images_fallback(images_array, chapter);
        }
        
        info!("[EROS DOWNLOADER] Extracted {} image URLs", image_urls.len());
        
        // Выводим все URL для отладки
        for (index, url) in image_urls.iter().enumerate() {
            info!("[EROS DOWNLOADER] URL {}: {}", index + 1, url);
        }
        
        // Создаем объекты ChapterPage
        for (index, image_url) in image_urls.into_iter().enumerate() {
            let page = ChapterPage::new(
                chapter.id.clone(), // Используем ID главы
                (index + 1) as i32,
                image_url,
            );
            
            pages.push(page);
        }
        
        if pages.is_empty() {
            return Err(GuLoaderError::scraping("Не удалось найти страницы главы (ErosScans)"));
        }
        
        info!("[EROS DOWNLOADER] Successfully found {} pages", pages.len());
        if let Some(last_page) = pages.last() {
            info!("[EROS DOWNLOADER] Last page URL: {}", last_page.image_url);
        }
        
        Ok(pages)
    }
    
    async fn download_image(&self, image_url: &str, local_path: &str) -> Result<()> {
        info!("[EROS DOWNLOADER] Downloading image: {} to {}", image_url, local_path);
        
        // Create directory if it doesn't exist
        if let Some(parent) = Path::new(local_path).parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        
        let response = self.http_client.get_raw(image_url).await?;
        let bytes = response.bytes().await?;
        
        tokio::fs::write(local_path, bytes).await?;
        
        info!("[EROS DOWNLOADER] Successfully downloaded image to: {}", local_path);
        Ok(())
    }
}

impl ErosDownloader {
    fn parse_images_fallback(&self, images_array: &str, chapter: &Chapter) -> Result<Vec<ChapterPage>> {
        info!("[EROS DOWNLOADER] Using fallback parsing method");
        
        // Убираем префикс "images":[ из начала массива
        let clean_array = images_array.replace("\"images\":[", "");
        
        info!("[EROS DOWNLOADER] Clean array first 200 chars: {}...", &clean_array[..clean_array.len().min(200)]);
        
        let image_urls: Vec<String> = clean_array
            .split(',')
            .filter_map(|url_string| {
                let cleaned = url_string
                    .trim()
                    .replace("\"", "")
                    .replace("\\", "")
                    .replace("]", ""); // Убираем лишние символы
                
                // Проверяем, что это валидный URL изображения
                if cleaned.starts_with("https://") && 
                   (cleaned.contains(".webp") || cleaned.contains(".jpg") || 
                    cleaned.contains(".jpeg") || cleaned.contains(".png")) {
                    Some(cleaned)
                } else {
                    None
                }
            })
            .filter(|url| !url.is_empty())
            .collect();
        
        info!("[EROS DOWNLOADER] Fallback extracted {} image URLs", image_urls.len());
        
        // Выводим все URL для отладки
        for (index, url) in image_urls.iter().enumerate() {
            info!("[EROS DOWNLOADER] Fallback URL {}: {}", index + 1, url);
        }
        
        // Создаем объекты ChapterPage
        let mut pages = Vec::new();
        for (index, image_url) in image_urls.into_iter().enumerate() {
            let page = ChapterPage::new(
                chapter.id.clone(), // Используем ID главы
                (index + 1) as i32,
                image_url,
            );
            
            pages.push(page);
        }
        
        if pages.is_empty() {
            return Err(GuLoaderError::scraping("Не удалось найти страницы главы (ErosScans) - fallback method"));
        }
        
        info!("[EROS DOWNLOADER] Fallback successfully found {} pages", pages.len());
        if let Some(last_page) = pages.last() {
            info!("[EROS DOWNLOADER] Last page URL: {}", last_page.image_url);
        }
        
        Ok(pages)
    }
}
