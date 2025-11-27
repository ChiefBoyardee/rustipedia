//! Wikipedia dump downloader

use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Write};

use anyhow::{Context, Result};
use bzip2::read::BzDecoder;
use indicatif::{ProgressBar, ProgressStyle};
use quick_xml::events::Event;
use quick_xml::Reader;

use crate::article::{Article, ExtractionStats};
use crate::config::Config;
use crate::parser::{ParsedArticle, WikiParser};

/// Wikipedia downloader and extractor
pub struct WikiDownloader {
    config: Config,
    parser: WikiParser,
}

impl WikiDownloader {
    /// Create a new downloader with default config
    pub fn new() -> Self {
        Self {
            config: Config::default(),
            parser: WikiParser::new(),
        }
    }

    /// Create a downloader with custom config
    pub fn with_config(config: Config) -> Self {
        let parser = WikiParser::new().with_min_length(config.min_length);
        Self { config, parser }
    }

    /// Get the config
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Download the Wikipedia dump file
    pub fn download(&self) -> Result<()> {
        let lang = self.config.wiki_language();
        let dump_path = self.config.dump_path();

        // Check if dump already exists
        if dump_path.exists() && self.config.skip_download {
            tracing::info!("Dump file already exists, skipping download: {:?}", dump_path);
            return Ok(());
        }

        // Create output directory
        fs::create_dir_all(&self.config.output_dir)
            .context("Failed to create output directory")?;

        let url = lang.dump_url();
        tracing::info!("Downloading {} Wikipedia dump...", lang.display_name());
        tracing::info!("URL: {}", url);
        tracing::info!("Estimated size: {}", lang.estimated_size());

        // Create HTTP client with long timeout
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(7200)) // 2 hours
            .build()?;

        let response = client.get(&url).send()
            .context("Failed to start download")?;

        if !response.status().is_success() {
            anyhow::bail!("Download failed with status: {}", response.status());
        }

        let total_size = response.content_length().unwrap_or(0);
        
        // Create progress bar
        let pb = if total_size > 0 {
            let pb = ProgressBar::new(total_size);
            pb.set_style(ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, ETA: {eta})")
                .unwrap()
                .progress_chars("█▉▊▋▌▍▎▏  "));
            pb
        } else {
            let pb = ProgressBar::new_spinner();
            pb.set_style(ProgressStyle::default_spinner()
                .template("{spinner:.green} [{elapsed_precise}] Downloaded: {bytes}")
                .unwrap());
            pb
        };

        // Stream to file
        let mut file = File::create(&dump_path)
            .context("Failed to create dump file")?;
        let mut response = response;
        let mut buffer = [0u8; 65536]; // 64KB buffer
        let mut downloaded = 0u64;

        loop {
            let bytes_read = response.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            file.write_all(&buffer[..bytes_read])?;
            downloaded += bytes_read as u64;
            pb.set_position(downloaded);
        }

        pb.finish_with_message("Download complete!");
        tracing::info!("Downloaded {} to {:?}", format_bytes(downloaded), dump_path);

        Ok(())
    }

    /// Extract articles from the downloaded dump
    pub fn extract(&self) -> Result<ExtractionStats> {
        let lang = self.config.wiki_language();
        let dump_path = self.config.dump_path();

        if !dump_path.exists() {
            anyhow::bail!("Dump file not found: {:?}. Run download first.", dump_path);
        }

        tracing::info!("Extracting articles from {:?}...", dump_path);

        // Initialize stats
        let dump_filename = dump_path.file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let mut stats = ExtractionStats::new(lang.code(), &dump_filename, self.config.min_length);

        // Open dump file
        let file = File::open(&dump_path)?;
        let file_size = file.metadata()?.len();
        let reader = BufReader::with_capacity(1024 * 1024, file); // 1MB buffer

        // Decompress bz2
        let decompressor = BzDecoder::new(reader);

        // Create output file
        let output_path = self.config.data_path();
        let mut writer = BufWriter::new(File::create(&output_path)?);

        // Progress bar (estimated based on file size)
        let pb = ProgressBar::new(file_size);
        pb.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} compressed bytes ({msg})")
            .unwrap()
            .progress_chars("█▉▊▋▌▍▎▏  "));
        pb.set_message("0 articles");

        // Parse XML
        let mut xml_reader = Reader::from_reader(BufReader::new(decompressor));
        xml_reader.config_mut().trim_text(true);

        let mut buf = Vec::with_capacity(1024 * 1024);
        let mut current_title = String::new();
        let mut current_text = String::new();
        let mut current_id: u64 = 0;
        let mut in_title = false;
        let mut in_text = false;
        let mut in_id = false;
        let mut first_id = true;

        loop {
            match xml_reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) => {
                    match e.name().as_ref() {
                        b"title" => in_title = true,
                        b"text" => in_text = true,
                        b"id" => {
                            if first_id {
                                in_id = true;
                            }
                        },
                        b"page" => first_id = true,
                        _ => {}
                    }
                }
                Ok(Event::End(e)) => {
                    match e.name().as_ref() {
                        b"title" => in_title = false,
                        b"id" => {
                            in_id = false;
                            first_id = false;
                        },
                        b"text" => {
                            in_text = false;

                            // Process the article
                            match self.parser.parse_article(&current_title, &current_text) {
                                Some(ParsedArticle::Article { title, content, categories, raw_markup }) => {
                                    let article = Article {
                                        id: current_id,
                                        title,
                                        content: content.clone(),
                                        raw_markup,
                                        categories,
                                        redirect_to: None,
                                        extracted_at: chrono::Utc::now(),
                                    };

                                    // Write as JSONL
                                    let json = serde_json::to_string(&article)?;
                                    writeln!(writer, "{}", json)?;

                                    stats.articles_extracted += 1;
                                    stats.total_bytes += content.len() as u64;

                                    if stats.articles_extracted % 1000 == 0 {
                                        pb.set_message(format!("{} articles", stats.articles_extracted));
                                    }

                                    // Check max articles limit
                                    if self.config.max_articles > 0 
                                        && stats.articles_extracted >= self.config.max_articles as u64 
                                    {
                                        tracing::info!("Reached max articles limit ({})", self.config.max_articles);
                                        break;
                                    }
                                }
                                Some(ParsedArticle::Redirect { .. }) => {
                                    stats.redirects += 1;
                                    stats.articles_skipped += 1;
                                }
                                None => {
                                    stats.articles_skipped += 1;
                                }
                            }

                            current_title.clear();
                            current_text.clear();
                            current_id = 0;
                        }
                        _ => {}
                    }
                }
                Ok(Event::Text(e)) => {
                    let text = e.unescape().unwrap_or_default();
                    if in_title {
                        current_title.push_str(&text);
                    } else if in_text {
                        current_text.push_str(&text);
                    } else if in_id {
                        if let Ok(id) = text.parse::<u64>() {
                            current_id = id;
                        }
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => {
                    tracing::warn!("XML parse error at article {}: {}", stats.articles_extracted, e);
                    current_title.clear();
                    current_text.clear();
                }
                _ => {}
            }

            // Update progress (approximate based on buffer position)
            pb.set_position(xml_reader.buffer_position() as u64);
            buf.clear();
        }

        writer.flush()?;
        pb.finish_with_message(format!("{} articles extracted!", stats.articles_extracted));

        // Finalize stats
        stats.finish();

        // Save stats
        let stats_json = serde_json::to_string_pretty(&stats)?;
        fs::write(self.config.stats_path(), stats_json)?;

        // Save config
        self.config.save()?;

        // Optionally clean up dump file
        if !self.config.keep_dump {
            tracing::info!("Cleaning up dump file...");
            fs::remove_file(&dump_path).ok();
        }

        tracing::info!("Extraction complete!");
        tracing::info!("  Articles extracted: {}", stats.articles_extracted);
        tracing::info!("  Articles skipped: {}", stats.articles_skipped);
        tracing::info!("  Redirects: {}", stats.redirects);
        tracing::info!("  Total content: {}", format_bytes(stats.total_bytes));
        tracing::info!("  Output: {:?}", output_path);

        Ok(stats)
    }

    /// Download and extract in one step
    pub fn run(&self) -> Result<ExtractionStats> {
        self.download()?;
        self.extract()
    }
}

impl Default for WikiDownloader {
    fn default() -> Self {
        Self::new()
    }
}

/// Format bytes as human-readable string
fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} bytes", bytes)
    }
}

