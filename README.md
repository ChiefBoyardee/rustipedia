# Rustipedia 📚

> Your personal Wikipedia server - Access the world's knowledge offline, anytime, anywhere.

**Rustipedia** lets you download and host Wikipedia locally on your computer. Browse millions of articles with full-text search, all without an internet connection!

## ✨ Features

- ⚡ **Fast Streaming** - Memory-efficient streaming parser handles even the 22GB English Wikipedia dump
- 🔍 **Full-Text Search** - Built-in Tantivy search engine for instant article lookup
- 🎨 **Beautiful UI** - Clean, responsive web interface for browsing and reading
- 📦 **Self-Contained** - Everything you need in simple command-line tools
- 🔒 **Offline First** - Access Wikipedia anytime, anywhere, no internet required
- 🌐 **Multi-Language** - Support for 100+ Wikipedia languages

---

## 🚀 Quick Start

### Step 1: Install Rustipedia

Choose the installation method for your operating system:

#### 📦 **Option A: Download Pre-Built Binaries (Recommended)**

Download the latest release for your platform from the [Releases page](https://github.com/ChiefBoyardee/rustipedia/releases):

##### **Windows**

1. Download `rustipedia-windows-x64.zip`
2. Extract the ZIP file to a folder (e.g., `C:\rustipedia`)
3. Add the folder to your PATH, or run the executables directly:
   ```powershell
   # Navigate to the extracted folder
   cd C:\rustipedia
   
   # Run the download tool
   .\rustipedia-download.exe --lang simple
   ```

##### **macOS**

1. Download `rustipedia-macos-x64.tar.gz` (Intel) or `rustipedia-macos-arm64.tar.gz` (Apple Silicon)
2. Extract and install:
   ```bash
   # Extract the archive
   tar -xzf rustipedia-macos-*.tar.gz
   cd rustipedia
   
   # Run the installer (requires sudo)
   sudo ./install.sh
   ```

##### **Linux**

1. Download `rustipedia-linux-x64.tar.gz`
2. Extract and install:
   ```bash
   # Extract the archive
   tar -xzf rustipedia-linux-x64.tar.gz
   cd rustipedia
   
   # Run the installer (requires sudo)
   sudo ./install.sh
   ```

The installer will copy the binaries to `/usr/local/bin` and run the setup wizard to help you get started.

#### 🛠️ **Option B: Build from Source**

If you prefer to build from source or pre-built binaries aren't available for your platform, see the [Building from Source](#-building-from-source) section below.

---

### Step 2: Download Wikipedia

Once installed, download a Wikipedia dump. We recommend starting with **Simple English Wikipedia** for testing:

```bash
# Download Simple English Wikipedia (~300MB, ~200K articles)
# Perfect for testing - downloads in minutes!
rustipedia-download --lang simple
```

**Other popular options:**

```bash
# Full English Wikipedia (~22GB, ~6.7M articles)
# Warning: This is large and takes several hours!
rustipedia-download --lang en

# German Wikipedia (~7GB, ~2.8M articles)
rustipedia-download --lang de

# Spanish Wikipedia (~4GB, ~1.9M articles)
rustipedia-download --lang es
```

The download process will:
1. Download the Wikipedia dump file
2. Extract and parse all articles
3. Build a full-text search index
4. Save everything to the `wikipedia/` folder (or your custom location)

---

### Step 3: Start the Web Server

Launch the local web server to browse Wikipedia:

```bash
rustipedia-serve
```

Then open your browser to **http://localhost:8080** and start exploring!

**Custom options:**

```bash
# Use a different port
rustipedia-serve --port 3000

# Use a custom data directory
rustipedia-serve --data ./my-wiki

# Bind to all network interfaces (access from other devices)
rustipedia-serve --host 0.0.0.0
```

---

## 📖 Detailed Usage

### Download Command Reference

```
rustipedia-download [OPTIONS]

Options:
  -l, --lang <LANG>           Wikipedia language code [default: simple]
  -o, --output <DIR>          Output directory [default: wikipedia]
  -m, --max-articles <N>      Maximum articles to extract (0 = unlimited) [default: 0]
      --min-length <N>        Minimum article length in characters [default: 200]
      --skip-download         Skip download if dump file already exists
      --download-only         Only download the dump, don't extract
      --build-index           Build search index after extraction [default: true]
      --keep-dump             Keep the raw dump file after extraction
  -v, --verbose               Show detailed progress information
  -h, --help                  Print help information
  -V, --version               Print version information
```

**Advanced Commands:**

```bash
# List all available Wikipedia languages
rustipedia-download list

# Download only (don't extract)
rustipedia-download --lang en --download-only

# Extract from an existing dump file
rustipedia-download extract --input enwiki-latest-pages-articles.xml.bz2

# Build search index from existing articles
rustipedia-download index --data ./wikipedia
```

### Serve Command Reference

```
rustipedia-serve [OPTIONS]

Options:
  -d, --data <DIR>    Directory containing Wikipedia data [default: wikipedia]
  -p, --port <PORT>   Port to listen on [default: 8080]
      --host <HOST>   Host address to bind to [default: 127.0.0.1]
  -v, --verbose       Show detailed server logs
  -h, --help          Print help information
  -V, --version       Print version information
```

---

## 🌐 Available Languages

Rustipedia supports 100+ Wikipedia languages. Here are the most popular:

| Code | Language | Articles | Download Size | Extracted Size |
|------|----------|----------|---------------|----------------|
| `simple` | Simple English | ~200K | ~300 MB | ~1 GB |
| `en` | English | ~6.7M | ~22 GB | ~50 GB |
| `de` | German | ~2.8M | ~7 GB | ~20 GB |
| `fr` | French | ~2.5M | ~5 GB | ~15 GB |
| `es` | Spanish | ~1.9M | ~4 GB | ~12 GB |
| `ja` | Japanese | ~1.4M | ~4 GB | ~10 GB |
| `ru` | Russian | ~1.9M | ~5 GB | ~15 GB |
| `zh` | Chinese | ~1.3M | ~3 GB | ~8 GB |
| `it` | Italian | ~1.8M | ~4 GB | ~12 GB |
| `pt` | Portuguese | ~1.1M | ~2 GB | ~6 GB |

**To see all available languages:**

```bash
rustipedia-download list
```

---

## 📁 Data Structure

After downloading, your data directory will look like this:

```
wikipedia/
├── articles.jsonl      # All articles in JSONL format (one article per line)
├── config.json         # Download configuration and metadata
├── stats.json          # Extraction statistics
└── search_index/       # Tantivy full-text search index
    ├── meta.json
    └── [index files]
```

**Article Format:**

Each line in `articles.jsonl` contains a JSON object:

```json
{
  "id": 12345,
  "title": "Albert Einstein",
  "content": "Albert Einstein was a German-born theoretical physicist...",
  "timestamp": "2024-01-15T10:30:00Z"
}
```

---

## 💡 Tips & Best Practices

### 🧪 For Testing

**Start with Simple English Wikipedia** (`--lang simple`):
- Only ~300MB download
- ~200,000 articles
- Downloads and indexes in minutes
- Perfect for testing the setup

### 🌍 For Full Wikipedia

**The full English Wikipedia is massive:**
- ~22GB compressed download
- ~50GB after extraction and indexing
- 6.7+ million articles
- Takes 2-4 hours to download and process (depending on your internet and hardware)

**Recommendations:**
- Ensure you have at least **60GB of free disk space**
- Use `--keep-dump` if you want to re-extract later without re-downloading
- Consider using `--max-articles` to limit the number of articles for testing

### 💾 For Low Disk Space

If you're limited on disk space:

```bash
# Download only the first 100,000 articles
rustipedia-download --lang en --max-articles 100000

# Don't keep the raw dump file after extraction
rustipedia-download --lang en
# (dump is deleted by default)
```

### 🧠 For Low Memory Systems

The streaming parser is memory-efficient, but building the search index requires RAM:
- **Simple Wikipedia**: ~1GB RAM recommended
- **Full English Wikipedia**: ~4-8GB RAM recommended

If you're low on memory, you can skip index building and add it later:

```bash
# Download and extract without building index
rustipedia-download --lang en --build-index=false

# Build the index later when you have more resources
rustipedia-download index --data ./wikipedia
```

### 🔧 Advanced: Link Validation

Rustipedia includes a link validator to check for broken internal links:

```bash
# Validate all links in your Wikipedia data
rustipedia-link-validator --data ./wikipedia

# This will identify articles that link to non-existent pages
```

---

## 🛠️ Building from Source

If you want to build Rustipedia from source or contribute to development:

### Prerequisites

- **Rust 1.70 or later** - Install from [rustup.rs](https://rustup.rs/)
- **Git** - For cloning the repository
- **~50GB disk space** - If you plan to download full English Wikipedia

### Build Instructions

```bash
# Clone the repository
git clone https://github.com/ChiefBoyardee/rustipedia
cd rustipedia

# Build in release mode (recommended)
cargo build --release

# The binaries will be in target/release/:
# - rustipedia-download
# - rustipedia-serve
# - rustipedia-link-validator
# - rustipedia-setup

# Optionally, install to your system
cargo install --path .
```

### Development Build

```bash
# Build in debug mode (faster compilation, slower runtime)
cargo build

# Run tests
cargo test

# Run with verbose logging
RUST_LOG=debug cargo run --bin rustipedia-serve
```

### Release Optimization

The release build is optimized for performance:

```toml
[profile.release]
opt-level = 3        # Maximum optimization
lto = true           # Link-time optimization
codegen-units = 1    # Better optimization, slower compile
```

---

## 🐛 Troubleshooting

### "Command not found" after installation

**On Linux/macOS:**
- Make sure `/usr/local/bin` is in your PATH
- Try running with the full path: `/usr/local/bin/rustipedia-download`

**On Windows:**
- Add the rustipedia folder to your PATH environment variable
- Or run the executables with the full path

### Download fails or times out

- Check your internet connection
- Try again with `--skip-download` if the dump file was partially downloaded
- Some Wikipedia dumps are very large and may take hours to download

### Out of disk space during extraction

- Check available disk space with `df -h` (Linux/macOS) or `dir` (Windows)
- The full English Wikipedia needs ~50GB after extraction
- Use `--max-articles` to limit the number of articles

### Search not working

- Make sure the search index was built (check for `search_index/` folder)
- Rebuild the index: `rustipedia-download index --data ./wikipedia`

---

## 📊 Performance Benchmarks

Tested on AMD Ryzen 7 5800X, 32GB RAM, NVMe SSD:

| Wikipedia | Download | Extract | Index | Total | Final Size |
|-----------|----------|---------|-------|-------|------------|
| Simple EN | 2 min | 3 min | 1 min | ~6 min | ~1 GB |
| English | 45 min | 90 min | 30 min | ~3 hrs | ~50 GB |
| German | 20 min | 40 min | 15 min | ~75 min | ~20 GB |

*Times vary based on internet speed and hardware.*

---

## 📄 License

This project is licensed under the MIT License - see the LICENSE file for details.

---

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

---

## 🙏 Acknowledgments

- **Wikipedia** - For making knowledge freely available
- **Wikimedia Foundation** - For hosting and maintaining Wikipedia dumps
- **Tantivy** - For the excellent full-text search engine
- **Rust Community** - For amazing tools and libraries

---

**Made with ❤️ and Rust**

*Rustipedia is not affiliated with or endorsed by the Wikimedia Foundation.*
