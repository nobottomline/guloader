use crate::traits::MangaScanner;
use crate::models::{Manga, Chapter};
use crate::config::SiteConfig;
use crate::error::Result;
use crate::utils::HttpClient;
use scraper::{Html, Selector};
use regex::Regex;
use chrono::{DateTime, Utc};
use tracing::{info, debug};

pub struct MadaraScanner {
    http_client: HttpClient,
}

impl MadaraScanner {
    pub fn new() -> Self {
        Self {
            http_client: HttpClient::new(),
        }
    }
}

#[async_trait::async_trait]
impl MangaScanner for MadaraScanner {
    async fn scan_manga(&self, config: &SiteConfig, manga: &Manga) -> Result<Vec<Chapter>> {
        info!("[MADARA/THUNDER PARSER] Scanning manga from: {}", manga.url);
        
        let html = self.http_client.get(&manga.url, config).await?;
        debug!("[MADARA/THUNDER PARSER] HTML length: {}", html.len());
        
        let document = Html::parse_document(&html);
        
        // Название
        let title_selector = Selector::parse("h1.entry-title").unwrap();
        let title = document
            .select(&title_selector)
            .next()
            .map(|el| el.text().collect::<String>().trim().to_string())
            .unwrap_or_default();
        
        info!("[MADARA/THUNDER PARSER] title={}", title);
        
        // Обложка
        let cover_selector = Selector::parse(".thumb img").unwrap();
        let cover = document
            .select(&cover_selector)
            .next()
            .and_then(|el| el.value().attr("src"))
            .unwrap_or_default()
            .to_string();
        
        info!("[MADARA/THUNDER PARSER] cover={}", cover);
        
        // Альтернативные названия
        let alt_selector = Selector::parse(".alternative .desktop-titles").unwrap();
        let alternative_names: Vec<String> = document
            .select(&alt_selector)
            .next()
            .map(|el| {
                el.text()
                    .collect::<String>()
                    .trim()
                    .split('|')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        
        debug!("[MADARA/THUNDER PARSER] alternativeNames={:?}", alternative_names);
        
        // Жанры
        let genre_selector = Selector::parse(".mgen a").unwrap();
        let genres: Vec<String> = document
            .select(&genre_selector)
            .map(|el| el.text().collect::<String>().trim().to_string())
            .collect();
        
        debug!("[MADARA/THUNDER PARSER] genres={:?}", genres);
        
        // Рейтинг
        let rating_selector = Selector::parse(".numscore").unwrap();
        let rating = document
            .select(&rating_selector)
            .next()
            .and_then(|el| el.text().collect::<String>().parse::<f64>().ok())
            .unwrap_or(0.0);
        
        debug!("[MADARA/THUNDER PARSER] rating={}", rating);
        
        // Описание
        let desc_selector = Selector::parse(".entry-content.entry-content-single p").unwrap();
        let description = document
            .select(&desc_selector)
            .next()
            .map(|el| el.text().collect::<String>().trim().to_string())
            .unwrap_or_default();
        
        debug!("[MADARA/THUNDER PARSER] description={}...", &description[..description.len().min(100)]);
        
        // Автор и другие детали
        let mut author = String::new();
        let mut artist: Option<String> = None;
        let mut _serialization: Option<String> = None;
        let mut status = crate::models::MangaStatus::Active;
        let mut _release_year: Option<String> = None;
        let mut _manga_type: Option<String> = None;
        let mut views: Option<i32> = None;
        let mut posted_by: Option<String> = None;
        let mut _posted_date: Option<DateTime<Utc>> = None;
        
        let imptdt_selector = Selector::parse(".imptdt").unwrap();
        for element in document.select(&imptdt_selector) {
            // Проверяем наличие h1 тега
            let h1_selector = Selector::parse("h1").unwrap();
            if let Some(h1_element) = element.select(&h1_selector).next() {
                let label = h1_element.text().collect::<String>().to_lowercase();
                let i_selector = Selector::parse("i").unwrap();
                let value = element
                    .select(&i_selector)
                    .next()
                    .map(|el| el.text().collect::<String>().trim().to_string())
                    .unwrap_or_default();
                
                match label.as_str() {
                    "author" => author = value,
                    "artist" => artist = Some(value),
                    "serialization" => _serialization = Some(value),
                    "status" => {
                        let status_text = value.to_lowercase();
                        status = if status_text.contains("ongoing") || status_text.contains("продолжается") {
                            crate::models::MangaStatus::Active
                        } else if status_text.contains("completed") || status_text.contains("завершен") {
                            crate::models::MangaStatus::Completed
                        } else if status_text.contains("hiatus") || status_text.contains("пауза") {
                            crate::models::MangaStatus::Paused
                        } else if status_text.contains("cancelled") || status_text.contains("отменен") ||
                                  status_text.contains("dropped") || status_text.contains("брошен") {
                            crate::models::MangaStatus::Error
                        } else if status_text.contains("season end") || status_text.contains("конец сезона") {
                            crate::models::MangaStatus::Completed
                        } else {
                            crate::models::MangaStatus::Active
                        };
                    }
                    "released" => _release_year = Some(value),
                    "type" => _manga_type = Some(value),
                    "views" => views = value.parse::<i32>().ok(),
                    _ => {}
                }
            } else {
                // Если нет h1, проверяем текстовое содержимое
                let div_text = element.text().collect::<String>().trim().to_string();
                
                if div_text.contains("Posted By") {
                    let author_span_selector = Selector::parse("span.author i").unwrap();
                    if let Some(author_span) = element.select(&author_span_selector).next() {
                        posted_by = Some(author_span.text().collect::<String>().trim().to_string());
                    }
                } else if div_text.contains("Views") {
                    let i_selector = Selector::parse("i").unwrap();
                    if let Some(value_element) = element.select(&i_selector).next() {
                        let value = value_element.text().collect::<String>();
                        let trimmed_value = value.trim();
                        views = trimmed_value.parse::<i32>().ok();
                    }
                } else if div_text.contains("Posted On") {
                    let time_selector = Selector::parse("time").unwrap();
                    if let Some(time_element) = element.select(&time_selector).next() {
                        if let Some(datetime) = time_element.value().attr("datetime") {
                            if let Ok(parsed_date) = DateTime::parse_from_rfc3339(datetime) {
                                _posted_date = Some(parsed_date.with_timezone(&Utc));
                            }
                        }
                    }
                }
            }
        }
        
        info!("[MADARA/THUNDER PARSER] author={}", author);
        debug!("[MADARA/THUNDER PARSER] artist={:?}", artist);
        debug!("[MADARA/THUNDER PARSER] status={:?}", status);
        debug!("[MADARA/THUNDER PARSER] views={:?}", views);
        debug!("[MADARA/THUNDER PARSER] postedBy={:?}", posted_by);
        
        // Главы
        let mut chapters = Vec::new();
        let chapter_selector = Selector::parse("#chapterlist li").unwrap();
        let number_regex = Regex::new(r"Chapter\s*([0-9]+(?:\.[0-9]+)?)").unwrap();
        
        for (index, element) in document.select(&chapter_selector).enumerate() {
            let link_selector = Selector::parse("a").unwrap();
            if let Some(chapter_link) = element.select(&link_selector).next() {
                let chapter_title_selector = Selector::parse(".chapternum").unwrap();
                let chapter_title = chapter_link
                    .select(&chapter_title_selector)
                    .next()
                    .map(|el| el.text().collect::<String>().trim().to_string())
                    .unwrap_or_default();
                
                let chapter_url = chapter_link
                    .value()
                    .attr("href")
                    .unwrap_or_default()
                    .to_string();
                
                // Извлекаем номер главы
                let mut number_string = None;
                if let Some(captures) = number_regex.captures(&chapter_title) {
                    if let Some(number_match) = captures.get(1) {
                        number_string = Some(number_match.as_str().to_string());
                    }
                }
                
                // Извлекаем дату
                let mut _upload_date = Utc::now();
                let date_selector = Selector::parse(".chapterdate").unwrap();
                if let Some(date_element) = chapter_link.select(&date_selector).next() {
                    let date_text = date_element.text().collect::<String>().trim().to_string();
                    if !date_text.is_empty() {
                        // Пытаемся распарсить дату в формате "MMMM d, yyyy"
                        if let Ok(parsed_date) = chrono::NaiveDate::parse_from_str(&date_text, "%B %d, %Y") {
                            _upload_date = parsed_date.and_hms_opt(0, 0, 0)
                                .unwrap_or_default()
                                .and_utc();
                        }
                    }
                }
                
                let chapter_number = number_string
                    .as_ref()
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or((index + 1) as f64);
                
                if !chapter_title.is_empty() && !chapter_url.is_empty() {
                    let chapter = Chapter::new(
                        manga.id.clone(), // manga_id
                        manga.title.clone(),   // manga_title
                        chapter_title,
                        chapter_number,
                        chapter_url,
                    );
                    
                    info!("[MADARA/THUNDER PARSER] Found chapter: {} (number: {}) at {}", 
                          chapter.title, chapter.number, chapter.url);
                    
                    chapters.push(chapter);
                }
            }
        }
        
        info!("[MADARA/THUNDER PARSER] Found {} chapters for manga: {}", chapters.len(), title);
        
        Ok(chapters)
    }
}
