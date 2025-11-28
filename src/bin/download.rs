//! Rustipedia Download CLI
//!
//! Download your own local copy of Wikipedia.
//!
//! # Examples
//!
//! Download Simple English Wikipedia (recommended for testing):
//! ```bash
//! rustipedia-download --lang simple
//! ```
//!
//! Download full English Wikipedia:
//! ```bash
//! rustipedia-download --lang en
//! ```
//!
//! Download with custom options:
//! ```bash
//! rustipedia-download --lang de --output ./german-wiki --max-articles 10000
//! ```

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

use rustipedia::{Config, WikiDownloader, WikiLanguage, SearchIndex};

#[derive(Parser)]
#[command(name = "rustipedia-download")]
#[command(author, version, about = "Download your own local copy of Wikipedia")]
#[command(long_about = r#"
╔══════════════════════════════════════════════════════════════════╗
║                     RUSTIPEDIA DOWNLOAD                           ║
║           Download & Serve Your Own Local Wikipedia              ║
╚══════════════════════════════════════════════════════════════════╝

Download Wikipedia dumps directly from Wikimedia and extract them
into a searchable, browsable local archive.

EXAMPLES:
  Download Simple English Wikipedia (fast, ~300MB):
    rustipedia-download simple

  Download full English Wikipedia (~22GB):
    rustipedia-download --lang en

  Download German Wikipedia to custom directory:
    rustipedia-download --lang de --output ./german-wiki

  List all available languages:
    rustipedia-download list

  Only download the dump (don't extract):
    rustipedia-download --lang simple --download-only

  Resume extraction from existing dump:
    rustipedia-download --lang simple --skip-download
"#)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Wikipedia language to download
    #[arg(short, long, default_value = "simple")]
    lang: String,

    /// Output directory for downloaded data
    #[arg(short, long, default_value = "wikipedia")]
    output: PathBuf,

    /// Maximum articles to extract (0 = unlimited)
    #[arg(short, long, default_value = "0")]
    max_articles: usize,

    /// Minimum article length in characters
    #[arg(long, default_value = "200")]
    min_length: usize,

    /// Skip download if dump already exists
    #[arg(long)]
    skip_download: bool,

    /// Only download, don't extract
    #[arg(long)]
    download_only: bool,

    /// Build search index after extraction
    #[arg(long, default_value = "true")]
    build_index: bool,

    /// Keep the raw bz2 dump file after extraction
    #[arg(long)]
    keep_dump: bool,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,

    /// Prune broken links (remove links to articles that don't exist in the dump)
    #[arg(long)]
    prune_links: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// List all available Wikipedia languages
    List,
    
    /// Download Wikipedia for a specific language
    Download {
        /// Wikipedia language code (e.g., simple, en, de, fr)
        lang: Option<String>,
    },
    
    /// Extract articles from an existing dump
    Extract {
        /// Path to the dump file
        dump: PathBuf,
        
        /// Output directory
        #[arg(short, long, default_value = "wikipedia")]
        output: PathBuf,
    },
    
    /// Build search index from extracted articles
    Index {
        /// Directory containing articles.jsonl
        #[arg(default_value = "wikipedia")]
        data_dir: PathBuf,
    },
    
    /// Prune broken links from extracted articles
    Prune {
        /// Directory containing articles.jsonl
        #[arg(default_value = "wikipedia")]
        data_dir: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let filter = if cli.verbose {
        EnvFilter::new("rustipedia_download=debug,info")
    } else {
        EnvFilter::new("rustipedia_download=info,warn")
    };
    
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();

    match &cli.command {
        Some(Commands::List) => {
            print_languages();
            Ok(())
        }
        
        Some(Commands::Download { lang }) => {
            let lang_code = lang.clone().unwrap_or_else(|| cli.lang.clone());
            download_wikipedia(&lang_code, &cli)
        }
        
        Some(Commands::Extract { dump, output }) => {
            extract_dump(dump, output, &cli)
        }
        
        Some(Commands::Index { data_dir }) => {
            build_index(data_dir)
        }
        
        Some(Commands::Prune { data_dir }) => {
            prune_articles(data_dir)
        }
        
        None => {
            // Default action: download + extract
            download_wikipedia(&cli.lang, &cli)
        }
    }
}

fn print_languages() {
    println!("\n╔══════════════════════════════════════════════════════════════════╗");
    println!("║               📚 Available Wikipedia Languages                   ║");
    println!("╠══════════════════════════════════════════════════════════════════╣");
    println!("║  Code    │ Name                  │ Articles  │ Dump Size        ║");
    println!("╠══════════════════════════════════════════════════════════════════╣");
    
    for lang in WikiLanguage::all() {
        println!(
            "║  {:<7} │ {:<21} │ {:<9} │ {:<16} ║",
            lang.code(),
            lang.display_name(),
            lang.estimated_articles(),
            lang.estimated_size()
        );
    }
    
    println!("╚══════════════════════════════════════════════════════════════════╝");
    println!("\nUsage: rustipedia-download --lang <CODE> [OPTIONS]");
    println!("\nRecommended for testing: rustipedia-download --lang simple");
    println!("(Simple English is only ~300MB and downloads in minutes)\n");
    println!("Note: Extracted size will be roughly 3-4x the dump size.");
}

fn download_wikipedia(lang: &str, cli: &Cli) -> Result<()> {
    // Parse language
    let language = WikiLanguage::from_code(lang)
        .ok_or_else(|| anyhow::anyhow!("Unknown language: {}. Use 'rustipedia-download list' to see available languages.", lang))?;

    print_banner(&language);

    // Create config
    let config = Config {
        language: language.code().to_string(),
        output_dir: cli.output.clone(),
        max_articles: cli.max_articles,
        min_length: cli.min_length,
        skip_download: cli.skip_download,
        build_index: cli.build_index,
        keep_dump: cli.keep_dump,
    };

    // Create downloader
    let downloader = WikiDownloader::with_config(config.clone());

    // Download
    if !cli.download_only {
        // Download and extract
        let stats = downloader.run()?;
        
        println!("\n╔══════════════════════════════════════════════════════════════════╗");
        println!("║                     ✅ Extraction Complete!                       ║");
        println!("╠══════════════════════════════════════════════════════════════════╣");
        println!("║  Articles extracted: {:>10}                                  ║", stats.articles_extracted);
        println!("║  Articles skipped:   {:>10}                                  ║", stats.articles_skipped);
        println!("║  Redirects:          {:>10}                                  ║", stats.redirects);
        if let Some(duration) = stats.duration_secs {
            println!("║  Duration:           {:>10.1}s                                 ║", duration);
        }
        println!("╚══════════════════════════════════════════════════════════════════╝");

        // Prune links if requested
        if cli.prune_links {
            prune_articles(&cli.output)?;
        }

        // Build search index if enabled
        if cli.build_index {
            println!("\n📇 Building search index...");
            let index_path = config.index_path();
            let data_path = config.data_path();
            
            let index = SearchIndex::create(&index_path)?;
            let indexed = index.build_from_jsonl(&data_path)?;
            println!("✅ Indexed {} articles", indexed);
        }
    } else {
        // Download only
        downloader.download()?;
        println!("\n✅ Download complete! Use --skip-download to extract.");
    }

    println!("\n📂 Data saved to: {:?}", cli.output);
    println!("\n🚀 To serve your Wikipedia:");
    println!("   rustipedia-serve --data {:?}", cli.output);

    Ok(())
}

fn extract_dump(dump: &PathBuf, output: &PathBuf, cli: &Cli) -> Result<()> {
    println!("📦 Extracting from {:?}...", dump);
    
    let config = Config {
        language: "custom".to_string(),
        output_dir: output.clone(),
        max_articles: cli.max_articles,
        min_length: cli.min_length,
        skip_download: true,
        build_index: cli.build_index,
        keep_dump: true,
    };

    let downloader = WikiDownloader::with_config(config);
    let stats = downloader.extract()?;
    
    println!("✅ Extracted {} articles", stats.articles_extracted);
    
    if cli.prune_links {
        prune_articles(output)?;
    }
    
    Ok(())
}

fn build_index(data_dir: &PathBuf) -> Result<()> {
    let index_path = data_dir.join("search_index");
    let data_path = data_dir.join("articles.jsonl");
    
    if !data_path.exists() {
        anyhow::bail!("Articles file not found: {:?}. Run download first.", data_path);
    }
    
    println!("📇 Building search index...");
    let index = SearchIndex::create(&index_path)?;
    let indexed = index.build_from_jsonl(&data_path)?;
    println!("✅ Indexed {} articles to {:?}", indexed, index_path);
    
    Ok(())
}

fn prune_articles(data_dir: &PathBuf) -> Result<()> {
    use std::collections::HashSet;
    use std::fs::File;
    use std::io::{BufRead, BufReader, Write, BufWriter};
    use rustipedia::Article;
    use indicatif::{ProgressBar, ProgressStyle};

    let articles_path = data_dir.join("articles.jsonl");
    let temp_path = data_dir.join("articles_pruned.jsonl");
    
    if !articles_path.exists() {
        anyhow::bail!("Articles file not found: {:?}", articles_path);
    }

    println!("\n✂️  Pruning broken links...");
    
    // Pass 1: Collect titles
    println!("   Scanning articles to build title index...");
    let mut title_index: HashSet<String> = HashSet::new();
    let file = File::open(&articles_path)?;
    let reader = BufReader::new(file);
    
    let pb = ProgressBar::new_spinner();
    pb.set_style(ProgressStyle::default_spinner()
        .template("{spinner:.green} {msg}")
        .unwrap());
    
    for line in reader.lines() {
        let line = line?;
        if line.is_empty() { continue; }
        
        // Fast parse just for title to avoid full deserialization overhead if possible
        // But we need to handle JSON correctly.
        // Let's just use serde_json::from_str::<Article> for safety, or a minimal struct
        #[derive(serde::Deserialize)]
        struct TitleOnly {
            title: String,
        }
        if let Ok(article) = serde_json::from_str::<TitleOnly>(&line) {
            title_index.insert(article.title.to_lowercase().replace('_', " "));
        }
        
        if title_index.len() % 1000 == 0 {
            pb.set_message(format!("Found {} articles...", title_index.len()));
        }
    }
    pb.finish_with_message(format!("✅ Found {} valid titles", title_index.len()));
    
    // Pass 2: Prune links
    println!("   Rewriting articles with valid links only...");
    let file = File::open(&articles_path)?;
    let reader = BufReader::new(file);
    let out_file = File::create(&temp_path)?;
    let mut writer = BufWriter::new(out_file);
    
    let pb = ProgressBar::new(title_index.len() as u64);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
        .unwrap()
        .progress_chars("#>-"));
        
    for line in reader.lines() {
        let line = line?;
        if line.is_empty() { continue; }
        
        let mut article: Article = serde_json::from_str(&line)?;
        
        // If we have raw markup, we should re-process it.
        // But currently Article struct stores `content` (HTML) and `raw_markup` (WikiText).
        // If we only have HTML in `content`, we can't easily "un-link" without parsing HTML.
        // However, `WikiParser::clean_wiki_markup` produced the HTML.
        // If we saved `raw_markup`, we can re-generate `content`.
        // If we didn't save `raw_markup`, we are in trouble unless we parse HTML.
        
        // The default `WikiParser` config has `keep_raw: false`.
        // So `article.raw_markup` is likely None.
        // This means we need to process the HTML in `article.content`.
        // But `clean_wiki_markup` produced HTML like `<a href="/wiki/Target">Text</a>`.
        // We can use Regex to replace these in the HTML!
        
        // Regex for HTML links: <a href="/wiki/([^"]+)">([^<]+)</a>
        let link_re = regex::Regex::new(r#"<a href="/wiki/([^"]+)">([^<]+)</a>"#).unwrap();
        
        let new_content = link_re.replace_all(&article.content, |caps: &regex::Captures| {
            let target = &caps[1];
            let text = &caps[2];
            let normalized = target.to_lowercase().replace('_', " ");
            
            if title_index.contains(&normalized) {
                // Keep link
                caps[0].to_string()
            } else {
                // Remove link, keep text
                text.to_string()
            }
        }).to_string();
        
        article.content = new_content;
        
        serde_json::to_writer(&mut writer, &article)?;
        writer.write_all(b"\n")?;
        pb.inc(1);
    }
    pb.finish_with_message("✅ Pruning complete");
    
    // Replace original file
    std::fs::rename(&temp_path, &articles_path)?;
    
    println!("✅ Replaced articles.jsonl with pruned version");
    
    Ok(())
}

fn print_banner(lang: &WikiLanguage) {
    println!();
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║                     RUSTIPEDIA DOWNLOAD                           ║");
    println!("╠══════════════════════════════════════════════════════════════════╣");
    println!("║  Language:    {} ({})                              ", lang.display_name(), lang.code());
    println!("║  Articles:    {}                                              ", lang.estimated_articles());
    println!("║  Dump Size:   {}                                              ", lang.estimated_size());
    println!("║  Final Size:  ~3-4x Dump Size                                    ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");
    println!();
}

