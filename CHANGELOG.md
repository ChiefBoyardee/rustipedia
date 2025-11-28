# Changelog

All notable changes to Rustipedia will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.2] - 2025-11-28

### 🌟 New Features

#### Branding & Customization
- **Custom Logo Support**: Users can now upload their own logo via the settings page to replace the default Rustipedia branding.
- **Embedded Default Logo**: The default logo is now embedded directly into the binary, removing external file dependencies.
- **Settings UI Update**: Added a new "Branding" section to the settings page for managing the server logo.

### 🐛 Bug Fixes

#### Installer & Setup
- **Windows Service Registration**: The MSI installer now correctly registers `rustipedia-serve` as a Windows Service.
- **Service Configuration**: The setup wizard now gracefully handles existing services, reconfiguring them instead of failing.
- **Configuration Priority**: The server now correctly prioritizes settings from `config.json` (port, host) over default values.

#### Other Fixes
- **Git Ignore**: Updated `.gitignore` to exclude user-uploaded `custom_logo.png` files.
- **Dependencies**: Enabled `multipart` feature in `axum` to support file uploads.

### 📚 Documentation
- **README**: Updated with the new logo and branding.

## [0.2.0] - 2025-11-27

### 🔒 Security Enhancements

This release focuses heavily on security hardening to ensure Rustipedia is safe for public use.

#### Added
- **SHA256 Checksum Verification**: All Wikipedia dumps are now verified against checksums before extraction
- **Download Size Limits**: Enforced 100GB maximum download size to prevent disk exhaustion
- **Disk Space Checks**: Validates available disk space before downloading (requires 2x download size)
- **Content Security Policy (CSP)**: Strict CSP headers on all web responses
- **Rate Limiting**: IP-based rate limiting (50 req/s) on all API endpoints using `tower-governor`
- **Request Timeouts**: 30-second timeout on all HTTP requests to prevent slowloris attacks
- **Input Validation**: 
  - Search queries limited to 200 characters
  - Article titles sanitized (control characters removed, max 255 chars)
  - Article size limited to 10MB during extraction
- **XSS Prevention**: HTML escaping for all user-generated content in wiki links
- **File Permissions**: Explicit file permissions (0644) set on Unix systems
- **Secure Random Number Generation**: Using cryptographically secure RNG for random article selection
- **Security Headers**: Added X-Content-Type-Options, X-Frame-Options, X-XSS-Protection headers

#### Security Fixes
- Fixed Stored XSS vulnerability in wiki markup parser
- Fixed path traversal vulnerability in output directory handling
- Disabled XML External Entity (XXE) expansion in XML parser
- Implemented proper URL encoding for link href attributes

### 🎨 User Experience Improvements

#### Added
- **Interactive Setup Wizard**: New `rustipedia-setup` command for guided first-time setup
- **Link Validation Tool**: New `rustipedia-link-validator` to identify broken internal links
- **Improved Progress Indicators**: Better progress bars and status messages during download/extraction
- **Enhanced Error Messages**: More helpful error messages with actionable suggestions

#### Changed
- **Redesigned README**: Comprehensive documentation with installation guides for all platforms
- **Better CLI Help**: Improved command-line help text and examples
- **Streamlined Installation**: Platform-specific installation scripts (install.sh, install_mac.sh)

### 🐛 Bug Fixes

#### Fixed
- Fixed duplicate timeout configuration in HTTP client
- Fixed duplicate route registration in web server
- Removed unused imports and cleaned up code
- Fixed file permissions not being set on created files
- Corrected random article selection to use proper CSPRNG

### 📚 Documentation

#### Added
- **SECURITY_AUDIT_REPORT.md**: Comprehensive security audit report
- **CHANGELOG.md**: This changelog file
- **Enhanced README.md**: Complete rewrite with:
  - Platform-specific installation instructions
  - Troubleshooting section
  - Performance benchmarks
  - Advanced usage examples
  - Contributing guidelines

### 🔧 Technical Improvements

#### Added
- New dependencies:
  - `fs2` for disk space checking
  - `tower_governor` for rate limiting
  - `rand` for secure random number generation
  - `sha2` and `hex` for checksum verification

#### Changed
- Upgraded to `tower-http` 0.5 with additional features (timeout, set-header)
- Improved error handling throughout the codebase
- Better resource cleanup and memory management

### 🧪 Testing

#### Added
- Unit test for XSS prevention in wiki markup parser
- Validation of HTML escaping in link generation

---

## [0.1.0] - 2024-11-26

### Initial Release

#### Added
- Wikipedia dump download from Wikimedia
- XML parsing and article extraction
- Full-text search using Tantivy
- Web server with clean UI
- Support for 10+ major Wikipedia languages
- JSONL article storage format
- Progress bars for long-running operations
- Category extraction from articles
- Redirect handling
- Wiki markup cleaning and conversion

#### Features
- Download Wikipedia dumps for multiple languages
- Extract and parse articles from XML dumps
- Build searchable index
- Serve articles via web interface
- Search functionality
- Random article feature
- Article browsing and pagination

---

## Release Notes

### Upgrading from 0.1.0 to 0.2.0

**No breaking changes** - existing data directories are fully compatible.

However, we **strongly recommend** re-downloading Wikipedia dumps to take advantage of checksum verification:

```bash
# Backup your existing data (optional)
mv wikipedia wikipedia-backup

# Re-download with checksum verification
rustipedia-download --lang simple
```

### Security Notice

Version 0.2.0 addresses several security vulnerabilities:
- **Stored XSS** in wiki markup parser (CRITICAL)
- **Path Traversal** in file operations (HIGH)
- **Missing Input Validation** (MEDIUM)

**If you are running a publicly accessible instance of Rustipedia, upgrading to 0.2.0 is strongly recommended.**

---

## Roadmap

### Planned for 0.3.0
- [ ] Image support (download and serve Wikipedia images)
- [ ] Article history and versioning
- [ ] Export functionality (PDF, EPUB)
- [ ] Mobile-optimized UI
- [ ] Dark mode
- [ ] Fuzzing tests for parsers
- [ ] Automated security scanning in CI/CD

### Future Considerations
- Multi-language search
- Wikidata integration
- Offline mobile apps
- P2P distribution of dumps
- Incremental updates

---

[0.2.0]: https://github.com/ChiefBoyardee/rustipedia/releases/tag/v0.2.0
[0.1.0]: https://github.com/ChiefBoyardee/rustipedia/releases/tag/v0.1.0
