# How to use OPFS from Python

`fsspec.filesystem("opfs")` uses browser OPFS under Pyodide. On native Python it emits a `RuntimeWarning` and uses an isolated temporary local directory for that instance. Browser setup failures raise an error; they never select the temporary fallback.

## Run normal fsspec code in Pyodide

Use a browser with JavaScript Promise Integration (JSPI) and call Python through `runPythonAsync` or `callPromising`. Pyodide's [`run_sync`](https://pyodide.org/en/stable/usage/api/python-api/ffi.html#pyodide.ffi.run_sync) suspends the Python call while OPFS completes on the JavaScript event loop. `runPython` does not provide this suspension and is rejected.

Build the browser assets and pure Python wheel from a checkout:

```bash
pnpm --dir js install --frozen-lockfile
pnpm --dir js setup
rustup target add wasm32-unknown-unknown
python -m pip install build hatchling hatch-js hatch-rs
make dist-py-browser
python -m http.server 8000
```

The wheel is written to `dist/browser`. It contains the Python adapter and browser assets, with no native extension. Keep the `js/dist/cdn` and `js/dist/pkg` directories together when serving the JavaScript package.

In a page with an initialized Pyodide instance, install the wheel and register the JavaScript module:

```javascript
await pyodide.loadPackage("micropip");
await pyodide.runPythonAsync(`
import micropip
await micropip.install("http://localhost:8000/dist/browser/fsspec_opfs-0.1.0-py3-none-any.whl")
`);
pyodide.registerJsModule(
  "fsspec_opfs_js",
  await import("http://localhost:8000/js/dist/cdn/index.js"),
);

await pyodide.runPythonAsync(`
import fsspec

with fsspec.filesystem("opfs") as fs:
    fs.makedirs("tables", exist_ok=True)
    with fs.open("opfs://tables/message.txt", "w") as file:
        file.write("hello from Python")
    with fs.open("tables/message.txt", "r") as file:
        print(file.read())
`);
```

Serve the page from the same origin as these URLs, or configure cross-origin access for the deployed assets. The module registration makes the existing Rust/WASM backend available to Python; it is performed once per Pyodide instance. Files share the origin's storage with JavaScript callers and persist across page reloads.

For binary data, use `rb` and `wb`. Text mode uses fsspec's text wrapper. Browser append, exclusive creation, and read/write update modes are unsupported. Closing a write file publishes its bytes. An exception in a binary-file context discards its pending replacement; use `fs.transaction` when an enclosing text wrapper must also abort on error. Multi-file transactions publish files individually and are not atomic across files.

```python
with fsspec.filesystem("opfs") as fs:
    fs.makedirs("tables", exist_ok=True)
    with fs.transaction:
        with fs.open("tables/part.bin", "wb") as file:
            file.write(b"first chunk")
            file.write(b"second chunk")
```

Open writers can create empty entries before publication. See [storage behavior](storage.md) for commit, abort, and concurrency limits. Close files before closing their filesystem.

## Use the native fallback

Install the package in native Python and retain one filesystem instance for related operations:

```python
import fsspec

with fsspec.filesystem("opfs") as fs:
    print(fs.local_root)  # Temporary directory, with a RuntimeWarning at construction.
    fs.pipe_file("example", b"temporary bytes")
    assert fs.cat_file("example") == b"temporary bytes"
```

Each instance gets a different temporary directory. Closing it deletes that directory; garbage collection also schedules temporary-directory cleanup. Native files are not shared with a browser or another filesystem instance. Keep important data elsewhere.

OPFS paths remain relative to that root, including paths beginning with `/`. Dot components, backslashes, NUL characters, and symbolic links resolving outside the root are rejected. This path confinement does not protect against another native process modifying directories during an operation.
