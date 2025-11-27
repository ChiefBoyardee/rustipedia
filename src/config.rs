//! Rustipedia Config

use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use crate::WikiLanguage;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Wikipedia language to download
    pub language: String,
    /// Output directory for downloaded data
    pub output_dir: PathBuf,
    /// Maximum articles to extract (0 = unlimited)
    pub max_articles: usize,
    /// Minimum article length in characters
    pub min_length: usize,
    /// Skip download if dump already exists
    pub skip_download: bool,
    /// Build search index after extraction
    pub build_index: bool,
    /// Keep the raw bz2 dump file after extraction
    pub keep_dump: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            language: "simple".to_string(),
            output_dir: PathBuf::from("wikipedia"),
            max_articles: 0,
            min_length: 200,
            skip_download: false,
            build_index: true,
            keep_dump: false,
        }
    }
}

impl Config {
    /// Create a new config with defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the language
    pub fn with_language(mut self, lang: WikiLanguage) -> Self {
        self.language = lang.code().to_string();
        self
    }

    /// Set the output directory
    pub fn with_output_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.output_dir = path.into();
        self
    }

    /// Set max articles
    pub fn with_max_articles(mut self, max: usize) -> Self {
        self.max_articles = max;
        self
    }

    /// Set minimum article length
    pub fn with_min_length(mut self, min: usize) -> Self {
        self.min_length = min;
        self
    }

    /// Get the wiki language enum
    pub fn wiki_language(&self) -> WikiLanguage {
        WikiLanguage::from_code(&self.language).unwrap_or_default()
    }

    /// Get the path to the dump file
    pub fn dump_path(&self) -> PathBuf {
        let lang = self.wiki_language();
        self.output_dir.join(format!("{}wiki-latest-pages-articles.xml.bz2", lang.code()))
    }

    /// Get the path to the articles directory
    pub fn articles_dir(&self) -> PathBuf {
        self.output_dir.join("articles")
    }

    /// Get the path to the JSONL data file
    pub fn data_path(&self) -> PathBuf {
        self.output_dir.join("articles.jsonl")
    }

    /// Get the path to the search index
    pub fn index_path(&self) -> PathBuf {
        self.output_dir.join("search_index")
    }

    /// Get the path to the stats file
    pub fn stats_path(&self) -> PathBuf {
        self.output_dir.join("stats.json")
    }

    /// Get the path to config file
    pub fn config_path(&self) -> PathBuf {
        self.output_dir.join("config.json")
    }

    /// Save config to file
    pub fn save(&self) -> anyhow::Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(self.config_path(), content)?;
        Ok(())
    }

    /// Load config from file
    pub fn load(path: impl Into<PathBuf>) -> anyhow::Result<Self> {
        let path = path.into();
        let content = std::fs::read_to_string(path)?;
        let config: Config = serde_json::from_str(&content)?;
        Ok(config)
    }
}

/// Configuration for the web server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Port to listen on
    pub port: u16,
    /// Host to bind to
    pub host: String,
    /// Path to Wikipedia data directory
    pub data_dir: PathBuf,
    /// Enable search functionality
    pub enable_search: bool,
    /// Maximum search results
    pub max_search_results: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: 8080,
            host: "127.0.0.1".to_string(),
            data_dir: PathBuf::from("wikipedia"),
            enable_search: true,
            max_search_results: 50,
        }
    }
}

impl ServerConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    pub fn with_host(mut self, host: impl Into<String>) -> Self {
        self.host = host.into();
        self
    }

    pub fn with_data_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.data_dir = path.into();
        self
    }

    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

