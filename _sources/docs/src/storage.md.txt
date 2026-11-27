# Browser storage and write behavior

OPFS gives a web application a private filesystem managed by the browser. The root belongs to the current origin and browser profile, so another origin or profile has different files. OPFS does not expose arbitrary files from the user's operating system.

The Rust implementation calls browser APIs through WebAssembly. JavaScript and Rust callers use the same async operations. Blocking Rust file handles are unsupported because browser I/O completes through the JavaScript event loop. The Python adapter uses Pyodide's JSPI support to suspend synchronous fsspec calls while that event loop processes I/O. On a server it warns and uses a temporary local directory; it cannot access a browser profile's files.

## Why writes require commit

A writer uses the browser's writable file stream to prepare replacement bytes. Commit closes that stream and publishes the replacement. Aborting or dropping an unfinished writer discards the replacement, preserving the previous contents of an existing file.

Opening a writer for a new path may create an empty file immediately. Aborting that writer can leave the empty entry. The filesystem therefore does not provide atomic create-if-absent or the guarantees needed to publish immutable table manifests.

Commit also has a cancellation boundary: once the browser has started closing the writable stream, dropping the Rust future cannot guarantee that publication stops. Applications need to account for an uncertain result if they stop waiting at that point.

Concurrent writers follow the browser's behavior. This backend adds no compare-and-swap, writer fencing, conflict detection, or cross-file transactions. A chunked writer avoids collecting the complete input in application code; it does not impose a hard limit on the browser's internal buffers. `pipeFile` accepts a complete input buffer, and `cpFile` currently buffers the complete source.

## Persistence belongs to the browser

Files persist across page reloads, but browser quotas, eviction, profile deletion, and clearing site data affect storage lifetime. A successful persistence request reduces eviction risk; it does not protect against explicit deletion. [Requesting persistence](browser.md) and maintaining recoverable copies are application responsibilities.

The current browser test suite runs against Chromium and checks reload persistence, ranges, read handles, directory behavior, and write commit/abort. Other browsers and worker-only synchronous access handles are outside that tested scope.
