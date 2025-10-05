use crate::traits::MangaScanner;
use crate::models::{Manga, Chapter};
use crate::config::SiteConfig;
use crate::error::{Result, GuLoaderError};
use crate::utils::HttpClient;
use scraper::{Html, Selector};
use regex::Regex;
use tracing::info;
use chrono::{DateTime, Utc};

pub struct ErosScanner {
    http_client: HttpClient,
}

impl ErosScanner {
    pub fn new() -> Self {
        Self {
            http_client: HttpClient::new(),
        }
    }
    
    fn extract_chapter_number(&self, title: &str) -> f64 {
        let re = Regex::new(r"Chapter\s*([0-9]+(?:\.[0-9]+)?)").unwrap();
        if let Some(captures) = re.captures(title) {
            if let Some(number_str) = captures.get(1) {
                return number_str.as_str().parse().unwrap_or(1.0);
            }
        }
        1.0
    }
    
    fn parse_date(&self, date_str: &str) -> DateTime<Utc> {
        // Try to parse various date formats
        let formats = [
            "%B %d, %Y",     // June 25, 2025
            "%b %d, %Y",     // Jun 25, 2025
            "%Y-%m-%d",      // 2025-06-25
            "%d/%m/%Y",      // 25/06/2025
        ];
        
        for format in &formats {
            if let Ok(date) = chrono::NaiveDate::parse_from_str(date_str.trim(), format) {
                return date.and_hms_opt(0, 0, 0).unwrap().and_utc();
            }
        }
        
        // If parsing fails, return current time
        Utc::now()
    }
}

#[async_trait::async_trait]
impl MangaScanner for ErosScanner {
    async fn scan_manga(&self, config: &SiteConfig, manga: &Manga) -> Result<Vec<Chapter>> {
        info!("[EROS PARSER] Scanning manga: {} from {}", manga.title, config.name);
        
        let response = self.http_client.get(&manga.url, config).await?;
        let html = Html::parse_document(&response);
        
        info!("[EROS PARSER] HTML preview: {}...", &response[..response.len().min(500)]);
        
        let chapter_selector = Selector::parse("#chapterlist li")
            .map_err(|e| GuLoaderError::scraping(format!("Invalid chapter selector: {}", e)))?;
        
        let mut chapters = Vec::new();
        
        for (index, element) in html.select(&chapter_selector).enumerate() {
            let chapter_link_selector = Selector::parse(".eph-num a").unwrap();
            if let Some(chapter_link) = element.select(&chapter_link_selector).next() {
                let chapter_url = chapter_link.value().attr("href")
                    .ok_or_else(|| GuLoaderError::scraping("Chapter URL not found"))?;
                
                let full_chapter_url = if chapter_url.starts_with("http") {
                    chapter_url.to_string()
                } else {
                    format!("{}{}", config.base_url, chapter_url)
                };
                
                let chapter_title_selector = Selector::parse(".chapternum").unwrap();
                let chapter_title = chapter_link.select(&chapter_title_selector)
                    .next()
                    .map(|e| e.text().collect::<String>().trim().to_string())
                    .unwrap_or_else(|| format!("Chapter {}", index + 1));
                
                // Extract chapter number
                let chapter_number = self.extract_chapter_number(&chapter_title);
                
                // Get upload date
                let date_selector = Selector::parse(".chapterdate").unwrap();
                let _upload_date = chapter_link.select(&date_selector)
                    .next()
                    .map(|e| {
                        let date_text = e.text().collect::<String>().trim().to_string();
                        self.parse_date(&date_text)
                    })
                    .unwrap_or_else(|| Utc::now());
                
                info!("[EROS PARSER] Found chapter: {} (number: {}) at {}", 
                      chapter_title, chapter_number, full_chapter_url);
                
                let chapter = Chapter::new(
                    manga.id.clone(),
                    manga.title.clone(),
                    chapter_title,
                    chapter_number,
                    full_chapter_url,
                );
                
                chapters.push(chapter);
            }
        }
        
        info!("[EROS PARSER] Found {} chapters for manga: {}", chapters.len(), manga.title);
        Ok(chapters)
    }
}