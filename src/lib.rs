//! # Wiki Download
//!
//! Download and serve your own local copy of Wikipedia.
//!
//! This crate provides tools to:
//! - Download Wikipedia dumps directly from Wikimedia
//! - Parse and extract articles from the XML dump
//! - Store articles in a searchable format
//! - Serve articles via a local web server
//!
//! ## Quick Start
//!
//! ```bash
//! # Download Simple English Wikipedia (~300MB, ~200K articles)
//! wiki-download --lang simple
//!
//! # Serve the downloaded Wikipedia
//! wiki-serve --port 8080
//! ```

pub mod article;
pub mod downloader;
pub mod parser;
pub mod search;
pub mod config;

pub use article::Article;
pub use downloader::WikiDownloader;
pub use parser::WikiParser;
pub use search::SearchIndex;
pub use config::Config;

/// Supported Wikipedia languages/editions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WikiLanguage {
    /// Simple English Wikipedia (~200K articles, ~500M tokens, ~300MB dump)
    #[default]
    Simple,
    /// Full English Wikipedia (~6.7M articles, ~20B tokens, ~22GB dump)
    English,
    /// German Wikipedia (~2.8M articles)
    German,
    /// French Wikipedia (~2.5M articles)
    French,
    /// Spanish Wikipedia (~1.9M articles)
    Spanish,
    /// Japanese Wikipedia (~1.4M articles)
    Japanese,
    /// Russian Wikipedia (~1.9M articles)
    Russian,
    /// Chinese Wikipedia (~1.3M articles)
    Chinese,
    /// Italian Wikipedia (~1.8M articles)
    Italian,
    /// Portuguese Wikipedia (~1.1M articles)
    Portuguese,
}

impl WikiLanguage {
    /// Get the Wikipedia language code
    pub fn code(&self) -> &'static str {
        match self {
            WikiLanguage::Simple => "simple",
            WikiLanguage::English => "en",
            WikiLanguage::German => "de",
            WikiLanguage::French => "fr",
            WikiLanguage::Spanish => "es",
            WikiLanguage::Japanese => "ja",
            WikiLanguage::Russian => "ru",
            WikiLanguage::Chinese => "zh",
            WikiLanguage::Italian => "it",
            WikiLanguage::Portuguese => "pt",
        }
    }

    /// Get the dump URL
    pub fn dump_url(&self) -> String {
        let code = self.code();
        format!(
            "https://dumps.wikimedia.org/{}wiki/latest/{}wiki-latest-pages-articles.xml.bz2",
            code, code
        )
    }

    /// Get the estimated dump size (human readable)
    pub fn estimated_size(&self) -> &'static str {
        match self {
            WikiLanguage::Simple => "~300 MB",
            WikiLanguage::English => "~22 GB",
            WikiLanguage::German => "~7 GB",
            WikiLanguage::French => "~5 GB",
            WikiLanguage::Spanish => "~4 GB",
            WikiLanguage::Japanese => "~4 GB",
            WikiLanguage::Russian => "~5 GB",
            WikiLanguage::Chinese => "~3 GB",
            WikiLanguage::Italian => "~4 GB",
            WikiLanguage::Portuguese => "~2 GB",
        }
    }

    /// Get estimated article count
    pub fn estimated_articles(&self) -> &'static str {
        match self {
            WikiLanguage::Simple => "~200K",
            WikiLanguage::English => "~6.7M",
            WikiLanguage::German => "~2.8M",
            WikiLanguage::French => "~2.5M",
            WikiLanguage::Spanish => "~1.9M",
            WikiLanguage::Japanese => "~1.4M",
            WikiLanguage::Russian => "~1.9M",
            WikiLanguage::Chinese => "~1.3M",
            WikiLanguage::Italian => "~1.8M",
            WikiLanguage::Portuguese => "~1.1M",
        }
    }

    /// Get display name
    pub fn display_name(&self) -> &'static str {
        match self {
            WikiLanguage::Simple => "Simple English",
            WikiLanguage::English => "English",
            WikiLanguage::German => "German (Deutsch)",
            WikiLanguage::French => "French (Français)",
            WikiLanguage::Spanish => "Spanish (Español)",
            WikiLanguage::Japanese => "Japanese (日本語)",
            WikiLanguage::Russian => "Russian (Русский)",
            WikiLanguage::Chinese => "Chinese (中文)",
            WikiLanguage::Italian => "Italian (Italiano)",
            WikiLanguage::Portuguese => "Portuguese (Português)",
        }
    }

    /// Parse from string
    pub fn from_code(code: &str) -> Option<WikiLanguage> {
        match code.to_lowercase().as_str() {
            "simple" => Some(WikiLanguage::Simple),
            "en" | "english" => Some(WikiLanguage::English),
            "de" | "german" | "deutsch" => Some(WikiLanguage::German),
            "fr" | "french" | "français" => Some(WikiLanguage::French),
            "es" | "spanish" | "español" => Some(WikiLanguage::Spanish),
            "ja" | "japanese" | "日本語" => Some(WikiLanguage::Japanese),
            "ru" | "russian" | "русский" => Some(WikiLanguage::Russian),
            "zh" | "chinese" | "中文" => Some(WikiLanguage::Chinese),
            "it" | "italian" | "italiano" => Some(WikiLanguage::Italian),
            "pt" | "portuguese" | "português" => Some(WikiLanguage::Portuguese),
            _ => None,
        }
    }

    /// Get all available languages
    pub fn all() -> &'static [WikiLanguage] {
        &[
            WikiLanguage::Simple,
            WikiLanguage::English,
            WikiLanguage::German,
            WikiLanguage::French,
            WikiLanguage::Spanish,
            WikiLanguage::Japanese,
            WikiLanguage::Russian,
            WikiLanguage::Chinese,
            WikiLanguage::Italian,
            WikiLanguage::Portuguese,
        ]
    }
}

impl std::fmt::Display for WikiLanguage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

impl std::str::FromStr for WikiLanguage {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        WikiLanguage::from_code(s)
            .ok_or_else(|| format!("Unknown language: {}. Use one of: simple, en, de, fr, es, ja, ru, zh, it, pt", s))
    }
}

