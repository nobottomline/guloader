use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing::{info, warn, debug};

mod config;
mod database;
mod models;
mod traits;
mod registry;
mod scanners;
mod downloaders;
mod storage;
mod utils;
mod error;

use config::Config;
use database::Database;

#[derive(Parser)]
#[command(name = "guloader")]
#[command(about = "Professional manga monitoring and downloading system")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    
    /// Configuration file path
    #[arg(short, long, default_value = "config.toml")]
    config: String,
    
    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Scan manga chapters
    Scan {
        /// Specific manga name to scan (optional)
        manga: Option<String>,
        /// Only scan for new chapters
        #[arg(short, long)]
        new: bool,
        /// Scan all manga and download new chapters (old update command)
        #[arg(short, long)]
        download: bool,
    },
    /// Download specific chapter from a site
    Download {
        /// Site name (e.g., "eros")
        site: String,
        /// Chapter URL to download
        chapter_url: String,
    },
    /// Monitor manga for new chapters and auto-download
    Monitor {
        /// Automatically commit downloaded files to git
        #[arg(long)]
        auto_commit: bool,
    },
    /// Initialize database and configuration
    Init,
    /// Show status of monitored manga
    Status,
    /// Clean up old downloads
    Cleanup {
        /// Days to keep downloads
        #[arg(default_value = "30")]
        days: u32,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    
    // Initialize logging
    init_logging(cli.verbose)?;
    
    // Load configuration
    let config = Config::load(&cli.config)?;
    
    // Initialize database
    let db = Database::new(&config.database.url).await?;
    
    match cli.command {
        Commands::Init => {
            info!("Initializing manga monitor...");
            db.init().await?;
            
            // Load manga from config into database
            info!("Loading manga from configuration...");
            for manga_config in &config.manga {
                if manga_config.active {
                    let manga = crate::models::Manga {
                        id: uuid::Uuid::new_v4().to_string(),
                        title: manga_config.title.clone(),
                        site: manga_config.site.clone(),
                        url: manga_config.url.clone(),
                        status: crate::models::MangaStatus::Active,
                        chapter_count: 0,
                        last_updated: chrono::Utc::now(),
                        created_at: chrono::Utc::now(),
                        updated_at: chrono::Utc::now(),
                        cover_url: None,
                        description: None,
                    };
                    
                    // Check if manga already exists
                    if let Ok(Some(_)) = db.get_manga_by_url(&manga.url).await {
                        info!("Manga '{}' already exists in database", manga.title);
                    } else {
                        db.create_manga(&manga).await?;
                        info!("Added manga '{}' to database", manga.title);
                    }
                }
            }
            
            info!("Database initialized successfully");
        }
        Commands::Scan { manga, new, download } => {
            if download {
                // This is the old "update" command - scan all and download new chapters
                info!("🔄 Starting automatic update process...");
                
                // Step 1: Scan all manga for new chapters
                info!("📡 Step 1: Scanning all manga for new chapters...");
                let scanner_registry = registry::ScannerRegistry::new();
                scanner_registry.scan_all_manga(&config, &db).await?;
                
                // Step 2: Download new chapters
                info!("⬇️ Step 2: Downloading new chapters...");
                run_download_new_chapters(&config, &db).await?;
                
            } else if let Some(manga_name) = manga {
                // Scan specific manga
                info!("Scanning manga: {}", manga_name);
                run_scan_manga(&config, &db, &manga_name).await?;
            } else if new {
                // Scan for new chapters only
                info!("Scanning for new chapters only...");
                run_scan_new(&config, &db).await?;
            } else {
                // Scan all manga
                info!("Scanning all manga chapters...");
                run_scan_all(&config, &db).await?;
            }
        }
        Commands::Download { site, chapter_url } => {
            info!("Downloading chapter from {}: {}", site, chapter_url);
            run_downloader_by_site(&config, &db, &site, &chapter_url).await?;
        }
        Commands::Monitor { auto_commit } => {
            info!("Starting manga monitoring...");
            run_monitor(&config, &db, auto_commit).await?;
        }
        Commands::Status => {
            info!("Showing manga status...");
            show_status(&db).await?;
        }
        Commands::Cleanup { days } => {
            info!("Cleaning up downloads older than {} days", days);
            cleanup_old_downloads(&config, &db, days).await?;
        }
    }
    
    Ok(())
}

fn init_logging(verbose: bool) -> Result<()> {
    let level = if verbose { "debug" } else { "info" };
    
    tracing_subscriber::fmt()
        .with_env_filter(format!("guloader={}", level))
        .with_target(false)
        .with_thread_ids(true)
        .with_thread_names(true)
        .init();
    
    Ok(())
}

async fn run_scan_all(config: &Config, db: &Database) -> Result<()> {
    use registry::ScannerRegistry;
    
    let scanner_registry = ScannerRegistry::new();
    let manga_list = db.get_all_manga().await?;
    
    println!("📚 Scanning all manga chapters...");
    println!("{:<30} {:<10} {:<20}", "Manga", "Chapters", "Latest Chapter");
    println!("{}", "-".repeat(60));
    
    for manga in manga_list {
        if manga.status == crate::models::MangaStatus::Active {
            match scanner_registry.scan_manga(config, db, &manga.id).await {
                Ok(_) => {
                    let chapters = db.get_chapters_by_manga_id(&manga.id).await?;
                    let latest_chapter = chapters.iter()
                        .max_by(|a, b| a.number.partial_cmp(&b.number).unwrap());
                    
                    let latest_title = latest_chapter
                        .map(|c| c.title.as_str())
                        .unwrap_or("No chapters");
                    
                    println!("{:<30} {:<10} {:<20}", 
                        manga.title, 
                        chapters.len(), 
                        latest_title
                    );
                }
                Err(e) => {
                    println!("{:<30} {:<10} {:<20}", 
                        manga.title, 
                        "ERROR", 
                        e.to_string()
                    );
                }
            }
        }
    }
    
    Ok(())
}

async fn run_scan_new(config: &Config, db: &Database) -> Result<()> {
    use registry::ScannerRegistry;
    
    let scanner_registry = ScannerRegistry::new();
    let manga_list = db.get_all_manga().await?;
    
    println!("🆕 Scanning for new chapters...");
    println!("{:<30} {:<10} {:<20}", "Manga", "New Chapters", "Latest New Chapter");
    println!("{}", "-".repeat(60));
    
    for manga in manga_list {
        if manga.status == crate::models::MangaStatus::Active {
            let old_chapters_count = db.get_chapters_by_manga_id(&manga.id).await?.len();
            
            match scanner_registry.scan_manga(config, db, &manga.id).await {
                Ok(_) => {
                    let new_chapters_count = db.get_chapters_by_manga_id(&manga.id).await?.len();
                    let new_chapters = new_chapters_count - old_chapters_count;
                    
                    if new_chapters > 0 {
                        let chapters = db.get_chapters_by_manga_id(&manga.id).await?;
                        let latest_chapter = chapters.iter()
                            .max_by(|a, b| a.number.partial_cmp(&b.number).unwrap());
                        
                        let latest_title = latest_chapter
                            .map(|c| c.title.as_str())
                            .unwrap_or("No chapters");
                        
                        println!("{:<30} {:<10} {:<20}", 
                            manga.title, 
                            new_chapters, 
                            latest_title
                        );
                    }
                }
                Err(e) => {
                    println!("{:<30} {:<10} {:<20}", 
                        manga.title, 
                        "ERROR", 
                        e.to_string()
                    );
                }
            }
        }
    }
    
    Ok(())
}

async fn run_scan_manga(config: &Config, db: &Database, manga_name: &str) -> Result<()> {
    use registry::ScannerRegistry;
    
    let scanner_registry = ScannerRegistry::new();
    
    // Find manga by name
    let manga_list = db.get_all_manga().await?;
    let manga = manga_list.iter()
        .find(|m| m.title.to_lowercase().contains(&manga_name.to_lowercase()))
        .ok_or_else(|| anyhow::anyhow!("Manga '{}' not found", manga_name))?;
    
    println!("📖 Scanning manga: {}", manga.title);
    println!("{:<10} {:<20} {:<15}", "Number", "Title", "Status");
    println!("{}", "-".repeat(45));
    
    match scanner_registry.scan_manga(config, db, &manga.id).await {
        Ok(_) => {
            let chapters = db.get_chapters_by_manga_id(&manga.id).await?;
            
            for chapter in chapters {
                println!("{:<10} {:<20} {:<15}", 
                    chapter.number, 
                    chapter.title, 
                    format!("{:?}", chapter.status)
                );
            }
        }
        Err(e) => {
            println!("Error scanning manga: {}", e);
        }
    }
    
    Ok(())
}

async fn run_downloader_by_site(config: &Config, db: &Database, site: &str, chapter_url: &str) -> Result<()> {
    use registry::DownloaderRegistry;
    
    let downloader_registry = DownloaderRegistry::new();
    
    // Get site config
    let _site_config = config.get_site_config(site)
        .ok_or_else(|| anyhow::anyhow!("Site '{}' not supported", site))?;
    
    // Create a temporary manga first
    let temp_manga = crate::models::Manga {
        id: uuid::Uuid::new_v4().to_string(),
        title: "temp".to_string(),
        site: site.to_string(),
        url: "temp".to_string(),
        description: None,
        cover_url: None,
        status: crate::models::MangaStatus::Active,
        chapter_count: 0,
        last_updated: chrono::Utc::now(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    
    // Save the temporary manga to database
    db.create_manga(&temp_manga).await?;
    
    // Create a temporary chapter object for downloading
    let chapter = crate::models::Chapter::new(
        temp_manga.id.clone(), // manga_id
        temp_manga.title.clone(), // manga_title
        "temp".to_string(), // title (will be updated)
        0.0, // number (will be updated)
        chapter_url.to_string(),
    );
    
    // First, save the chapter to database
    db.create_chapter(&chapter).await?;
    
    // Download using the site-specific downloader
    match downloader_registry.download_chapter(config, db, chapter_url).await {
        Ok(_) => {
            println!("✅ Successfully downloaded chapter from {}", site);
        }
        Err(e) => {
            println!("❌ Failed to download chapter: {}", e);
            return Err(e.into());
        }
    }
    
    Ok(())
}

async fn run_download_new_chapters(config: &Config, db: &Database) -> Result<()> {
    use registry::DownloaderRegistry;

    info!("⬇️ Finding and downloading new chapters...");
    let downloader_registry = DownloaderRegistry::new();

    // Get all manga from database
    let manga_list = db.get_all_manga().await?;

    for manga in manga_list {
        if manga.status == crate::models::MangaStatus::Active {
            info!("🔍 Checking manga: {}", manga.title);

            // Get chapters for this manga
            let chapters = db.get_chapters_by_manga_id(&manga.id).await?;

            // Find chapters that haven't been downloaded yet
            for chapter in chapters {
                if chapter.status == crate::models::ChapterStatus::Pending {
                    info!("📥 Found new chapter: {} - {}", manga.title, chapter.title);

                    // Download the chapter using the new method
                    match downloader_registry.download_chapter_to_scans(config, db, &chapter).await {
                        Ok(_) => {
                            info!("✅ Successfully downloaded: {} - {}", manga.title, chapter.title);
                        }
                        Err(e) => {
                            warn!("❌ Failed to download {} - {}: {}", manga.title, chapter.title, e);
                        }
                    }
                }
            }
        }
    }

    info!("🎉 Download process completed!");
    Ok(())
}

async fn show_status(db: &Database) -> Result<()> {
    let manga_list = db.get_all_manga().await?;
    
    println!("📚 Monitored Manga Status:");
    println!("{:<20} {:<10} {:<15} {:<20}", "Title", "Chapters", "Last Update", "Status");
    println!("{}", "-".repeat(70));
    
    for manga in manga_list {
        println!(
            "{:<20} {:<10} {:<15} {:<20}",
            manga.title,
            manga.chapter_count,
            manga.last_updated.format("%Y-%m-%d"),
            manga.status
        );
    }
    
    Ok(())
}

async fn cleanup_old_downloads(config: &Config, db: &Database, days: u32) -> Result<()> {
    use storage::StorageManager;
    
    let storage = StorageManager::new(&config.storage);
    let cutoff_date = chrono::Utc::now() - chrono::Duration::days(days as i64);
    
    let old_chapters = db.get_old_chapters(cutoff_date).await?;
    let old_chapters_count = old_chapters.len();
    
    for chapter in old_chapters {
        info!("Cleaning up chapter: {}", chapter.title);
        storage.remove_chapter(&chapter.manga_title, &chapter).await?;
        db.mark_chapter_deleted(&chapter.id).await?;
    }
    
    info!("Cleanup completed. Removed {} old chapters", old_chapters_count);
    Ok(())
}

async fn run_monitor(config: &Config, db: &Database, auto_commit: bool) -> Result<()> {
    use registry::{ScannerRegistry, DownloaderRegistry};
    
    info!("🔍 Starting manga monitoring cycle...");
    
    let scanner_registry = ScannerRegistry::new();
    let downloader_registry = DownloaderRegistry::new();
    
    // Получаем все манги из конфига
    let all_manga = db.get_all_manga().await?;
    let mut new_chapters_found = 0;
    let mut chapters_downloaded = 0;
    let mut failed_downloads = 0;
    
    for manga in &all_manga {
        info!("📖 Monitoring manga: {}", manga.title);
        
        // Получаем конфигурацию сайта
        let site_config = match config.get_site_config(&manga.site) {
            Some(config) => config,
            None => {
                warn!("⚠️ Site '{}' not configured, skipping manga: {}", manga.site, manga.title);
                continue;
            }
        };
        
        // Сканируем мангу для поиска новых глав
        let scanner = scanner_registry.get_scanner(&manga.site)
            .ok_or_else(|| anyhow::anyhow!("Scanner for site '{}' not found", manga.site))?;
        
        match scanner.scan_manga(site_config, &manga).await {
            Ok(chapters) => {
                // Получаем существующие главы из базы данных
                let existing_chapters = db.get_chapters_by_manga_id(&manga.id).await?;
                let existing_numbers: std::collections::HashSet<String> = existing_chapters
                    .iter()
                    .map(|c| c.number.to_string())
                    .collect();
                
                // Находим новые главы
                let new_chapters: Vec<_> = chapters
                    .into_iter()
                    .filter(|chapter| !existing_numbers.contains(&chapter.number.to_string()))
                    .collect();
                
                if !new_chapters.is_empty() {
                    info!("🆕 Found {} new chapters for manga: {}", new_chapters.len(), manga.title);
                    new_chapters_found += new_chapters.len();
                    
                    // Скачиваем новые главы
                    for chapter in new_chapters {
                        info!("⬇️ Downloading new chapter: {} (Chapter {})", chapter.title, chapter.number);
                        
                        // Сначала сохраняем главу в базу данных
                        db.create_chapter(&chapter).await?;
                        
                        // Пытаемся скачать главу
                        match downloader_registry.download_chapter(config, db, &chapter.url).await {
                            Ok(_) => {
                                info!("✅ Successfully downloaded chapter: {}", chapter.title);
                                chapters_downloaded += 1;
                                
                                // Обновляем статус главы
                                let mut updated_chapter = chapter.clone();
                                updated_chapter.status = crate::models::ChapterStatus::Downloaded;
                                updated_chapter.downloaded_at = Some(chrono::Utc::now());
                                db.update_chapter(&updated_chapter).await?;
                            }
                            Err(e) => {
                                warn!("❌ Failed to download chapter: {} - {}", chapter.title, e);
                                failed_downloads += 1;
                                
                                // Обновляем статус главы как неудачной
                                let mut updated_chapter = chapter.clone();
                                updated_chapter.status = crate::models::ChapterStatus::Failed;
                                db.update_chapter(&updated_chapter).await?;
                            }
                        }
                    }
                } else {
                    debug!("📋 No new chapters found for manga: {}", manga.title);
                }
            }
            Err(e) => {
                warn!("⚠️ Failed to scan manga '{}': {}", manga.title, e);
            }
        }
        
        // Проверяем неудачные загрузки и пытаемся их перезагрузить
        let failed_chapters = db.get_chapters_by_manga_id(&manga.id).await?
            .into_iter()
            .filter(|c| c.status == crate::models::ChapterStatus::Failed)
            .collect::<Vec<_>>();
        
        for chapter in failed_chapters {
            info!("🔄 Retrying failed chapter: {}", chapter.title);
            
            match downloader_registry.download_chapter(config, db, &chapter.url).await {
                Ok(_) => {
                    info!("✅ Successfully retried chapter: {}", chapter.title);
                    chapters_downloaded += 1;
                    
                    // Обновляем статус главы
                    let mut updated_chapter = chapter.clone();
                    updated_chapter.status = crate::models::ChapterStatus::Downloaded;
                    updated_chapter.downloaded_at = Some(chrono::Utc::now());
                    db.update_chapter(&updated_chapter).await?;
                }
                Err(e) => {
                    debug!("⏳ Chapter still not available: {} - {}", chapter.title, e);
                }
            }
        }
    }
    
    // Обновляем статистику манги
    for manga in &all_manga {
        let chapter_count = db.get_chapters_by_manga_id(&manga.id).await?.len() as i32;
        let mut updated_manga = manga.clone();
        updated_manga.chapter_count = chapter_count;
        updated_manga.last_updated = chrono::Utc::now();
        updated_manga.updated_at = chrono::Utc::now();
        db.update_manga(&updated_manga).await?;
    }
    
    info!("📊 Monitoring cycle completed:");
    info!("   🆕 New chapters found: {}", new_chapters_found);
    info!("   ⬇️ Chapters downloaded: {}", chapters_downloaded);
    info!("   ❌ Failed downloads: {}", failed_downloads);
    
    if auto_commit && (new_chapters_found > 0 || chapters_downloaded > 0) {
        info!("🔄 Auto-commit is enabled, but git operations should be handled by GitHub Actions");
    }
    
    Ok(())
}
