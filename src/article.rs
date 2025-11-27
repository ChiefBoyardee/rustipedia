//! Article data structures

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// A Wikipedia article
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Article {
    /// Article ID (unique within the Wikipedia dump)
    pub id: u64,
    /// Article title
    pub title: String,
    /// Plain text content (wiki markup removed)
    pub content: String,
    /// Original wiki markup (optional, for debugging)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_markup: Option<String>,
    /// Article categories
    #[serde(default)]
    pub categories: Vec<String>,
    /// Redirect target if this is a redirect page
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redirect_to: Option<String>,
    /// Extraction timestamp
    #[serde(default = "Utc::now")]
    pub extracted_at: DateTime<Utc>,
}

impl Article {
    /// Create a new article
    pub fn new(id: u64, title: String, content: String) -> Self {
        Self {
            id,
            title,
            content,
            raw_markup: None,
            categories: Vec::new(),
            redirect_to: None,
            extracted_at: Utc::now(),
        }
    }

    /// Check if this is a redirect page
    pub fn is_redirect(&self) -> bool {
        self.redirect_to.is_some()
    }

    /// Get article length in characters
    pub fn length(&self) -> usize {
        self.content.len()
    }

    /// Get estimated word count
    pub fn word_count(&self) -> usize {
        self.content.split_whitespace().count()
    }

    /// Get a preview/summary of the article (first N characters)
    pub fn preview(&self, max_chars: usize) -> &str {
        if self.content.len() <= max_chars {
            &self.content
        } else {
            // Find a good break point (end of word)
            let mut end = max_chars;
            while end > 0 && !self.content.is_char_boundary(end) {
                end -= 1;
            }
            // Try to end at a space
            if let Some(space_pos) = self.content[..end].rfind(' ') {
                &self.content[..space_pos]
            } else {
                &self.content[..end]
            }
        }
    }
}

/// Statistics about extracted articles
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExtractionStats {
    /// Total articles extracted
    pub articles_extracted: u64,
    /// Articles skipped (too short, redirects, special pages)
    pub articles_skipped: u64,
    /// Redirect pages encountered
    pub redirects: u64,
    /// Special pages skipped
    pub special_pages: u64,
    /// Total bytes of content
    pub total_bytes: u64,
    /// Minimum article length requirement
    pub min_length: usize,
    /// Source Wikipedia edition
    pub language: String,
    /// Dump filename
    pub source_file: String,
    /// Extraction start time
    pub started_at: DateTime<Utc>,
    /// Extraction end time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,
    /// Duration in seconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_secs: Option<f64>,
}

impl ExtractionStats {
    pub fn new(language: &str, source_file: &str, min_length: usize) -> Self {
        Self {
            language: language.to_string(),
            source_file: source_file.to_string(),
            min_length,
            started_at: Utc::now(),
            ..Default::default()
        }
    }

    pub fn finish(&mut self) {
        let now = Utc::now();
        self.duration_secs = Some((now - self.started_at).num_milliseconds() as f64 / 1000.0);
        self.completed_at = Some(now);
    }

    /// Articles processed per second
    pub fn articles_per_second(&self) -> f64 {
        if let Some(duration) = self.duration_secs {
            if duration > 0.0 {
                return self.articles_extracted as f64 / duration;
            }
        }
        0.0
    }
}

