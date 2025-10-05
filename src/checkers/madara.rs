use crate::traits::{CatalogChecker, CatalogEntry};
use crate::config::SiteConfig;
use crate::error::Result;
use crate::utils::HttpClient;
use scraper::{Html, Selector};
use regex::Regex;
use tracing::info;

pub struct MadaraCatalogChecker {
    http_client: HttpClient,
}

impl MadaraCatalogChecker {
    pub fn new() -> Self { Self { http_client: HttpClient::new() } }
}

#[async_trait::async_trait]
impl CatalogChecker for MadaraCatalogChecker {
    async fn fetch_first_page(&self, config: &SiteConfig) -> Result<Vec<CatalogEntry>> {
        // Для скорости берём только первую страницу, как в ТЗ
        let html = self.http_client.get(&config.base_url, config).await?;
        let document = Html::parse_document(&html);

        // 1) Попытка через CSS-селекторы
        let list_sel = Selector::parse(".listupd, .list-update, .postbody, body").unwrap();
        let item_sel = Selector::parse(".bsx, .bs, .utao .bsx").unwrap();
        let a_sel = Selector::parse("a[title][href]").unwrap();
        let img_sel = Selector::parse("img[src]").unwrap();

        let mut out = Vec::new();
        for list in document.select(&list_sel) {
            for item in list.select(&item_sel) {
                let mut url = String::new();
                let mut title = String::new();
                if let Some(a) = item.select(&a_sel).next() {
                    if let Some(href) = a.value().attr("href") { url = href.to_string(); }
                    if let Some(t) = a.value().attr("title") { title = t.trim().to_string(); }
                }
                let mut cover = String::new();
                if let Some(img) = item.select(&img_sel).next() {
                    if let Some(src) = img.value().attr("src") { cover = src.to_string(); }
                }
                if !url.is_empty() && !title.is_empty() && !cover.is_empty() {
                    // нормализация cover
                    if cover.starts_with("//") {
                        cover = format!("https:{}", cover);
                    } else if cover.starts_with('/') {
                        cover = format!("{}{}", config.base_url, cover);
                    }
                    out.push(CatalogEntry { title, url, cover_url: Some(cover) });
                }
            }
        }

        // 2) Если ничего не нашли — fallback на regex по шаблону из Swift
        if out.is_empty() {
            let bsx_re = Regex::new(r#"<div class=\"bsx\">([\s\S]*?)</div>\s*</div>"#).ok();
            if let Some(re) = bsx_re {
                for caps in re.captures_iter(&html) {
                    let block = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                    let url_re = Regex::new(r#"<a href=\"([^\"]+)\"[^>]*title=\"([^\"]+)\""#).ok();
                    let img_re = Regex::new(r#"<img[^>]+src=\"([^\"]+)\"[^>]*>"#).ok();
                    let mut url = String::new();
                    let mut title = String::new();
                    let mut cover = String::new();
                    if let Some(Some(m)) = url_re.as_ref().map(|r| r.captures(block)) {
                        url = m.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
                        title = m.get(2).map(|m| m.as_str().to_string()).unwrap_or_default();
                        title = title.trim().to_string();
                    }
                    if let Some(Some(m)) = img_re.as_ref().map(|r| r.captures(block)) {
                        cover = m.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
                    }
                    if !url.is_empty() && !title.is_empty() && !cover.is_empty() {
                        if cover.starts_with("//") {
                            cover = format!("https:{}", cover);
                        } else if cover.starts_with('/') {
                            cover = format!("{}{}", config.base_url, cover);
                        }
                        out.push(CatalogEntry { title, url, cover_url: Some(cover) });
                    }
                }
            }
        }

        info!("[MADARA/THUNDER CATALOG] first page parsed, items: {}", out.len());
        Ok(out)
    }
}