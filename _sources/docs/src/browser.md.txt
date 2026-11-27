# How to read and write browser files

Use a browser with OPFS support over HTTPS or localhost. The examples run in a browser ES module; Node.js cannot access the browser's private storage.

## Build and serve the JavaScript package

From a checkout of this repository, build the browser assets:

```bash
rustup target add wasm32-unknown-unknown
pnpm --dir js install --frozen-lockfile
pnpm --dir js setup
pnpm --dir js build
python -m http.server 8000 --directory js
```

Open `http://localhost:8000` and run the following snippets in the browser console. Keep `dist/pkg` alongside `dist/cdn`: the module loads its WASM binary and JavaScript support files from that directory.

```javascript
const { FileSystem } = await import("/dist/cdn/index.js");
const fs = await FileSystem.connect();
await fs.mkdir("examples", true);
await fs.pipeFile("examples/message.txt", new TextEncoder().encode("hello"));
new TextDecoder().decode(await fs.catFile("examples/message.txt"));
// "hello"
```

Reload the page, import the module, and connect again. `catFile` reads the saved file without another write.

For an application using the built package, import `FileSystem` from `fsspec-opfs`. Configure deployment to preserve the same `dist/esm` and `dist/pkg` layout, including the WASM binary and `pkg/snippets` directory.

## Write chunks and publish explicitly

Open a writer when input arrives in chunks. Await each operation before starting the next operation on the same handle.

```javascript
const writer = await fs.writer("examples/chunks.bin");
try {
  await writer.write(new Uint8Array([1, 2]));
  await writer.write(new Uint8Array([3]));
  await writer.commit();
} finally {
  await writer.abort();
  writer.free();
}
```

Call `commit()` to publish the replacement bytes. `abort()` discards an unfinished replacement and is safe after commit. See [write behavior](storage.md) for the empty-file and concurrent-write limitations.

## Read a range or use a handle

Read bytes 1 through 2 with an exclusive end offset:

```javascript
await fs.catFile("examples/chunks.bin", 1, 3); // Uint8Array [2, 3]
await fs.catFile("examples/chunks.bin", -1); // Uint8Array [3]
```

For repeated reads, use a reader and close it in cleanup:

```javascript
const reader = await fs.open("examples/chunks.bin");
try {
  await reader.seek(1);
  const bytes = await reader.read(2);
  console.log(bytes);
} finally {
  await reader.close();
  reader.free();
}
```

When finished, call `fs.free()` to release its WASM allocation. This does not delete stored files.

## Request persistent storage

Request persistence through the browser and handle a declined request in the application:

```javascript
const persistent = await navigator.storage.persist();
const { usage, quota } = await navigator.storage.estimate();
console.log({ persistent, usage, quota });
```

Persistence does not prevent users from clearing site data. Keep recoverable copies of data that must outlast the browser profile.
