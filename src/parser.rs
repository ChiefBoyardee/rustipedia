//! Wikipedia XML dump parser

use regex::Regex;
use once_cell::sync::Lazy;

/// Regex patterns for wiki markup cleaning (compiled once)
static REF_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"<ref[^>]*>.*?</ref>").unwrap());
static REF_SELF_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"<ref[^/]*/\s*>").unwrap());
static COMMENT_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"<!--.*?-->").unwrap());
static LINK_PIPE_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\[\[([^\]|]*)\|([^\]]*)\]\]").unwrap());
static LINK_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\[\[([^\]]*)\]\]").unwrap());
static EXT_LINK_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\[https?://[^\s\]]*\s*([^\]]*)\]").unwrap());

static HEADER_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"={2,}[^=]+={2,}").unwrap());
static BULLET_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?m)^\s*[\*#:]+\s*").unwrap());
static HTML_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"<[^>]+>").unwrap());
static MULTI_SPACE_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"[ \t]+").unwrap());
static MULTI_NEWLINE_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\n{3,}").unwrap());

/// Special page namespace prefixes to skip
const SKIP_PREFIXES: &[&str] = &[
    "Wikipedia:", "Template:", "Category:", "File:", "Image:",
    "Help:", "Portal:", "Draft:", "MediaWiki:", "Module:",
    "User:", "Talk:", "User talk:", "Wikipedia talk:",
    "Template talk:", "Category talk:", "File talk:",
    "Help talk:", "Portal talk:", "Draft talk:",
];

/// Wikipedia XML dump parser
pub struct WikiParser {
    /// Minimum article length to include
    min_length: usize,
    /// Keep raw markup in articles
    keep_raw: bool,
}

impl WikiParser {
    /// Create a new parser with default settings
    pub fn new() -> Self {
        Self {
            min_length: 200,
            keep_raw: false,
        }
    }

    /// Set minimum article length
    pub fn with_min_length(mut self, min: usize) -> Self {
        self.min_length = min;
        self
    }

    /// Keep raw wiki markup in articles
    pub fn with_raw_markup(mut self, keep: bool) -> Self {
        self.keep_raw = keep;
        self
    }

    /// Check if text is a redirect page
    pub fn is_redirect(text: &str) -> bool {
        let lower = text.trim().to_lowercase();
        lower.starts_with("#redirect") || lower.starts_with("# redirect")
    }

    /// Extract redirect target from redirect text
    pub fn extract_redirect_target(text: &str) -> Option<String> {
        if !Self::is_redirect(text) {
            return None;
        }
        // Match [[Target]] or [[Target|Display]]
        LINK_RE.captures(text).map(|caps| {
            caps.get(1).map(|m| {
                let full = m.as_str();
                // Handle [[Target|Display]] - take just Target
                if let Some(pipe_pos) = full.find('|') {
                    full[..pipe_pos].to_string()
                } else {
                    full.to_string()
                }
            }).unwrap_or_default()
        })
    }

    /// Check if this is a content article (not a special page)
    pub fn is_content_article(title: &str) -> bool {
        for prefix in SKIP_PREFIXES {
            if title.starts_with(prefix) {
                return false;
            }
        }
        true
    }

    /// Extract categories from wiki markup
    pub fn extract_categories(text: &str) -> Vec<String> {
        let cat_re = Regex::new(r"\[\[Category:([^\]|]+)").unwrap();
        cat_re.captures_iter(text)
            .filter_map(|caps| caps.get(1).map(|m| m.as_str().trim().to_string()))
            .collect()
    }



    /// Escape HTML special characters
    fn html_escape(s: &str) -> String {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&#x27;")
    }

    /// Clean Wikipedia markup to plain text
    pub fn clean_wiki_markup(text: &str) -> String {
        Self::clean_wiki_markup_with_filter(text, None)
    }

    /// Clean Wikipedia markup to plain text, optionally filtering links
    pub fn clean_wiki_markup_with_filter(text: &str, valid_titles: Option<&std::collections::HashSet<String>>) -> String {
        let mut result = text.to_string();

        // Remove templates {{...}} and tables {|...|} using a stack to handle nesting
        let mut clean_buffer = String::with_capacity(result.len());
        let mut stack: Vec<&str> = Vec::new();
        let mut chars = result.chars().peekable();
        
        while let Some(c) = chars.next() {
            let next_char = chars.peek().copied();
            
            // Check for starts
            if c == '{' && next_char == Some('{') {
                chars.next(); // consume second {
                stack.push("}}");
                continue;
            }
            if c == '{' && next_char == Some('|') {
                chars.next(); // consume |
                stack.push("|}");
                continue;
            }
            
            // Check for ends
            if let Some(&expected_close) = stack.last() {
                if expected_close == "}}" {
                    if c == '}' && next_char == Some('}') {
                        chars.next(); // consume second }
                        stack.pop();
                        continue;
                    }
                } else if expected_close == "|}" {
                    if c == '|' && next_char == Some('}') {
                        chars.next(); // consume }
                        stack.pop();
                        continue;
                    }
                }
                
                // Inside a structure, ignore content
                continue;
            }
            
            // Not inside a structure, keep character
            clean_buffer.push(c);
        }
        result = clean_buffer;

        // Remove references <ref>...</ref>
        result = REF_RE.replace_all(&result, "").to_string();
        result = REF_SELF_RE.replace_all(&result, "").to_string();

        // Remove HTML comments
        result = COMMENT_RE.replace_all(&result, "").to_string();

        // Remove File/Image/Category links BEFORE processing other links
        // We use a loop to handle potential nesting or adjacent tags that regex might miss in one go
        let mut clean_buffer = String::with_capacity(result.len());
        let mut chars = result.chars().peekable();
        
        while let Some(c) = chars.next() {
            if c == '[' && chars.peek() == Some(&'[') {
                // Potential start of link
                chars.next(); // consume second [
                
                // Read ahead to see if it's File/Image/Category
                // We need to capture the content until matching ]]
                let mut depth = 1;
                let mut content = String::new();
                let mut is_closed = false;
                
                while let Some(inner_c) = chars.next() {
                    if inner_c == '[' && chars.peek() == Some(&'[') {
                        chars.next();
                        depth += 1;
                        content.push_str("[[");
                    } else if inner_c == ']' && chars.peek() == Some(&']') {
                        chars.next();
                        depth -= 1;
                        if depth == 0 {
                            is_closed = true;
                            break;
                        }
                        content.push_str("]]");
                    } else {
                        content.push(inner_c);
                    }
                }
                
                if is_closed {
                    // Check if it's a file/category
                    let lower_content = content.trim_start().to_lowercase();
                    if lower_content.starts_with("file:") || 
                       lower_content.starts_with("image:") || 
                       lower_content.starts_with("category:") {
                        // Skip (remove)
                    } else {
                        // It's a regular link, keep it (reconstruct)
                        clean_buffer.push_str("[[");
                        clean_buffer.push_str(&content);
                        clean_buffer.push_str("]]");
                    }
                } else {
                    // Unclosed brackets, just keep as is
                    clean_buffer.push_str("[[");
                    clean_buffer.push_str(&content);
                }
            } else {
                clean_buffer.push(c);
            }
        }
        result = clean_buffer;

        // Convert wiki links [[text|display]] or [[text]] to just the display text
        // Remove external links [http://... text]
        result = EXT_LINK_RE.replace_all(&result, "$1").to_string();

        // Remove bold/italic markup
        result = result.replace("'''", "").replace("''", "");

        // Remove section headers (== Title ==) but keep the title text
        result = HEADER_RE.replace_all(&result, "\n").to_string();

        // Remove bullet points and indentation
        result = BULLET_RE.replace_all(&result, "").to_string();

        // Remove remaining HTML tags
        result = HTML_RE.replace_all(&result, "").to_string();

        // Convert wiki links [[target|display]] to HTML
        result = LINK_PIPE_RE.replace_all(&result, |caps: &regex::Captures| {
            let target = &caps[1];
            let text = &caps[2];
            
            if let Some(valid) = valid_titles {
                let normalized = target.to_lowercase().replace('_', " ");
                if valid.contains(&normalized) {
                    format!("<a href=\"/wiki/{}\">{}</a>", urlencoding::encode(target), Self::html_escape(text))
                } else {
                    Self::html_escape(text)
                }
            } else {
                format!("<a href=\"/wiki/{}\">{}</a>", urlencoding::encode(target), Self::html_escape(text))
            }
        }).to_string();

        // Convert wiki links [[target]] to HTML
        result = LINK_RE.replace_all(&result, |caps: &regex::Captures| {
            let target = &caps[1];
            
            if let Some(valid) = valid_titles {
                let normalized = target.to_lowercase().replace('_', " ");
                if valid.contains(&normalized) {
                    format!("<a href=\"/wiki/{}\">{}</a>", urlencoding::encode(target), Self::html_escape(target))
                } else {
                    Self::html_escape(target)
                }
            } else {
                format!("<a href=\"/wiki/{}\">{}</a>", urlencoding::encode(target), Self::html_escape(target))
            }
        }).to_string();

        // Clean up whitespace
        result = MULTI_SPACE_RE.replace_all(&result, " ").to_string();
        result = MULTI_NEWLINE_RE.replace_all(&result, "\n\n").to_string();

        result.trim().to_string()
    }

    /// Parse article content, return None if it should be skipped
    pub fn parse_article(&self, title: &str, text: &str) -> Option<ParsedArticle> {
        // Skip non-content pages
        if !Self::is_content_article(title) {
            return None;
        }

        // Handle redirects
        if Self::is_redirect(text) {
            if let Some(target) = Self::extract_redirect_target(text) {
                return Some(ParsedArticle::Redirect {
                    title: title.to_string(),
                    target,
                });
            }
            return None;
        }

        // Extract categories before cleaning
        let categories = Self::extract_categories(text);

        // Clean the markup
        let content = Self::clean_wiki_markup(text);

        // Check minimum length
        if content.len() < self.min_length {
            return None;
        }

        Some(ParsedArticle::Article {
            title: title.to_string(),
            content,
            categories,
            raw_markup: if self.keep_raw { Some(text.to_string()) } else { None },
        })
    }
}

impl Default for WikiParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of parsing an article
#[derive(Debug, Clone)]
pub enum ParsedArticle {
    /// A regular article
    Article {
        title: String,
        content: String,
        categories: Vec<String>,
        raw_markup: Option<String>,
    },
    /// A redirect page
    Redirect {
        title: String,
        target: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_wiki_markup_links() {
        let input = "This is a [[link]] and [[target|displayed text]].";
        let result = WikiParser::clean_wiki_markup(input);
        assert_eq!(result, "This is a <a href=\"/wiki/link\">link</a> and <a href=\"/wiki/target\">displayed text</a>.");
    }

    #[test]
    fn test_is_redirect() {
        assert!(WikiParser::is_redirect("#REDIRECT [[Target]]"));
        assert!(WikiParser::is_redirect("# redirect [[Target]]"));
        assert!(!WikiParser::is_redirect("Normal article text"));
    }

    #[test]
    fn test_extract_redirect_target() {
        let target = WikiParser::extract_redirect_target("#REDIRECT [[United States]]");
        assert_eq!(target, Some("United States".to_string()));
    }

    #[test]
    fn test_is_content_article() {
        assert!(WikiParser::is_content_article("Albert Einstein"));
        assert!(!WikiParser::is_content_article("Wikipedia:About"));
        assert!(!WikiParser::is_content_article("Category:Science"));
    }

    #[test]
    fn test_extract_categories() {
        let text = "Some text [[Category:Science]] and [[Category:Physics]] more text.";
        let cats = WikiParser::extract_categories(text);
        assert_eq!(cats, vec!["Science", "Physics"]);
    }

    #[test]
    fn test_clean_wiki_markup_xss() {
        // Test 1: HTML tags should be stripped by HTML_RE
        let input = "[[<script>alert(1)</script>]]";
        let result = WikiParser::clean_wiki_markup(input);
        assert!(!result.contains("<script>"));
        assert!(!result.contains("&lt;script&gt;")); // It's stripped, not escaped
        assert!(result.contains("alert(1)"));

        // Test 2: Special chars in text should be escaped
        let input = "[[Link|Text \" with quotes]]";
        let result = WikiParser::clean_wiki_markup(input);
        assert!(result.contains("Text &quot; with quotes"));
    }
}

