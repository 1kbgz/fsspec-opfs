# fsspec-opfs

Async browser storage backed by the Origin Private File System (OPFS).
The Rust crate implements `fsspec_rs::AsyncFileSystem` and `AsyncFsFile` for
`wasm32-unknown-unknown`. The JavaScript package exposes the same backend with
TypeScript declarations.

## Documentation

- [How to read and write browser files](docs/src/browser.md)
- [JavaScript and Rust API](docs/src/api.md)
- [Browser storage and write behavior](docs/src/storage.md)
- [How to build and test changes](docs/src/development.md)

## JavaScript

```typescript
import { FileSystem } from "fsspec-opfs";

const fs = await FileSystem.connect();
await fs.mkdir("tables", true);
await fs.pipeFile("opfs://tables/example", new Uint8Array([1, 2, 3]));
const bytes = await fs.catFile("tables/example", 1); // [2, 3]
const entries = await fs.ls("tables");

const writer = await fs.writer("tables/example");
try {
  await writer.write(new Uint8Array([4, 5]));
  await writer.write(new Uint8Array([6]));
  await writer.commit();
} finally {
  await writer.abort(); // Safe after commit; otherwise discards the write.
  writer.free();
}

const reader = await fs.open("tables/example");
await reader.seek(1);
const chunk = await reader.read(2);
await reader.close();
reader.free();
fs.free();
```

`catFile` accepts optional start and end byte offsets, including negative offsets
from the end. `info`, `ls`, `mkdir`, `rmdir`, `rmFile`, and `cpFile` operate on
paths relative to the origin's OPFS root. `mkdir(path, true)` creates parents.
`rmdir` removes empty directories. Errors expose a `code` such as `not_found`,
`permission_denied`, `is_a_directory`, or `invalid`.

Serve the package's `dist/pkg` directory alongside its `dist/esm` or `dist/cdn`
directory so the WASM URL resolves. The CDN entry is an ES module.

## Rust

```toml
[dependencies]
fsspec_opfs = "0.1"
fsspec_rs = { version = "0.4.1", default-features = false }
```

```rust
use fsspec_opfs::Opfs;
use fsspec_rs::AsyncFileSystem;

let fs = Opfs::new().await?;
fs.mkdir("tables", true).await?;
fs.pipe_file("tables/example", &[1, 2, 3]).await?;
let bytes = fs.cat_file("tables/example", None, None).await?;
```

The crate is browser-only. Native Python distributions contain browser assets;
they do not provide native Python access to OPFS. Blocking filesystem handles,
worker-only synchronous access handles, and registry integration are not provided.

## Write behavior and storage lifetime

A writer publishes bytes only when `commit()` completes. `abort()`, Rust `Drop`,
and JavaScript `free()` discard uncommitted replacement bytes. Explicitly abort
writers in application cleanup; JavaScript garbage collection is not timely.
Once commit has started, dropping its future cannot guarantee cancellation.

Opening a new path for writing can create an empty file before commit. This backend
does not claim atomic create-if-absent, conditional writes, conflict detection, or
manifest publication guarantees. Concurrent writes follow browser behavior.
`pipeFile` buffers its input; use a writer for chunked output. `cpFile` currently
buffers the source file.

OPFS requires a secure context (HTTPS or localhost), a supporting browser, and
storage permission. Files persist across reloads within the same origin and
browser profile. Storage quotas, browser eviction, and clearing site data can
remove them. Applications needing persistence should request it through
`navigator.storage.persist()` and inspect the result.

## Development

```bash
rustup target add wasm32-unknown-unknown
pnpm --dir js install
pnpm --dir js setup
pnpm --dir js exec playwright install chromium
pnpm --dir js build
pnpm --dir js test:js
cargo clippy -p fsspec_opfs -p fsspec_opfs_js --target wasm32-unknown-unknown -- -D warnings
```

Browser tests exercise real OPFS, including page reloads, byte ranges, async read
handles, file and directory errors, and committed and aborted writes.

> [!NOTE]
> This library was generated using [copier](https://copier.readthedocs.io/en/stable/) from the [Base Python Project Template repository](https://github.com/python-project-templates/base).
