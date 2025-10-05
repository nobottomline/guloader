use crate::traits::{CatalogChecker, CatalogEntry};
use crate::config::SiteConfig;
use crate::error::Result;
use crate::utils::HttpClient;
use scraper::{Html, Selector};
use tracing::info;

pub struct ErosCatalogChecker {
    http_client: HttpClient,
}

impl ErosCatalogChecker {
    pub fn new() -> Self {
        Self { http_client: HttpClient::new() }
    }
}

#[async_trait::async_trait]
impl CatalogChecker for ErosCatalogChecker {
    async fn fetch_first_page(&self, config: &SiteConfig) -> Result<Vec<CatalogEntry>> {
        let html = self.http_client.get(&config.base_url, config).await?;
        let document = Html::parse_document(&html);

        // Blocks of latest updates
        let container_sel = Selector::parse("div.utao.styletwo").unwrap();
        let img_link_sel = Selector::parse(".imgu a.series").unwrap();
        let img_sel = Selector::parse(".imgu img").unwrap();
        let title_sel = Selector::parse("h4").unwrap();

        let mut out = Vec::new();
        for container in document.select(&container_sel) {
            let url = container
                .select(&img_link_sel)
                .filter_map(|a| a.value().attr("href"))
                .next()
                .unwrap_or("")
                .to_string();

            let mut cover = container
                .select(&img_sel)
                .filter_map(|i| i.value().attr("src"))
                .next()
                .unwrap_or("")
                .to_string();

            let title = container
                .select(&title_sel)
                .next()
                .and_then(|h| h.text().next())
                .map(|s| s.trim().to_string())
                .unwrap_or_default();

            if url.is_empty() || title.is_empty() { continue; }

            if cover.starts_with("//") {
                cover = format!("https:{}", cover);
            } else if cover.starts_with('/') {
                cover = format!("{}{}", config.base_url, cover);
            }

            out.push(CatalogEntry { title, url, cover_url: if cover.is_empty() { None } else { Some(cover) } });
        }

        info!("[EROS CATALOG] first page parsed, items: {}", out.len());
        Ok(out)
    }
}


