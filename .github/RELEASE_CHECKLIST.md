# Release Checklist for v0.2.0

## Pre-Release Tasks

### Code Quality
- [x] All tests passing (`cargo test`)
- [x] No compiler warnings (`cargo clippy`)
- [x] Security audit completed (`cargo audit`)
- [x] Code formatted (`cargo fmt`)
- [x] Documentation updated

### Version Updates
- [x] Version bumped in `Cargo.toml` (0.1.0 → 0.2.0)
- [x] `CHANGELOG.md` created and updated
- [x] `README.md` updated with new features
- [x] `SECURITY_AUDIT_REPORT.md` created

### Testing
- [x] Manual testing on Windows
- [ ] Manual testing on macOS
- [ ] Manual testing on Linux
- [x] Download and extraction tested (Simple English)
- [ ] Download and extraction tested (Full English)
- [x] Web server tested
- [x] Search functionality tested
- [x] Security features tested (rate limiting, CSP, etc.)

### Documentation
- [x] README has installation instructions for all platforms
- [x] README has troubleshooting section
- [x] All CLI commands documented
- [x] Security features documented
- [x] CHANGELOG follows Keep a Changelog format

## Release Tasks

### Git
- [ ] Commit all changes
- [ ] Create git tag: `git tag -a v0.2.0 -m "Release v0.2.0"`
- [ ] Push commits: `git push origin master`
- [ ] Push tags: `git push origin v0.2.0`

### GitHub Release
- [ ] Create GitHub release from tag v0.2.0
- [ ] Copy CHANGELOG entry to release notes
- [ ] Highlight security fixes in release notes
- [ ] Build release binaries for all platforms:
  - [ ] Windows x64 (`rustipedia-windows-x64.zip`)
  - [ ] macOS Intel (`rustipedia-macos-x64.tar.gz`)
  - [ ] macOS Apple Silicon (`rustipedia-macos-arm64.tar.gz`)
  - [ ] Linux x64 (`rustipedia-linux-x64.tar.gz`)
- [ ] Upload binaries to GitHub release
- [ ] Mark as "Latest Release"

### Post-Release
- [ ] Announce on project channels
- [ ] Update project website (if applicable)
- [ ] Monitor for issues
- [ ] Respond to user feedback

## Build Commands

### Windows (x64)
```powershell
cargo build --release --target x86_64-pc-windows-msvc
# Package: zip rustipedia-windows-x64.zip target/x86_64-pc-windows-msvc/release/*.exe install.sh README.md LICENSE
```

### macOS (Intel)
```bash
cargo build --release --target x86_64-apple-darwin
# Package: tar -czf rustipedia-macos-x64.tar.gz -C target/x86_64-apple-darwin/release rustipedia-* install_mac.sh README.md LICENSE
```

### macOS (Apple Silicon)
```bash
cargo build --release --target aarch64-apple-darwin
# Package: tar -czf rustipedia-macos-arm64.tar.gz -C target/aarch64-apple-darwin/release rustipedia-* install_mac.sh README.md LICENSE
```

### Linux (x64)
```bash
cargo build --release --target x86_64-unknown-linux-gnu
# Package: tar -czf rustipedia-linux-x64.tar.gz -C target/x86_64-unknown-linux-gnu/release rustipedia-* install.sh README.md LICENSE
```

## Release Notes Template

```markdown
# Rustipedia v0.2.0 - Security Hardening Release

## 🔒 Security Notice

This release addresses several critical security vulnerabilities. **Upgrading is strongly recommended**, especially if you're running a publicly accessible instance.

### Fixed Vulnerabilities
- **CRITICAL**: Stored XSS in wiki markup parser
- **HIGH**: Path traversal in file operations
- **MEDIUM**: Missing input validation

## What's New

### Security Enhancements
- SHA256 checksum verification for all downloads
- Rate limiting (50 requests/second per IP)
- Content Security Policy (CSP) headers
- Request timeouts to prevent DoS attacks
- Input validation for search queries and article content
- Secure random number generation

### User Experience
- Interactive setup wizard
- Link validation tool
- Comprehensive installation guides
- Better error messages and progress indicators

### Documentation
- Complete README rewrite
- Security audit report
- Detailed changelog

## Download

Choose the appropriate binary for your platform:
- **Windows**: `rustipedia-windows-x64.zip`
- **macOS (Intel)**: `rustipedia-macos-x64.tar.gz`
- **macOS (Apple Silicon)**: `rustipedia-macos-arm64.tar.gz`
- **Linux**: `rustipedia-linux-x64.tar.gz`

Or build from source with Rust 1.70+

## Upgrading

No breaking changes - existing data directories are fully compatible. However, we recommend re-downloading Wikipedia dumps to benefit from checksum verification.

See [CHANGELOG.md](CHANGELOG.md) for full details.
```

## Notes

- This is a security-focused release
- Emphasize the importance of upgrading in all communications
- Consider creating a security advisory on GitHub for the XSS vulnerability
- Test thoroughly before releasing binaries
