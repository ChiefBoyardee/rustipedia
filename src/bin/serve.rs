//! Wikipedia Web Server
//!
//! Serve your downloaded Wikipedia locally with a beautiful, searchable interface.
//!
//! # Examples
//!
//! Serve with default settings:
//! ```bash
//! wiki-serve
//! ```
//!
//! Specify data directory and port:
//! ```bash
//! wiki-serve --data ./my-wiki --port 3000
//! ```

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
use clap::Parser;
use tokio::sync::RwLock;
use tracing_subscriber::EnvFilter;

use wiki_download::{Article, SearchIndex, WikiLanguage};

#[derive(Parser)]
#[command(name = "wiki-serve")]
#[command(author, version, about = "Serve your local Wikipedia")]
#[command(long_about = r#"
╔══════════════════════════════════════════════════════════════════╗
║                     🌐 WIKI SERVE                                 ║
║            Serve Your Local Wikipedia Collection                 ║
╚══════════════════════════════════════════════════════════════════╝

Serve your downloaded Wikipedia with a beautiful, searchable interface.
Browse articles, search by title or content, and explore offline!

EXAMPLES:
  Start server with defaults:
    wiki-serve

  Use custom data directory:
    wiki-serve --data ./my-wiki

  Use custom port:
    wiki-serve --port 3000

  Bind to all interfaces (for network access):
    wiki-serve --host 0.0.0.0
"#)]
struct Cli {
    /// Directory containing Wikipedia data
    #[arg(short, long, default_value = "wikipedia")]
    data: PathBuf,

    /// Port to listen on
    #[arg(short, long, default_value = "8080")]
    port: u16,

    /// Host to bind to
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

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
}

impl AppState {
    fn load(data_dir: &PathBuf) -> Result<Self> {
        let articles_path = data_dir.join("articles.jsonl");
        if !articles_path.exists() {
            anyhow::bail!("Articles file not found: {:?}. Run wiki-download first.", articles_path);
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
            tracing::warn!("No search index found. Search disabled. Run: wiki-download index {:?}", data_dir);
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

        Ok(Self {
            articles,
            by_title,
            search_index,
            all_titles,
            language,
            article_count,
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

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let filter = if cli.verbose {
        EnvFilter::new("wiki_serve=debug,tower_http=debug,info")
    } else {
        EnvFilter::new("wiki_serve=info,warn")
    };
    
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();

    // Load data
    let state = AppState::load(&cli.data)?;
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
        .with_state(shared_state);

    let addr = format!("{}:{}", cli.host, cli.port);
    
    println!();
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║                     🌐 WIKI SERVE                                 ║");
    println!("╠══════════════════════════════════════════════════════════════════╣");
    println!("║  Server running at: http://{}                          ", addr);
    println!("║  Data directory:    {:?}                                ", cli.data);
    println!("╚══════════════════════════════════════════════════════════════════╝");
    println!();
    println!("Press Ctrl+C to stop the server");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

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
    <title>{} - Local Wikipedia</title>
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
            <a href="/" class="logo">📚 <span>Local Wiki</span></a>
            <form action="/search" method="GET" class="search-form">
                <input type="search" name="q" placeholder="Search articles..." class="search-input">
            </form>
            <nav>
                <a href="/browse">Browse</a>
                <a href="/random">Random</a>
            </nav>
        </div>
    </header>
    
    <main class="container">
        {}
    </main>
    
    <footer class="container">
        <p>Local Wikipedia Server • {} articles • Powered by wiki-download</p>
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
    
    use std::time::{SystemTime, UNIX_EPOCH};
    let seed = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() as usize;
    let idx = seed % state.all_titles.len();
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

