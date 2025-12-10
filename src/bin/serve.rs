//! Rustipedia Web Server
//!
//! Serve your downloaded Wikipedia locally with a beautiful, searchable interface.
//!
//! # Examples
//!
//! Serve with default settings:
//! ```bash
//! rustipedia-serve
//! ```
//!
//! Specify data directory and port:
//! ```bash
//! rustipedia-serve --data ./my-wiki --port 3000
//! ```

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::ffi::OsString;

use anyhow::Result;
use axum::{
    extract::{Path, Query, State, Form, Json, Multipart},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Router,
    http::{HeaderName, HeaderValue, header},
};
use clap::Parser;
use tokio::sync::RwLock;
use tracing_subscriber::EnvFilter;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::cors::{CorsLayer, Any};
use tower_governor::{governor::GovernorConfigBuilder, GovernorLayer};
use rand::Rng;

use rustipedia::{Article, SearchIndex, WikiLanguage, UpdateConfig, UpdateSchedule, Weekday, UpdateManager};

// Windows service support
#[cfg(windows)]
use std::sync::Mutex;
#[cfg(windows)]
use std::time::Duration;
#[cfg(windows)]
use windows_service::{
    define_windows_service,
    service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult},
    service_dispatcher,
};

const DEFAULT_LOGO: &[u8] = include_bytes!("Logo.png");

// Global shutdown flag for Windows service
#[cfg(windows)]
static SHUTDOWN_FLAG: Mutex<bool> = Mutex::new(false);

// Service name for Windows
#[cfg(windows)]
const SERVICE_NAME: &str = "rustipedia-serve";

#[derive(Parser)]
#[command(name = "rustipedia-serve")]
#[command(author, version, about = "Serve your local Wikipedia")]
#[command(long_about = r#"
╔══════════════════════════════════════════════════════════════════╗
║                     RUSTIPEDIA                                    ║
║            Your Local Wikipedia Server                           ║
╚══════════════════════════════════════════════════════════════════╝

Serve your downloaded Wikipedia with a beautiful, searchable interface.
Browse articles, search by title or content, and explore offline!

EXAMPLES:
  Start server with defaults:
    rustipedia-serve

  Use custom data directory:
    rustipedia-serve --data ./my-wiki

  Use custom port:
    rustipedia-serve --port 3000

  Bind to all interfaces (for network access):
    rustipedia-serve --host 0.0.0.0
"#)]
struct Cli {
    /// Directory containing Wikipedia data
    #[arg(short, long, default_value = "wikipedia")]
    data: PathBuf,

    /// Port to listen on
    #[arg(short, long)]
    port: Option<u16>,

    /// Host to bind to
    #[arg(long)]
    host: Option<String>,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,
}

/// Application state shared across handlers
struct AppState {
    /// Articles indexed by ID (fallback if search index is missing)
    articles: HashMap<u64, Article>,
    /// Articles indexed by title (lowercase)
    by_title: HashMap<String, u64>,
    /// Search index (optional)
    search_index: Option<SearchIndex>,
    /// List of all titles for browsing
    all_titles: Vec<(u64, String)>,
    /// Wikipedia language
    language: String,
    /// Total article count
    article_count: usize,
    /// Data directory
    data_dir: PathBuf,
    /// Auto-update configuration
    update_config: UpdateConfig,
    /// Configured port (from config.json)
    config_port: Option<u16>,
    /// Configured host (from config.json)
    config_host: Option<String>,
}

impl AppState {
    fn load(data_dir: &PathBuf) -> Result<Self> {
        let articles_path = data_dir.join("articles.jsonl");
        if !articles_path.exists() {
            anyhow::bail!("Articles file not found: {:?}. Run rustipedia-download first.", articles_path);
        }

        // Try to load search index
        let index_path = data_dir.join("search_index");
        let search_index = if index_path.exists() {
            tracing::info!("Loading search index...");
            match SearchIndex::open(&index_path) {
                Ok(index) => {
                    tracing::info!("Search index loaded");
                    Some(index)
                }
                Err(e) => {
                    tracing::warn!("Failed to load search index: {}. Search disabled.", e);
                    None
                }
            }
        } else {
            tracing::warn!("No search index found. Search disabled. Run: rustipedia-download index {:?}", data_dir);
            None
        };

        tracing::info!("Loading articles from {:?}...", articles_path);
        
        let file = File::open(&articles_path)?;
        let reader = BufReader::new(file);
        
        let mut articles = HashMap::new();
        let mut by_title = HashMap::new();
        let mut all_titles = Vec::new();
        
        for line in reader.lines() {
            let line = line?;
            if line.is_empty() {
                continue;
            }
            
            // Optimization: If we have a search index, we don't need to load the full article content into RAM.
            // We just need the ID and Title for routing/listing.
            // However, we currently parse the whole line anyway. 
            // To truly optimize, we would need a lighter parsing or separate index file.
            // For now, we just avoid storing the heavy content in the HashMap.
            
            let article: Article = serde_json::from_str(&line)?;
            let id = article.id;
            let title = article.title.clone();
            
            by_title.insert(title.to_lowercase(), id);
            all_titles.push((id, title));
            
            if search_index.is_none() {
                articles.insert(id, article);
            }
        }
        
        all_titles.sort_by(|a, b| a.1.cmp(&b.1));
        let article_count = all_titles.len();
        
        tracing::info!("Loaded {} articles (Content loaded: {})", article_count, search_index.is_none());

        // Try to load config for language info
        let config_path = data_dir.join("config.json");
        let language = if config_path.exists() {
            let content = fs::read_to_string(&config_path).unwrap_or_default();
            serde_json::from_str::<serde_json::Value>(&content)
                .ok()
                .and_then(|v| v["language"].as_str().map(String::from))
                .unwrap_or_else(|| "unknown".to_string())
        } else {
            "unknown".to_string()
        };

        // Try to load port/host from config
        let (config_port, config_host) = if config_path.exists() {
            let content = fs::read_to_string(&config_path).unwrap_or_default();
            let v: serde_json::Value = serde_json::from_str(&content).unwrap_or(serde_json::json!({}));
            (
                v["port"].as_u64().map(|p| p as u16),
                v["host"].as_str().map(String::from)
            )
        } else {
            (None, None)
        };

        // Load update config
        let update_config = UpdateConfig::load(UpdateConfig::config_path(data_dir)).unwrap_or_default();

        Ok(Self {
            articles,
            by_title,
            search_index,
            all_titles,
            language,
            article_count,
            data_dir: data_dir.clone(),
            update_config,
            config_port,
            config_host,
        })
    }

    /// Get an article by ID from either the search index or in-memory storage
    fn get_article_by_id(&self, id: u64) -> Option<Article> {
        if let Some(ref index) = self.search_index {
            index.get_article(id).ok().flatten()
        } else {
            self.articles.get(&id).cloned()
        }
    }

    /// Get an article by title
    fn get_article_by_title(&self, title: &str) -> Option<Article> {
        let title_lower = title.to_lowercase().replace('_', " ");
        let id = self.by_title.get(&title_lower)?;
        self.get_article_by_id(*id)
    }

    /// Get article preview by ID
    fn get_article_preview(&self, id: u64, length: usize) -> String {
        if let Some(ref index) = self.search_index {
            index.get_by_id(id)
                .ok()
                .flatten()
                .map(|r| r.preview)
                .unwrap_or_default()
        } else {
            self.articles.get(&id)
                .map(|a| a.preview(length).to_string())
                .unwrap_or_default()
        }
    }
}


type SharedState = Arc<RwLock<AppState>>;

// Main entry point - detects if running as service or CLI
fn main() -> Result<()> {
    #[cfg(windows)]
    {
        // Try to run as Windows service first
        if let Err(_) = service_dispatcher::start(SERVICE_NAME, ffi_service_main) {
            // If that fails, we're probably running as CLI
            run_cli_mode()
        } else {
            Ok(())
        }
    }
    
    #[cfg(not(windows))]
    {
        run_cli_mode()
    }
}

// CLI mode entry point
fn run_cli_mode() -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        run_server(None).await
    })
}

// Windows service entry point
#[cfg(windows)]
define_windows_service!(ffi_service_main, service_main);

#[cfg(windows)]
fn service_main(arguments: Vec<OsString>) {
    if let Err(e) = run_service(arguments) {
        // Log error to Windows Event Log or file
        let _ = log_service_error(&format!("Service error: {}", e));
    }
}

#[cfg(windows)]
fn log_service_error(msg: &str) -> Result<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("C:\\ProgramData\\Rustipedia\\service_error.log")?;
    writeln!(file, "[{}] {}", chrono::Local::now(), msg)?;
    Ok(())
}

#[cfg(windows)]
fn run_service(_arguments: Vec<OsString>) -> Result<()> {
    use std::sync::mpsc;
    
    let (shutdown_tx, shutdown_rx) = mpsc::channel();
    
    // Define service control handler
    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Stop | ServiceControl::Interrogate => {
                let _ = shutdown_tx.send(());
                ServiceControlHandlerResult::NoError
            }
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };
    
    // Register service control handler
    let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)?;
    
    // Tell Windows we're starting
    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::StartPending,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::from_secs(5),
        process_id: None,
    })?;
    
    // Start the server in a separate thread
    let server_handle = std::thread::spawn(|| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            run_server(Some("Service mode")).await
        })
    });
    
    // Tell Windows we're running
    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;
    
    // Wait for shutdown signal
    let _ = shutdown_rx.recv();
    
    // Tell Windows we're stopping
    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::StopPending,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::from_secs(5),
        process_id: None,
    })?;
    
    // Set shutdown flag
    *SHUTDOWN_FLAG.lock().unwrap() = true;
    
    // Wait for server to stop (with timeout)
    let _ = server_handle.join();
    
    // Tell Windows we've stopped
    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;
    
    Ok(())
}

// Server logic - can be called from either service or CLI mode
async fn run_server(mode: Option<&str>) -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging - for service mode, log to file
    let is_service = mode.is_some();
    
    if is_service {
        // Service mode - log to file only
        // Create log directory if needed
        #[cfg(windows)]
        {
            let _ = std::fs::create_dir_all("C:\\ProgramData\\Rustipedia");
        }
        
        let log_path = if cfg!(windows) {
            "C:\\ProgramData\\Rustipedia\\server.log"
        } else {
            "server.log"
        };
        
        let log_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)
            .unwrap_or_else(|_| {
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open("server.log")
                    .unwrap()
            });
        
        let filter = EnvFilter::new("rustipedia_serve=info,warn");
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_target(false)
            .with_writer(std::sync::Arc::new(log_file))
            .init();
            
        tracing::info!("Starting in {}", mode.unwrap_or("unknown mode"));
    } else {
        // CLI mode - log to stdout
        let filter = if cli.verbose {
            EnvFilter::new("rustipedia_serve=debug,tower_http=debug,info")
        } else {
            EnvFilter::new("rustipedia_serve=info,warn")
        };
        
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_target(false)
            .init();
    }

    // Load data
    let state = AppState::load(&cli.data)?;
    let config_port = state.config_port;
    let config_host = state.config_host.clone();
    let shared_state: SharedState = Arc::new(RwLock::new(state));

    // Build router
    let app = Router::new()
        .route("/", get(home))
        .route("/article/:id", get(article_by_id))
        .route("/wiki/:title", get(article_by_title))
        .route("/search", get(search))
        .route("/browse", get(browse))
        .route("/random", get(random_article))
        .route("/api/articles", get(api_articles))
        .route("/api/search", get(api_search))
        .route("/settings", get(settings_page).post(update_settings))
        .route("/api/update/status", get(api_update_status))
        .route("/api/update/trigger", post(api_trigger_update))
        .route("/api/update/history", get(api_update_history))
        .route("/logo", get(logo_handler))
        .route("/settings/logo", post(upload_logo))
        .with_state(shared_state);

    // Rate Limiting Configuration
    let governor_conf = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(50) // 50 requests per second
            .burst_size(100)
            .finish()
            .unwrap(),
    );

    // CSP Header
    // Note: 'unsafe-inline' is currently required for the inline styles in base_html.
    // Ideally we should move to a CSS file or use nonces.
    let csp = "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; font-src 'self' https://fonts.gstatic.com; img-src 'self' data:;";

    let app = app
        .layer(GovernorLayer { config: governor_conf })
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("content-security-policy"),
            HeaderValue::from_str(csp).unwrap(),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("x-content-type-options"),
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("x-frame-options"),
            HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("x-xss-protection"),
            HeaderValue::from_static("1; mode=block"),
        ))
        .layer(TimeoutLayer::new(std::time::Duration::from_secs(30)))
        .layer(
            CorsLayer::new()
                .allow_origin(Any) // For a local tool, Any is acceptable, but in prod we'd restrict.
                .allow_methods(Any)
                .allow_headers(Any)
        );

    // Determine port and host: CLI > Config > Default
    let port = cli.port.or(config_port).unwrap_or(8080);
    let host = cli.host.or(config_host).unwrap_or_else(|| "127.0.0.1".to_string());
    let addr = format!("{}:{}", host, port);
    
    if !is_service {
        println!();
        println!("╔══════════════════════════════════════════════════════════════════╗");
        println!("║                     RUSTIPEDIA                                    ║");
        println!("╠══════════════════════════════════════════════════════════════════╣");
        println!("║  Server running at: http://{}                          ", addr);
        println!("║  Data directory:    {:?}                                ", cli.data);
        println!("╚══════════════════════════════════════════════════════════════════╝");
        println!();
        println!("Press Ctrl+C to stop the server");
    } else {
        tracing::info!("Server starting at http://{}", addr);
        tracing::info!("Data directory: {:?}", cli.data);
    }

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    
    // Run server with graceful shutdown for service mode
    #[cfg(windows)]
    if is_service {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>()
        )
        .with_graceful_shutdown(async {
            loop {
                tokio::time::sleep(Duration::from_millis(100)).await;
                if *SHUTDOWN_FLAG.lock().unwrap() {
                    break;
                }
            }
        })
        .await?;
    } else {
        axum::serve(listener, app.into_make_service_with_connect_info::<std::net::SocketAddr>()).await?;
    }
    
    #[cfg(not(windows))]
    {
        axum::serve(listener, app.into_make_service_with_connect_info::<std::net::SocketAddr>()).await?;
    }

    Ok(())
}


// ============================================================================
// HTML Templates
// ============================================================================

fn base_html(title: &str, content: &str, state: &AppState) -> String {
    format!(r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{} - Rustipedia</title>
    <link rel="preconnect" href="https://fonts.googleapis.com">
    <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
    <link href="https://fonts.googleapis.com/css2?family=Outfit:wght@300;400;500;600;700&family=Crimson+Pro:ital,wght@0,400;0,600;1,400&display=swap" rel="stylesheet">
    <style>
        :root {{
            --bg-primary: #f8fafc;
            --bg-secondary: #ffffff;
            --text-primary: #0f172a;
            --text-secondary: #475569;
            --text-muted: #94a3b8;
            --accent: #3b82f6;
            --accent-hover: #2563eb;
            --border: #e2e8f0;
            --shadow-sm: 0 1px 2px 0 rgb(0 0 0 / 0.05);
            --shadow: 0 4px 6px -1px rgb(0 0 0 / 0.1), 0 2px 4px -2px rgb(0 0 0 / 0.1);
            --shadow-lg: 0 10px 15px -3px rgb(0 0 0 / 0.1), 0 4px 6px -4px rgb(0 0 0 / 0.1);
            --radius: 12px;
        }}

        @media (prefers-color-scheme: dark) {{
            :root {{
                --bg-primary: #0f172a;
                --bg-secondary: #1e293b;
                --text-primary: #f8fafc;
                --text-secondary: #cbd5e1;
                --text-muted: #64748b;
                --accent: #60a5fa;
                --accent-hover: #3b82f6;
                --border: #334155;
                --shadow-sm: 0 1px 2px 0 rgb(0 0 0 / 0.3);
                --shadow: 0 4px 6px -1px rgb(0 0 0 / 0.3), 0 2px 4px -2px rgb(0 0 0 / 0.3);
                --shadow-lg: 0 10px 15px -3px rgb(0 0 0 / 0.3), 0 4px 6px -4px rgb(0 0 0 / 0.3);
            }}
        }}
        
        * {{
            box-sizing: border-box;
            margin: 0;
            padding: 0;
        }}
        
        body {{
            font-family: 'Outfit', -apple-system, BlinkMacSystemFont, sans-serif;
            background: var(--bg-primary);
            color: var(--text-primary);
            line-height: 1.6;
            min-height: 100vh;
            transition: background-color 0.3s, color 0.3s;
        }}
        
        .container {{
            max-width: 1000px;
            margin: 0 auto;
            padding: 0 24px;
        }}
        
        header {{
            background: rgba(255, 255, 255, 0.8);
            backdrop-filter: blur(12px);
            -webkit-backdrop-filter: blur(12px);
            border-bottom: 1px solid var(--border);
            padding: 16px 0;
            position: sticky;
            top: 0;
            z-index: 100;
            transition: background-color 0.3s, border-color 0.3s;
        }}

        @media (prefers-color-scheme: dark) {{
            header {{
                background: rgba(30, 41, 59, 0.8);
            }}
        }}
        
        .header-inner {{
            display: flex;
            align-items: center;
            justify-content: space-between;
            gap: 24px;
        }}
        
        .logo {{
            font-family: 'Outfit', sans-serif;
            font-size: 1.5rem;
            font-weight: 700;
            color: var(--text-primary);
            text-decoration: none;
            display: flex;
            align-items: center;
            gap: 8px;
            letter-spacing: -0.02em;
        }}
        
        .logo span {{
            color: var(--accent);
        }}
        
        .search-form {{
            flex: 1;
            max-width: 400px;
        }}
        
        .search-input {{
            width: 100%;
            padding: 12px 20px;
            border: 1px solid var(--border);
            border-radius: 99px;
            font-size: 1rem;
            background: var(--bg-secondary);
            color: var(--text-primary);
            transition: all 0.2s;
            font-family: 'Outfit', sans-serif;
        }}
        
        .search-input:focus {{
            outline: none;
            border-color: var(--accent);
            box-shadow: 0 0 0 3px rgba(59, 130, 246, 0.2);
        }}
        
        nav {{
            display: flex;
            gap: 8px;
        }}
        
        nav a {{
            color: var(--text-secondary);
            text-decoration: none;
            font-weight: 500;
            padding: 8px 16px;
            border-radius: 99px;
            transition: all 0.2s;
        }}
        
        nav a:hover {{
            background: var(--bg-secondary);
            color: var(--accent);
        }}
        
        main {{
            padding: 40px 0;
        }}
        
        .article {{
            background: var(--bg-secondary);
            border-radius: var(--radius);
            box-shadow: var(--shadow);
            padding: 48px;
            border: 1px solid var(--border);
        }}
        
        .article h1 {{
            font-family: 'Outfit', sans-serif;
            font-size: 3rem;
            font-weight: 700;
            margin-bottom: 16px;
            line-height: 1.1;
            letter-spacing: -0.03em;
            background: linear-gradient(to right, var(--text-primary), var(--text-secondary));
            -webkit-background-clip: text;
            -webkit-text-fill-color: transparent;
        }}
        
        .article-meta {{
            color: var(--text-muted);
            font-size: 0.95rem;
            margin-bottom: 32px;
            padding-bottom: 32px;
            border-bottom: 1px solid var(--border);
            display: flex;
            gap: 16px;
            align-items: center;
        }}
        
        .article-content {{
            font-family: 'Crimson Pro', serif;
            font-size: 1.25rem;
            line-height: 1.8;
            color: var(--text-primary);
            max-width: 70ch;
            margin-left: auto;
            margin-right: auto;
        }}
        
        .article-content p {{
            margin-bottom: 1.5em;
        }}
        
        .categories {{
            display: flex;
            flex-wrap: wrap;
            gap: 8px;
            margin-top: 40px;
            padding-top: 32px;
            border-top: 1px solid var(--border);
        }}
        
        .category {{
            background: var(--bg-primary);
            color: var(--text-secondary);
            padding: 6px 16px;
            border-radius: 99px;
            font-size: 0.85rem;
            font-weight: 500;
            border: 1px solid var(--border);
            transition: all 0.2s;
        }}

        .category:hover {{
            border-color: var(--accent);
            color: var(--accent);
        }}
        
        .article-list {{
            list-style: none;
            display: grid;
            gap: 16px;
        }}
        
        .article-list li {{
            background: var(--bg-secondary);
            border-radius: var(--radius);
            border: 1px solid var(--border);
            transition: all 0.2s;
        }}

        .article-list li:hover {{
            transform: translateY(-2px);
            box-shadow: var(--shadow);
            border-color: var(--accent);
        }}
        
        .article-list a {{
            display: block;
            padding: 24px;
            color: var(--text-primary);
            text-decoration: none;
        }}
        
        .article-list .title {{
            font-family: 'Outfit', sans-serif;
            font-size: 1.25rem;
            font-weight: 600;
            margin-bottom: 8px;
            color: var(--accent);
        }}
        
        .article-list .preview {{
            color: var(--text-secondary);
            font-size: 0.95rem;
            line-height: 1.5;
        }}
        
        .search-results-count {{
            color: var(--text-muted);
            margin-bottom: 24px;
            font-size: 1.1rem;
        }}
        
        .pagination {{
            display: flex;
            justify-content: center;
            gap: 8px;
            margin-top: 40px;
        }}
        
        .pagination a, .pagination span {{
            padding: 10px 20px;
            border-radius: var(--radius);
            text-decoration: none;
            color: var(--text-secondary);
            background: var(--bg-secondary);
            border: 1px solid var(--border);
            font-weight: 500;
            transition: all 0.2s;
        }}
        
        .pagination a:hover {{
            border-color: var(--accent);
            color: var(--accent);
            transform: translateY(-1px);
        }}
        
        .pagination .current {{
            background: var(--accent);
            color: white;
            border-color: var(--accent);
        }}
        
        .stats {{
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
            gap: 24px;
            margin-bottom: 48px;
        }}
        
        .stat-card {{
            background: var(--bg-secondary);
            padding: 32px;
            border-radius: var(--radius);
            border: 1px solid var(--border);
            text-align: center;
            transition: all 0.2s;
        }}

        .stat-card:hover {{
            transform: translateY(-4px);
            box-shadow: var(--shadow);
        }}
        
        .stat-value {{
            font-size: 2.5rem;
            font-weight: 700;
            color: var(--accent);
            margin-bottom: 8px;
            font-family: 'Outfit', sans-serif;
        }}
        
        .stat-label {{
            color: var(--text-muted);
            font-size: 0.9rem;
            font-weight: 500;
            text-transform: uppercase;
            letter-spacing: 0.05em;
        }}
        
        .hero {{
            text-align: center;
            padding: 80px 0;
        }}
        
        .hero h1 {{
            font-family: 'Outfit', sans-serif;
            font-size: 4rem;
            font-weight: 700;
            margin-bottom: 24px;
            letter-spacing: -0.03em;
            background: linear-gradient(135deg, var(--text-primary) 0%, var(--text-muted) 100%);
            -webkit-background-clip: text;
            -webkit-text-fill-color: transparent;
        }}
        
        .hero p {{
            color: var(--text-secondary);
            font-size: 1.5rem;
            margin-bottom: 48px;
            max-width: 600px;
            margin-left: auto;
            margin-right: auto;
        }}
        
        .hero-search {{
            max-width: 600px;
            margin: 0 auto;
            position: relative;
        }}
        
        .hero-search input {{
            padding: 20px 32px;
            font-size: 1.25rem;
            border-radius: 99px;
            box-shadow: var(--shadow-lg);
            border: 2px solid transparent;
        }}

        .hero-search input:focus {{
            border-color: var(--accent);
            transform: scale(1.02);
        }}
        
        footer {{
            text-align: center;
            padding: 48px 0;
            color: var(--text-muted);
            font-size: 0.9rem;
            border-top: 1px solid var(--border);
            margin-top: 48px;
        }}
        
        @media (max-width: 768px) {{
            .header-inner {{
                flex-wrap: wrap;
            }}
            
            .search-form {{
                order: 3;
                max-width: 100%;
                width: 100%;
                margin-top: 16px;
            }}
            
            .article {{
                padding: 24px;
            }}
            
            .article h1 {{
                font-size: 2rem;
            }}
            
            .hero h1 {{
                font-size: 2.5rem;
            }}

            .hero p {{
                font-size: 1.1rem;
            }}
        }}
    </style>
</head>
<body>
    <header>
        <div class="container header-inner">
            <a href="/" class="logo">
                <img src="/logo" alt="Logo" style="height: 32px; width: auto;">
                <span>Rustipedia</span>
            </a>
            <form action="/search" method="GET" class="search-form">
                <input type="search" name="q" placeholder="Search articles..." class="search-input">
            </form>
            <nav>
                <a href="/browse">Browse</a>
                <a href="/random">Random</a>
                <a href="/settings">Settings</a>
            </nav>
        </div>
    </header>
    
    <main class="container">
        {}
    </main>
    
    <footer class="container">
        <p>Rustipedia • {} articles • Powered by rustipedia-download</p>
    </footer>
</body>
</html>"#, title, content, state.article_count)
}

// ============================================================================
// Route Handlers
// ============================================================================

async fn home(State(state): State<SharedState>) -> impl IntoResponse {
    let state = state.read().await;
    
    let lang = WikiLanguage::from_code(&state.language)
        .map(|l| l.display_name())
        .unwrap_or("Wikipedia");
    
    let content = format!(r#"
        <div class="hero">
            <h1>📚 Your Local {}</h1>
            <p>Browse and search {} articles offline</p>
            <form action="/search" method="GET" class="hero-search">
                <input type="search" name="q" placeholder="Search for any article..." class="search-input" autofocus>
            </form>
        </div>
        
        <div class="stats">
            <div class="stat-card">
                <div class="stat-value">{}</div>
                <div class="stat-label">Total Articles</div>
            </div>
            <div class="stat-card">
                <div class="stat-value">{}</div>
                <div class="stat-label">Language</div>
            </div>
            <div class="stat-card">
                <div class="stat-value">{}</div>
                <div class="stat-label">Search</div>
            </div>
        </div>
        
        <h2 style="margin-bottom: 16px;">Recent Articles</h2>
        <ul class="article-list">
            {}
        </ul>
    "#, 
        lang,
        state.article_count,
        format_number(state.article_count),
        lang,
        if state.search_index.is_some() { "✅ Enabled" } else { "❌ Disabled" },
        state.all_titles.iter().take(10).map(|(id, title)| {
            let preview = state.get_article_preview(*id, 150);
            format!(r#"<li><a href="/article/{}"><div class="title">{}</div><div class="preview">{}</div></a></li>"#, 
                id, html_escape(title), html_escape(&preview))
        }).collect::<Vec<_>>().join("\n")
    );
    
    Html(base_html("Home", &content, &state))
}

async fn article_by_id(
    Path(id): Path<u64>,
    State(state): State<SharedState>,
) -> Response {
    let state = state.read().await;
    
    if let Some(article) = state.get_article_by_id(id) {
        let content = render_article_html(&article);
        Html(base_html(&article.title, &content, &state)).into_response()
    } else {
        (StatusCode::NOT_FOUND, Html(base_html("Not Found", "<p>Article not found</p>", &state))).into_response()
    }
}

async fn article_by_title(
    Path(title): Path<String>,
    State(state): State<SharedState>,
) -> Response {
    let state = state.read().await;
    
    if let Some(article) = state.get_article_by_title(&title) {
        let content = render_article_html(&article);
        Html(base_html(&article.title, &content, &state)).into_response()
    } else {
        (StatusCode::NOT_FOUND, Html(base_html("Not Found", "<p>Article not found</p>", &state))).into_response()
    }
}

#[derive(serde::Deserialize)]
struct SearchQuery {
    q: String,
    #[serde(default = "default_page")]
    page: usize,
}

fn default_page() -> usize { 1 }

async fn search(
    Query(params): Query<SearchQuery>,
    State(state): State<SharedState>,
) -> impl IntoResponse {
    let state = state.read().await;
    let query = params.q.trim();
    let page = params.page.max(1);
    let per_page = 20;
    
    if query.is_empty() {
        return Html(base_html("Search", "<p>Enter a search query</p>", &state));
    }

    // Security: Validate search query length
    if query.len() > 200 {
        return Html(base_html("Search", "<p>Search query too long (max 200 characters)</p>", &state));
    }
    
    let results = if let Some(ref index) = state.search_index {
        // Use full-text search
        match index.search(query, 100) {
            Ok(results) => results.into_iter()
                .map(|r| (r.id, r.title, r.preview))
                .collect(),
            Err(_) => Vec::new(),
        }
    } else {
        // Fallback to simple title search
        let query_lower = query.to_lowercase();
        state.all_titles.iter()
            .filter(|(_, title)| title.to_lowercase().contains(&query_lower))
            .take(100)
            .filter_map(|(id, title)| {
                state.articles.get(id).map(|a| (*id, title.clone(), a.preview(150).to_string()))
            })
            .collect()
    };
    
    let total = results.len();
    let start = (page - 1) * per_page;
    let page_results: Vec<_> = results.into_iter().skip(start).take(per_page).collect();
    let total_pages = (total + per_page - 1) / per_page;
    
    let content = format!(r#"
        <h1>Search: "{}"</h1>
        <p class="search-results-count">{} results found</p>
        <ul class="article-list">
            {}
        </ul>
        {}
    "#,
        html_escape(query),
        total,
        page_results.iter().map(|(id, title, preview)| {
            format!(r#"<li><a href="/article/{}"><div class="title">{}</div><div class="preview">{}</div></a></li>"#,
                id, html_escape(title), html_escape(preview))
        }).collect::<Vec<_>>().join("\n"),
        if total_pages > 1 {
            format!(r#"<div class="pagination">{}</div>"#,
                (1..=total_pages.min(10)).map(|p| {
                    if p == page {
                        format!(r#"<span class="current">{}</span>"#, p)
                    } else {
                        format!(r#"<a href="/search?q={}&page={}">{}</a>"#, urlencoding::encode(query), p, p)
                    }
                }).collect::<Vec<_>>().join("")
            )
        } else {
            String::new()
        }
    );
    
    Html(base_html(&format!("Search: {}", query), &content, &state))
}

#[derive(serde::Deserialize)]
struct BrowseQuery {
    #[serde(default = "default_page")]
    page: usize,
    #[serde(default)]
    letter: Option<char>,
}

async fn browse(
    Query(params): Query<BrowseQuery>,
    State(state): State<SharedState>,
) -> impl IntoResponse {
    let state = state.read().await;
    let page = params.page.max(1);
    let per_page = 50;
    
    let filtered: Vec<_> = if let Some(letter) = params.letter {
        state.all_titles.iter()
            .filter(|(_, title)| title.chars().next().map(|c| c.to_ascii_uppercase()) == Some(letter.to_ascii_uppercase()))
            .cloned()
            .collect()
    } else {
        state.all_titles.clone()
    };
    
    let total = filtered.len();
    let start = (page - 1) * per_page;
    let page_titles: Vec<_> = filtered.into_iter().skip(start).take(per_page).collect();
    let total_pages = (total + per_page - 1) / per_page;
    
    // Letter navigation
    let letters: Vec<char> = ('A'..='Z').collect();
    let letter_nav = letters.iter().map(|l| {
        let class = if params.letter == Some(*l) { "current" } else { "" };
        format!(r#"<a href="/browse?letter={}" class="{}">{}</a>"#, l, class, l)
    }).collect::<Vec<_>>().join(" ");
    
    let content = format!(r#"
        <h1>Browse Articles</h1>
        <p class="search-results-count">{} articles{}</p>
        <div class="pagination" style="margin-bottom: 24px;">
            <a href="/browse">All</a> {}
        </div>
        <ul class="article-list">
            {}
        </ul>
        {}
    "#,
        total,
        params.letter.map(|l| format!(" starting with '{}'", l)).unwrap_or_default(),
        letter_nav,
        page_titles.iter().map(|(id, title)| {
            let preview = state.get_article_preview(*id, 100);
            format!(r#"<li><a href="/article/{}"><div class="title">{}</div><div class="preview">{}</div></a></li>"#,
                id, html_escape(title), html_escape(&preview))
        }).collect::<Vec<_>>().join("\n"),
        if total_pages > 1 {
            let letter_param = params.letter.map(|l| format!("&letter={}", l)).unwrap_or_default();
            format!(r#"<div class="pagination">{}</div>"#,
                (1..=total_pages.min(20)).map(|p| {
                    if p == page {
                        format!(r#"<span class="current">{}</span>"#, p)
                    } else {
                        format!(r#"<a href="/browse?page={}{}">{}</a>"#, p, letter_param, p)
                    }
                }).collect::<Vec<_>>().join("")
            )
        } else {
            String::new()
        }
    );
    
    Html(base_html("Browse", &content, &state))
}

async fn random_article(State(state): State<SharedState>) -> Response {
    let state = state.read().await;
    
    if state.all_titles.is_empty() {
        return (StatusCode::NOT_FOUND, "No articles available").into_response();
    }
    
    // Security: Use cryptographically secure RNG
    let mut rng = rand::rng();
    let idx = rng.random_range(0..state.all_titles.len());
    let (id, _) = &state.all_titles[idx];
    
    axum::response::Redirect::to(&format!("/article/{}", id)).into_response()
}

// ============================================================================
// API Endpoints
// ============================================================================

async fn api_articles(
    Query(params): Query<BrowseQuery>,
    State(state): State<SharedState>,
) -> impl IntoResponse {
    let state = state.read().await;
    let page = params.page.max(1);
    let per_page = 50;
    let start = (page - 1) * per_page;
    
    let articles: Vec<_> = state.all_titles.iter()
        .skip(start)
        .take(per_page)
        .map(|(id, title)| {
            let preview = state.get_article_preview(*id, 200);
            let word_count = if state.search_index.is_none() {
                state.articles.get(id).map(|a| a.word_count()).unwrap_or(0)
            } else {
                0 // Approximate or fetch full article if needed
            };
            
            serde_json::json!({
                "id": id,
                "title": title,
                "preview": preview,
                "word_count": word_count
            })
        })
        .collect();
    
    axum::Json(serde_json::json!({
        "articles": articles,
        "page": page,
        "total": state.article_count
    }))
}

async fn api_search(
    Query(params): Query<SearchQuery>,
    State(state): State<SharedState>,
) -> impl IntoResponse {
    let state = state.read().await;
    
    let results = if let Some(ref index) = state.search_index {
        match index.search(&params.q, 50) {
            Ok(results) => results.into_iter()
                .map(|r| {
                    serde_json::json!({
                        "id": r.id,
                        "title": r.title,
                        "preview": r.preview,
                        "score": r.score
                    })
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    } else {
        Vec::new()
    };
    
    axum::Json(serde_json::json!({
        "query": params.q,
        "results": results
    }))
}

// ============================================================================
// Utilities
// ============================================================================

/// Render an article as HTML
fn render_article_html(article: &Article) -> String {
    let categories_html = if !article.categories.is_empty() {
        format!(r#"<div class="categories">{}</div>"#,
            article.categories.iter()
                .map(|c| format!(r#"<span class="category">{}</span>"#, html_escape(c)))
                .collect::<Vec<_>>()
                .join("")
        )
    } else {
        String::new()
    };

    format!(r#"
        <article class="article">
            <h1>{}</h1>
            <div class="article-meta">
                Article ID: {} • {} words
            </div>
            <div class="article-content">
                {}
            </div>
            {}
        </article>
    "#, 
        html_escape(&article.title),
        article.id,
        article.word_count(),
        article.content.split("\n\n").map(|p| format!("<p>{}</p>", p)).collect::<Vec<_>>().join("\n"),
        categories_html
    )
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

fn format_number(n: usize) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.insert(0, ',');
        }
        result.insert(0, c);
    }
    result
}


#[derive(serde::Deserialize)]
struct SettingsForm {
    enabled: Option<String>,
    frequency: String,
    day: Option<String>,
    hour: u8,
    minute: u8,
    language: String,
}

async fn settings_page(State(state): State<SharedState>) -> impl IntoResponse {
    let state = state.read().await;
    let html = settings_html(&state);
    Html(base_html("Settings", &html, &state))
}

async fn update_settings(
    State(state): State<SharedState>,
    Form(form): Form<SettingsForm>,
) -> impl IntoResponse {
    let mut state = state.write().await;
    
    let schedule = match form.frequency.as_str() {
        "Daily" => UpdateSchedule::Daily {
            hour: form.hour,
            minute: form.minute,
        },
        "Weekly" => {
            let day = match form.day.as_deref() {
                Some("Sunday") => Weekday::Sunday,
                Some("Monday") => Weekday::Monday,
                Some("Tuesday") => Weekday::Tuesday,
                Some("Wednesday") => Weekday::Wednesday,
                Some("Thursday") => Weekday::Thursday,
                Some("Friday") => Weekday::Friday,
                Some("Saturday") => Weekday::Saturday,
                _ => Weekday::Sunday,
            };
            UpdateSchedule::Weekly {
                day,
                hour: form.hour,
                minute: form.minute,
            }
        },
        "Monthly" => UpdateSchedule::Monthly {
            day: 1, // Simplified for now
            hour: form.hour,
            minute: form.minute,
        },
        _ => UpdateSchedule::Weekly { day: Weekday::Sunday, hour: 3, minute: 0 },
    };

    state.update_config.enabled = form.enabled.is_some();
    state.update_config.schedule = schedule;
    state.update_config.language = form.language;
    
    // Save config
    if let Err(e) = state.update_config.save(UpdateConfig::config_path(&state.data_dir)) {
        tracing::error!("Failed to save update config: {}", e);
    }

    // Redirect back to settings
    (StatusCode::SEE_OTHER, [("Location", "/settings")])
}

async fn api_update_status(State(state): State<SharedState>) -> impl IntoResponse {
    let state = state.read().await;
    let manager = UpdateManager::load(&state.data_dir).unwrap_or_else(|_| {
        UpdateManager::new(UpdateConfig::default())
    });
    let status = manager.get_status().await;
    Json(status)
}

async fn api_trigger_update(State(state): State<SharedState>) -> impl IntoResponse {
    let state = state.read().await;
    
    let data_dir = state.data_dir.clone();
    
    tokio::spawn(async move {
        let manager = UpdateManager::load(&data_dir).unwrap_or_else(|_| {
            UpdateManager::new(UpdateConfig::default())
        });
        let _ = manager.perform_update().await;
    });

    Json(serde_json::json!({ "status": "started" }))
}

async fn api_update_history(State(state): State<SharedState>) -> impl IntoResponse {
    let state = state.read().await;
    let manager = UpdateManager::load(&state.data_dir).unwrap_or_else(|_| {
        UpdateManager::new(UpdateConfig::default())
    });
    
    let history = manager.get_history(50).await.unwrap_or_default();
    Json(history)
}

async fn logo_handler(State(state): State<SharedState>) -> impl IntoResponse {
    let state = state.read().await;
    let custom_logo_path = state.data_dir.join("custom_logo.png");
    
    if custom_logo_path.exists() {
        match fs::read(&custom_logo_path) {
            Ok(bytes) => return (
                [(header::CONTENT_TYPE, "image/png")],
                bytes
            ).into_response(),
            Err(e) => tracing::error!("Failed to read custom logo: {}", e),
        }
    }
    
    (
        [(header::CONTENT_TYPE, "image/png")],
        DEFAULT_LOGO.to_vec()
    ).into_response()
}

async fn upload_logo(
    State(state): State<SharedState>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    while let Some(field) = multipart.next_field().await.unwrap_or(None) {
        let name = field.name().unwrap_or("").to_string();
        if name == "logo" {
            let data = match field.bytes().await {
                Ok(data) => data,
                Err(e) => return (StatusCode::BAD_REQUEST, format!("Failed to read upload: {}", e)).into_response(),
            };
            
            if data.is_empty() {
                continue;
            }

            let state = state.read().await;
            let custom_logo_path = state.data_dir.join("custom_logo.png");
            
            if let Err(e) = fs::write(&custom_logo_path, data) {
                return (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to save logo: {}", e)).into_response();
            }
            
            return (StatusCode::SEE_OTHER, [("Location", "/settings")]).into_response();
        }
    }
    
    (StatusCode::BAD_REQUEST, "No logo file provided").into_response()
}

fn settings_html(state: &AppState) -> String {
    let config = &state.update_config;
    
    let freq_daily = matches!(config.schedule, UpdateSchedule::Daily { .. });
    let freq_weekly = matches!(config.schedule, UpdateSchedule::Weekly { .. });
    let freq_monthly = matches!(config.schedule, UpdateSchedule::Monthly { .. });
    
    let (hour, minute, day_str) = match &config.schedule {
        UpdateSchedule::Daily { hour, minute } => (*hour, *minute, ""),
        UpdateSchedule::Weekly { day, hour, minute } => (*hour, *minute, match day {
            Weekday::Sunday => "Sunday",
            Weekday::Monday => "Monday",
            Weekday::Tuesday => "Tuesday",
            Weekday::Wednesday => "Wednesday",
            Weekday::Thursday => "Thursday",
            Weekday::Friday => "Friday",
            Weekday::Saturday => "Saturday",
        }),
        UpdateSchedule::Monthly { day: _, hour, minute } => (*hour, *minute, ""),
        #[allow(unreachable_patterns)]
        _ => (3, 0, ""),
    };

    format!(r#"
        <div class="article">
            <h1>⚙️ Settings</h1>
            
            <div style="margin-bottom: 48px; padding: 24px; background: var(--bg-primary); border-radius: var(--radius); border: 1px solid var(--border);">
                <h2 style="margin-bottom: 16px; font-size: 1.25rem;">Branding</h2>
                <div style="display: flex; gap: 24px; align-items: center; flex-wrap: wrap;">
                    <div style="text-align: center;">
                        <div style="margin-bottom: 8px; font-weight: 500; font-size: 0.9rem; color: var(--text-muted);">Current Logo</div>
                        <img src="/logo" alt="Current Logo" style="height: 64px; width: auto; border: 1px solid var(--border); border-radius: 8px; padding: 8px; background: white;">
                    </div>
                    <form action="/settings/logo" method="POST" enctype="multipart/form-data" style="flex: 1; min-width: 300px;">
                        <label style="display: block; margin-bottom: 8px; font-weight: 500;">Upload Custom Logo</label>
                        <div style="display: flex; gap: 12px; flex-wrap: wrap;">
                            <input type="file" name="logo" accept="image/png,image/jpeg" class="search-input" style="padding: 8px; flex: 1;">
                            <button type="submit" style="background: var(--accent); color: white; border: none; padding: 12px 24px; border-radius: 99px; font-size: 0.95rem; font-weight: 600; cursor: pointer;">Upload</button>
                        </div>
                        <p style="margin-top: 8px; font-size: 0.85rem; color: var(--text-muted);">Recommended: PNG or JPG, square aspect ratio.</p>
                    </form>
                </div>
            </div>

            <form action="/settings" method="POST" style="max-width: 600px;">
                <div style="margin-bottom: 24px;">
                    <label style="display: flex; align-items: center; gap: 12px; font-size: 1.1rem; font-weight: 500;">
                        <input type="checkbox" name="enabled" {} style="width: 20px; height: 20px;">
                        Enable Automatic Updates
                    </label>
                </div>

                <div style="margin-bottom: 24px;">
                    <label style="display: block; margin-bottom: 8px; font-weight: 500;">Language Code</label>
                    <input type="text" name="language" value="{}" class="search-input" style="width: 100%;">
                </div>

                <div style="margin-bottom: 24px;">
                    <label style="display: block; margin-bottom: 8px; font-weight: 500;">Update Frequency</label>
                    <select name="frequency" class="search-input" style="width: 100%;" onchange="toggleDay(this.value)">
                        <option value="Daily" {}>Daily</option>
                        <option value="Weekly" {}>Weekly</option>
                        <option value="Monthly" {}>Monthly</option>
                    </select>
                </div>

                <div id="day-select" style="margin-bottom: 24px; display: {};">
                    <label style="display: block; margin-bottom: 8px; font-weight: 500;">Day of Week</label>
                    <select name="day" class="search-input" style="width: 100%;">
                        <option value="Sunday" {}>Sunday</option>
                        <option value="Monday" {}>Monday</option>
                        <option value="Tuesday" {}>Tuesday</option>
                        <option value="Wednesday" {}>Wednesday</option>
                        <option value="Thursday" {}>Thursday</option>
                        <option value="Friday" {}>Friday</option>
                        <option value="Saturday" {}>Saturday</option>
                    </select>
                </div>

                <div style="display: flex; gap: 16px; margin-bottom: 32px;">
                    <div style="flex: 1;">
                        <label style="display: block; margin-bottom: 8px; font-weight: 500;">Hour (0-23)</label>
                        <input type="number" name="hour" value="{}" min="0" max="23" class="search-input" style="width: 100%;">
                    </div>
                    <div style="flex: 1;">
                        <label style="display: block; margin-bottom: 8px; font-weight: 500;">Minute (0-59)</label>
                        <input type="number" name="minute" value="{}" min="0" max="59" class="search-input" style="width: 100%;">
                    </div>
                </div>

                <button type="submit" style="background: var(--accent); color: white; border: none; padding: 12px 24px; border-radius: 99px; font-size: 1rem; font-weight: 600; cursor: pointer;">
                    Save Settings
                </button>
            </form>

            <hr style="margin: 48px 0; border: none; border-top: 1px solid var(--border);">

            <h2>Update Status</h2>
            <div id="update-status" style="margin-top: 16px; padding: 24px; background: var(--bg-primary); border-radius: var(--radius); border: 1px solid var(--border);">
                Loading status...
            </div>
            
            <button onclick="triggerUpdate()" style="margin-top: 16px; background: var(--bg-secondary); color: var(--text-primary); border: 1px solid var(--border); padding: 12px 24px; border-radius: 99px; font-size: 1rem; font-weight: 600; cursor: pointer;">
                Check for Updates Now
            </button>

            <hr style="margin: 48px 0; border: none; border-top: 1px solid var(--border);">

            <h2>Update History</h2>
            <div id="update-history" style="margin-top: 16px; padding: 24px; background: var(--bg-primary); border-radius: var(--radius); border: 1px solid var(--border); max-height: 300px; overflow-y: auto; font-family: monospace; font-size: 0.9rem;">
                Loading history...
            </div>

            <script>
                function toggleDay(freq) {{
                    const daySelect = document.getElementById('day-select');
                    daySelect.style.display = freq === 'Weekly' ? 'block' : 'none';
                }}

                async function loadStatus() {{
                    const res = await fetch('/api/update/status');
                    const status = await res.json();
                    const el = document.getElementById('update-status');
                    
                    let html = `
                        <div style="display: grid; gap: 8px;">
                            <div><strong>Status:</strong> ${{status.current_status}}</div>
                            <div><strong>Last Check:</strong> ${{status.last_check || 'Never'}}</div>
                            <div><strong>Last Update:</strong> ${{status.last_update || 'Never'}}</div>
                        </div>
                    `;
                    
                    if (status.error_message) {{
                        html += `<div style="color: #ef4444; margin-top: 8px;">Error: ${{status.error_message}}</div>`;
                    }}
                    
                    if (status.progress > 0 && status.progress < 100) {{
                        html += `
                            <div style="margin-top: 12px; height: 8px; background: var(--border); border-radius: 4px; overflow: hidden;">
                                <div style="height: 100%; width: ${{status.progress}}%; background: var(--accent);"></div>
                            </div>
                            <div style="text-align: right; font-size: 0.9rem; margin-top: 4px;">${{status.progress.toFixed(1)}}%</div>
                        `;
                    }}
                    
                    el.innerHTML = html;
                }}

                async function loadHistory() {{
                    try {{
                        const res = await fetch('/api/update/history');
                        const history = await res.json();
                        const el = document.getElementById('update-history');
                        
                        if (history.length === 0) {{
                            el.innerHTML = '<div style="color: var(--text-muted);">No update history found.</div>';
                            return;
                        }}
                        
                        el.innerHTML = history.map(line => `<div>${{line}}</div>`).join('');
                    }} catch (e) {{
                        console.error('Failed to load history:', e);
                    }}
                }}

                async function triggerUpdate() {{
                    if (!confirm('Are you sure you want to start an update check?')) return;
                    
                    try {{
                        const res = await fetch('/api/update/trigger', {{ method: 'POST' }});
                        const data = await res.json();
                        alert('Update started!');
                        loadStatus();
                    }} catch (e) {{
                        alert('Failed to trigger update: ' + e);
                    }}
                }}

                // Initial load
                loadStatus();
                loadHistory();
                // Poll every 5 seconds
                setInterval(loadStatus, 5000);
            </script>
        </div>
    "#,
        if config.enabled { "checked" } else { "" },
        config.language,
        if freq_daily { "selected" } else { "" },
        if freq_weekly { "selected" } else { "" },
        if freq_monthly { "selected" } else { "" },
        if freq_weekly { "block" } else { "none" },
        if day_str == "Sunday" { "selected" } else { "" },
        if day_str == "Monday" { "selected" } else { "" },
        if day_str == "Tuesday" { "selected" } else { "" },
        if day_str == "Wednesday" { "selected" } else { "" },
        if day_str == "Thursday" { "selected" } else { "" },
        if day_str == "Friday" { "selected" } else { "" },
        if day_str == "Saturday" { "selected" } else { "" },
        hour,
        minute
    )
}
