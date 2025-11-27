//! Full-text search index for Wikipedia articles

use std::path::Path;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};

use regex::Regex;
use once_cell::sync::Lazy;
use anyhow::{Context, Result};
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::*;
use tantivy::{Index, IndexWriter, ReloadPolicy, TantivyDocument};

use crate::article::Article;

static HTML_TAG_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"<[^>]+>").unwrap());

/// Search result
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Article ID
    pub id: u64,
    /// Article title
    pub title: String,
    /// Preview/snippet of the content
    pub preview: String,
    /// Search score
    pub score: f32,
}

/// Full-text search index for Wikipedia articles
pub struct SearchIndex {
    index: Index,
    query_parser: QueryParser,
    title_field: Field,
    content_field: Field,
    raw_content_field: Field,
    categories_field: Field,
    id_field: Field,
}

impl SearchIndex {
    /// Create a new search index in the given directory
    pub fn create(index_path: impl AsRef<Path>) -> Result<Self> {
        let index_path = index_path.as_ref();
        
        // Create directory if needed
        if !index_path.exists() {
            fs::create_dir_all(index_path)?;
        }

        // Build schema
        let mut schema_builder = Schema::builder();
        let id_field = schema_builder.add_u64_field("id", STORED | INDEXED);
        let title_field = schema_builder.add_text_field("title", TEXT | STORED);
        let content_field = schema_builder.add_text_field("content", TEXT);
        let raw_content_field = schema_builder.add_text_field("raw_content", STORED);
        let categories_field = schema_builder.add_text_field("categories", TEXT | STORED);
        let schema = schema_builder.build();

        // Create index
        let index = Index::create_in_dir(index_path, schema.clone())
            .context("Failed to create search index")?;

        let mut query_parser = QueryParser::for_index(&index, vec![title_field, content_field]);
        query_parser.set_field_boost(title_field, 5.0);
        query_parser.set_conjunction_by_default();

        Ok(Self {
            index,
            query_parser,
            title_field,
            content_field,
            raw_content_field,
            categories_field,
            id_field,
        })
    }

    /// Open an existing search index
    pub fn open(index_path: impl AsRef<Path>) -> Result<Self> {
        let index_path = index_path.as_ref();
        
        let index = Index::open_in_dir(index_path)
            .context("Failed to open search index")?;

        let schema = index.schema();
        let id_field = schema.get_field("id").context("Missing id field")?;
        let title_field = schema.get_field("title").context("Missing title field")?;
        let content_field = schema.get_field("content").context("Missing content field")?;
        let raw_content_field = schema.get_field("raw_content").context("Missing raw_content field")?;
        let categories_field = schema.get_field("categories").context("Missing categories field")?;

        let mut query_parser = QueryParser::for_index(&index, vec![title_field, content_field]);
        query_parser.set_field_boost(title_field, 5.0);
        query_parser.set_conjunction_by_default();

        Ok(Self {
            index,
            query_parser,
            title_field,
            content_field,
            raw_content_field,
            categories_field,
            id_field,
        })
    }

    /// Build index from JSONL file
    pub fn build_from_jsonl(&self, jsonl_path: impl AsRef<Path>) -> Result<u64> {
        use indicatif::{ProgressBar, ProgressStyle};
        
        let file = File::open(jsonl_path.as_ref())?;
        let file_size = file.metadata()?.len();
        let reader = BufReader::new(file);

        // Create progress bar
        let pb = ProgressBar::new(file_size);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta}) {msg}")
                .unwrap()
                .progress_chars("#>-")
        );
        pb.set_message("Building search index...");

        let mut writer = self.index.writer(100_000_000)?; // 100MB heap
        let mut count = 0u64;
        let mut bytes_read = 0u64;

        for line in reader.lines() {
            let line = line?;
            bytes_read += line.len() as u64 + 1; // +1 for newline
            
            if line.is_empty() {
                continue;
            }

            let article: Article = serde_json::from_str(&line)
                .context("Failed to parse article JSON")?;

            self.add_article_to_writer(&mut writer, &article)?;
            count += 1;

            if count % 1000 == 0 {
                pb.set_position(bytes_read);
                pb.set_message(format!("Indexed {} articles", count));
            }

            if count % 10000 == 0 {
                writer.commit()?;
            }
        }

        writer.commit()?;
        pb.finish_with_message(format!("✓ Indexed {} articles", count));

        Ok(count)
    }

    /// Add a single article to the index
    fn add_article_to_writer(&self, writer: &mut IndexWriter, article: &Article) -> Result<()> {
        let mut doc = TantivyDocument::default();
        doc.add_u64(self.id_field, article.id);
        doc.add_text(self.title_field, &article.title);
        
        // Store original content with HTML for display
        doc.add_text(self.raw_content_field, &article.content);
        
        // Strip HTML tags for search indexing
        let content_text = HTML_TAG_RE.replace_all(&article.content, " ");
        doc.add_text(self.content_field, &content_text);
        
        for cat in &article.categories {
            doc.add_text(self.categories_field, cat);
        }

        writer.add_document(doc)?;
        Ok(())
    }

    /// Search for articles
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let reader = self.index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()?;
        let searcher = reader.searcher();

        let query = self.query_parser.parse_query(query)
            .context("Failed to parse search query")?;

        let top_docs = searcher.search(&query, &TopDocs::with_limit(limit))?;

        let mut results = Vec::new();
        for (score, doc_address) in top_docs {
            let doc: TantivyDocument = searcher.doc(doc_address)?;
            
            let id = doc.get_first(self.id_field)
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            
            let title = doc.get_first(self.title_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            
            let content = doc.get_first(self.content_field)
                .and_then(|v| v.as_str())
                .unwrap_or("");
            
            // Create a preview (first 200 chars)
            let preview = if content.chars().count() > 200 {
                content.chars().take(200).collect::<String>() + "..."
            } else {
                content.to_string()
            };

            results.push(SearchResult {
                id,
                title,
                preview,
                score,
            });
        }

        Ok(results)
    }

    /// Get article by ID
    pub fn get_by_id(&self, article_id: u64) -> Result<Option<SearchResult>> {
        let results = self.search(&format!("id:{}", article_id), 1)?;
        Ok(results.into_iter().next())
    }

    /// Get full article by ID
    pub fn get_article(&self, article_id: u64) -> Result<Option<Article>> {
        let reader = self.index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()?;
        let searcher = reader.searcher();

        let query = self.query_parser.parse_query(&format!("id:{}", article_id))?;
        let top_docs = searcher.search(&query, &TopDocs::with_limit(1))?;

        if let Some((_, doc_address)) = top_docs.first() {
            let doc: TantivyDocument = searcher.doc(*doc_address)?;
            
            let id = doc.get_first(self.id_field)
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            
            let title = doc.get_first(self.title_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            
            // Get raw content with HTML for display
            let content = doc.get_first(self.raw_content_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let categories = doc.get_all(self.categories_field)
                .filter_map(|v| v.as_str())
                .map(|s| s.to_string())
                .collect();

            // Note: We don't store raw_markup/etc in the index currently,
            // so we return a basic Article object.
            let mut article = Article::new(id, title, content);
            article.categories = categories;
            Ok(Some(article))
        } else {
            Ok(None)
        }
    }
}

