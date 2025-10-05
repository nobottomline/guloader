use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub database: DatabaseConfig,
    pub storage: StorageConfig,
    pub sites: HashMap<String, SiteConfig>,
    pub scanner: ScannerConfig,
    pub notifications: NotificationConfig,
    pub manga: Vec<MangaConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub base_path: String,
    pub scans_path: String,
    pub max_size_gb: u64,
    pub compression: bool,
    pub thumbnail_size: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteConfig {
    pub name: String,
    pub base_url: String,
    pub scanner_type: String,
    pub downloader_type: String,
    pub rate_limit_ms: u64,
    pub user_agent: Option<String>,
    pub headers: Option<HashMap<String, String>>,
    pub selectors: SelectorsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectorsConfig {
    pub manga_list: String,
    pub chapter_list: String,
    pub chapter_title: String,
    pub chapter_url: String,
    pub image_container: String,
    pub image_url: String,
    pub next_page: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MangaConfig {
    pub title: String,
    pub site: String,
    pub url: String,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScannerConfig {
    pub interval_minutes: u64,
    pub max_concurrent_scans: usize,
    pub retry_attempts: u32,
    pub retry_delay_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationConfig {
    pub discord_webhook: Option<String>,
    pub telegram_bot_token: Option<String>,
    pub telegram_chat_id: Option<String>,
    pub email_smtp: Option<SmtpConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmtpConfig {
    pub server: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub from: String,
    pub to: Vec<String>,
}

impl Config {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }
    
    
    pub fn get_site_config(&self, site_name: &str) -> Option<&SiteConfig> {
        self.sites.get(site_name)
    }
}

impl Default for Config {
    fn default() -> Self {
        let mut sites = HashMap::new();
        
        // Eros-moon.xyz configuration
        sites.insert("eros".to_string(), SiteConfig {
            name: "Eros Moon".to_string(),
            base_url: "https://eros-moon.xyz".to_string(),
            scanner_type: "eros".to_string(),
            downloader_type: "eros".to_string(),
            rate_limit_ms: 1500,
            user_agent: Some("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36".to_string()),
            headers: None,
            selectors: SelectorsConfig {
                manga_list: ".story_item".to_string(),
                chapter_list: "#chapterlist li".to_string(),
                chapter_title: ".chapternum".to_string(),
                chapter_url: ".eph-num a".to_string(),
                image_container: ".reader-main".to_string(),
                image_url: ".reader-main img".to_string(),
                next_page: Some(".nav-next a".to_string()),
            },
        });
        
        Config {
            database: DatabaseConfig {
                url: "sqlite:data/guloader.db".to_string(),
                max_connections: 10,
            },
            storage: StorageConfig {
                base_path: "./downloads".to_string(),
                scans_path: "./scans".to_string(),
                max_size_gb: 50,
                compression: true,
                thumbnail_size: 200,
            },
            sites,
            scanner: ScannerConfig {
                interval_minutes: 10,
                max_concurrent_scans: 5,
                retry_attempts: 3,
                retry_delay_ms: 5000,
            },
            notifications: NotificationConfig {
                discord_webhook: None,
                telegram_bot_token: None,
                telegram_chat_id: None,
                email_smtp: None,
            },
            manga: Vec::new(),
        }
    }
}
