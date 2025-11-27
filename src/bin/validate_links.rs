//! Analyze and validate internal wiki links
//!
//! This tool scans all articles and checks if internal wiki links point to existing articles.

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use regex::Regex;
use once_cell::sync::Lazy;

use wiki_download::Article;

static LINK_PIPE_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r#"<a href="/wiki/([^"]+)">([^<]+)</a>"#).unwrap());

#[derive(Parser)]
#[command(name = "wiki-link-validator")]
#[command(about = "Validate internal wiki links in articles")]
struct Cli {
    /// Directory containing articles.jsonl
    #[arg(short, long, default_value = "wikipedia")]
    data: PathBuf,
    
    /// Show detailed broken links (can be verbose)
    #[arg(short, long)]
    verbose: bool,
    
    /// Maximum number of broken links to display
    #[arg(short, long, default_value = "20")]
    limit: usize,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    
    println!("🔍 Loading articles and analyzing links...\n");
    
    let articles_path = cli.data.join("articles.jsonl");
    if !articles_path.exists() {
        anyhow::bail!("Articles file not found: {:?}", articles_path);
    }
    
    // First pass: build title index (case-insensitive)
    println!("📚 Building article index...");
    let mut title_index: HashSet<String> = HashSet::new();
    let file = File::open(&articles_path)?;
    let reader = BufReader::new(file);
    
    for line in reader.lines() {
        let line = line?;
        if line.is_empty() {
            continue;
        }
        let article: Article = serde_json::from_str(&line)?;
        title_index.insert(article.title.to_lowercase().replace('_', " "));
    }
    
    println!("   Found {} articles\n", title_index.len());
    
    // Second pass: check all links
    println!("🔗 Scanning links in articles...");
    let file = File::open(&articles_path)?;
    let reader = BufReader::new(file);
    
    let mut total_articles = 0;
    let mut articles_with_links = 0;
    let mut total_links = 0;
    let mut valid_links = 0;
    let mut broken_links: HashMap<String, usize> = HashMap::new();
    let mut broken_link_examples: Vec<(String, String, String)> = Vec::new(); // (article, link_target, link_text)
    
    for line in reader.lines() {
        let line = line?;
        if line.is_empty() {
            continue;
        }
        
        let article: Article = serde_json::from_str(&line)?;
        total_articles += 1;
        
        let mut article_has_links = false;
        
        // Extract all links from the article content
        for cap in LINK_PIPE_RE.captures_iter(&article.content) {
            article_has_links = true;
            total_links += 1;
            
            let target = cap.get(1).unwrap().as_str();
            let link_text = cap.get(2).unwrap().as_str();
            let normalized_target = target.to_lowercase().replace('_', " ");
            
            if title_index.contains(&normalized_target) {
                valid_links += 1;
            } else {
                *broken_links.entry(target.to_string()).or_insert(0) += 1;
                
                if broken_link_examples.len() < cli.limit {
                    broken_link_examples.push((
                        article.title.clone(),
                        target.to_string(),
                        link_text.to_string(),
                    ));
                }
            }
        }
        
        if article_has_links {
            articles_with_links += 1;
        }
    }
    
    // Print statistics
    println!("\n╔══════════════════════════════════════════════════════════════════╗");
    println!("║                      📊 LINK ANALYSIS RESULTS                     ║");
    println!("╠══════════════════════════════════════════════════════════════════╣");
    println!("║  Total articles:           {:>8}                              ║", total_articles);
    println!("║  Articles with links:      {:>8}                              ║", articles_with_links);
    println!("║  Total internal links:     {:>8}                              ║", total_links);
    println!("║  Valid links:              {:>8} ({:>5.1}%)                   ║", 
        valid_links, 
        if total_links > 0 { (valid_links as f64 / total_links as f64) * 100.0 } else { 0.0 }
    );
    println!("║  Broken links:             {:>8} ({:>5.1}%)                   ║", 
        total_links - valid_links,
        if total_links > 0 { ((total_links - valid_links) as f64 / total_links as f64) * 100.0 } else { 0.0 }
    );
    println!("║  Unique broken targets:    {:>8}                              ║", broken_links.len());
    println!("╚══════════════════════════════════════════════════════════════════╝");
    
    if !broken_links.is_empty() {
        println!("\n🔴 Most Common Broken Link Targets:");
        let mut broken_vec: Vec<_> = broken_links.iter().collect();
        broken_vec.sort_by(|a, b| b.1.cmp(a.1));
        
        for (target, count) in broken_vec.iter().take(20) {
            println!("   /wiki/{:<40} ({} occurrences)", target, count);
        }
        
        if cli.verbose && !broken_link_examples.is_empty() {
            println!("\n📝 Example Broken Links:");
            for (article_title, target, link_text) in broken_link_examples.iter() {
                println!("   Article: \"{}\"", article_title);
                println!("   Link: [{}] -> /wiki/{}", link_text, target);
                println!();
            }
        }
        
        println!("\n💡 Recommendations:");
        println!("   1. These broken links are expected - not all Wikipedia pages exist in your dump");
        println!("   2. The /wiki/:title route correctly returns 404 for missing articles");
        println!("   3. Options to improve user experience:");
        println!("      a) Add visual indication (e.g., red color) for broken links");
        println!("      b) Remove broken links during parsing (simplest but loses information)");
        println!("      c) Link to full Wikipedia for missing articles");
        println!("      d) Do nothing - 404 pages are acceptable");
    } else {
        println!("\n✅ All links are valid!");
    }
    
    Ok(())
}
