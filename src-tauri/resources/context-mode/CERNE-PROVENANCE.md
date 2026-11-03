# Context Mode reference and native port

Upstream: https://github.com/mksglu/context-mode
Commit: 8de5f73af9365d459bdfc564c045948cc3fc488b
Version: 1.0.169
Copyright: 2026 Mert Koseoglu
License: Elastic License 2.0 (see LICENSE)

The original tracked source is preserved as a development reference. Cerne's
context management port lives in src-tauri/src/context_economy*. It implements
native SQLite FTS5, chunking/search, session capture, snapshots, output archival,
fetch/cache, execution, metrics and local slash commands. Native code derived
from the reference retains upstream attribution and Elastic License 2.0.

The old cerne-entry.ts adapter, generated JavaScript bundle and private Node
build scripts are development references only. They are NOT packaged or
started by the native port. The installer distributes LICENSE and this notice.
JavaScript analysis uses the Boa Rust engine in a child of the Cerne executable.
SQLite is linked into Cerne via rusqlite. Neither requires system Node.

This is not a literal port of all Node APIs, third-party editor adapters,
background runtime detach behavior or the hosted organizational Insight service.
The event classifier is adapted to Cerne; complete captured content remains
searchable regardless of classification. See docs/context-economy.md and
docs/context-economy-rust-plan.md for contracts and validation boundaries.

Additional component notices are shipped in context-economy-notices.
