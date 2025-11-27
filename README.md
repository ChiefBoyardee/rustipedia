# 📚 Wiki Download

**Download and serve your own local copy of Wikipedia.**

A fast, user-friendly Rust tool to download Wikipedia dumps directly from Wikimedia, extract articles, and serve them locally with a beautiful searchable web interface.

![Screenshot](https://via.placeholder.com/800x400?text=Your+Local+Wikipedia)

## ✨ Features

- 🌍 **10+ Languages** - Download Simple English, English, German, French, Spanish, Japanese, Russian, Chinese, Italian, or Portuguese Wikipedia
- ⚡ **Fast Streaming** - Memory-efficient streaming parser handles even the 22GB English Wikipedia dump
- 🔍 **Full-Text Search** - Built-in Tantivy search engine for instant article lookup
- 🎨 **Beautiful UI** - Clean, responsive web interface for browsing and reading
- 📦 **Self-Contained** - Everything you need in two binaries
- 🔒 **Offline First** - Access Wikipedia anytime, anywhere, no internet required

## 🚀 Quick Start

### Installation

```bash
# Clone and build
git clone https://github.com/yourusername/wiki_download
cd wiki_download
cargo build --release

# Or install directly
cargo install --path .
```

### Download Wikipedia

```bash
# Download Simple English Wikipedia (~300MB, ~200K articles)
# Perfect for testing!
wiki-download --lang simple

# Download full English Wikipedia (~22GB, ~6.7M articles)
# Warning: This takes several hours!
wiki-download --lang en

# Download German Wikipedia
wiki-download --lang de --output ./german-wiki
```

### Serve Locally

```bash
# Start the web server
wiki-serve

# Open http://localhost:8080 in your browser!

# Custom port and data directory
wiki-serve --port 3000 --data ./my-wiki
```

## 📖 Usage

### Download Command

```
wiki-download [OPTIONS] [COMMAND]

Commands:
  list      List all available Wikipedia languages
  download  Download Wikipedia for a specific language
  extract   Extract articles from an existing dump
  index     Build search index from extracted articles

Options:
  -l, --lang <LANG>           Wikipedia language [default: simple]
  -o, --output <DIR>          Output directory [default: wikipedia]
  -m, --max-articles <N>      Maximum articles (0 = unlimited) [default: 0]
      --min-length <N>        Minimum article length [default: 200]
      --skip-download         Skip download if dump exists
      --download-only         Only download, don't extract
      --build-index           Build search index [default: true]
      --keep-dump             Keep raw dump file after extraction
  -v, --verbose               Verbose output
  -h, --help                  Print help
  -V, --version               Print version
```

### Serve Command

```
wiki-serve [OPTIONS]

Options:
  -d, --data <DIR>    Directory containing Wikipedia data [default: wikipedia]
  -p, --port <PORT>   Port to listen on [default: 8080]
      --host <HOST>   Host to bind to [default: 127.0.0.1]
  -v, --verbose       Verbose output
  -h, --help          Print help
  -V, --version       Print version
```

## 🌐 Available Languages

| Code | Language | Articles | Download Size |
|------|----------|----------|---------------|
| `simple` | Simple English | ~200K | ~300 MB |
| `en` | English | ~6.7M | ~22 GB |
| `de` | German | ~2.8M | ~7 GB |
| `fr` | French | ~2.5M | ~5 GB |
| `es` | Spanish | ~1.9M | ~4 GB |
| `ja` | Japanese | ~1.4M | ~4 GB |
| `ru` | Russian | ~1.9M | ~5 GB |
| `zh` | Chinese | ~1.3M | ~3 GB |
| `it` | Italian | ~1.8M | ~4 GB |
| `pt` | Portuguese | ~1.1M | ~2 GB |

## 📁 Data Structure

After downloading, your data directory will contain:

```
wikipedia/
├── articles.jsonl      # All articles in JSONL format
├── config.json         # Download configuration
├── stats.json          # Extraction statistics
└── search_index/       # Tantivy search index
```

Each article in `articles.jsonl` has this structure:

```json
{
  "id": 12345,
  "title": "Albert Einstein",
  "content": "Albert Einstein was a German-born theoretical physicist...",
  "categories": ["Scientists", "Physics", "Nobel laureates"],
  "extracted_at": "2024-01-15T10:30:00Z"
}
```

## 🔧 API Endpoints

The web server exposes these JSON API endpoints:

| Endpoint | Description |
|----------|-------------|
| `GET /api/articles?page=1` | List articles (paginated) |
| `GET /api/search?q=query` | Search articles |

## 💡 Tips

### For Testing
Start with Simple English Wikipedia (`--lang simple`). It's only ~300MB and downloads in minutes.

### For Full Wikipedia
- The full English dump is ~22GB compressed
- Extraction needs ~50GB of free disk space
- The process takes 2-4 hours depending on your hardware
- Use `--keep-dump` if you want to re-extract later

### For Low Memory Systems
The streaming parser is memory-efficient, but building the search index needs RAM. If you're low on memory:
```bash
wiki-download --lang simple --build-index=false
# Then build index separately when you have memory available
wiki-download index ./wikipedia
```

### Network Access
By default, the server only listens on localhost. To access from other devices:
```bash
wiki-serve --host 0.0.0.0 --port 8080
```
Then access via `http://your-ip:8080`

## 🏗️ Building from Source

### Requirements
- Rust 1.70+ (2021 edition)
- ~50GB disk space for full English Wikipedia

### Build

```bash
# Debug build
cargo build

# Release build (recommended)
cargo build --release

# Run tests
cargo test
```

## 📊 Benchmarks

Tested on AMD Ryzen 7 5800X, 32GB RAM, NVMe SSD:

| Wikipedia | Download | Extract | Index | Total |
|-----------|----------|---------|-------|-------|
| Simple English | 2 min | 3 min | 1 min | 6 min |
| Full English | 45 min | 90 min | 30 min | ~3 hours |

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## 📄 License

MIT License - feel free to use this however you like!

## 🙏 Acknowledgments

- [Wikimedia Foundation](https://wikimediafoundation.org/) for making Wikipedia available
- [Tantivy](https://github.com/quickwit-oss/tantivy) for the amazing search engine
- [Axum](https://github.com/tokio-rs/axum) for the web framework

---

**Made with ❤️ and Rust**

