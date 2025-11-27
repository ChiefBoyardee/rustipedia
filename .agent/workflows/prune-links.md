---
description: How to prune broken links from the wiki database
---

# Pruning Broken Links

This workflow removes internal wiki links that point to articles not present in your local database. This is useful for smaller Wikipedia dumps (like Simple English) where many links point to articles that only exist in the full English Wikipedia.

## Steps

1. **Ensure you have downloaded and extracted a wiki dump**
   ```bash
   cargo run --bin wiki-download -- --lang simple
   ```

2. **Run the prune command**
   This will scan all articles and rewrite the database to remove broken links.
   ```bash
   cargo run --release --bin wiki-download -- prune
   ```
   *Note: This process can take several minutes depending on the database size.*

3. **Re-build the search index**
   Since the article content has changed, you must update the search index.
   ```bash
   cargo run --release --bin wiki-download -- index
   ```

4. **Restart the server**
   ```bash
   cargo run --release --bin wiki-serve
   ```

## Verification

After pruning, you can verify that broken links are gone:
1. Navigate to an article that previously had broken links (e.g., "Professional wrestling").
2. Check links like "desktop computers" (which pointed to "PC").
3. They should now be plain text instead of clickable links.
