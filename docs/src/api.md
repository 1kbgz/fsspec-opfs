# JavaScript and Rust API

## Paths

Paths are relative to the current origin's OPFS root. The `opfs://` prefix, leading and trailing slashes, and repeated slashes are accepted. `""`, `"/"`, and `"opfs://"` refer to the root. Dot components (`.` and `..`), backslashes, and NUL characters are rejected. Hidden names such as `.cache` are allowed.

## JavaScript

The package exports `FileSystem`, `Reader`, `Writer`, and the TypeScript `FileInfo` interface. Browser I/O methods return promises. Operations on an individual reader or writer must be awaited sequentially.

### FileSystem

| Method                         | Result and behavior                                                                                                            |
| ------------------------------ | ------------------------------------------------------------------------------------------------------------------------------ |
| `FileSystem.connect()`         | `Promise<FileSystem>` rooted at the current origin. Initializes WASM on the first connection.                                  |
| `catFile(path, start?, end?)`  | `Promise<Uint8Array>`. End is exclusive; negative offsets count from the end. Bounds are clamped to file size.                 |
| `pipeFile(path, data)`         | `Promise<void>`. Replaces file contents and commits before returning. Accepts `Uint8Array`.                                    |
| `open(path)`                   | `Promise<Reader>`. Opens an existing file for reading.                                                                         |
| `writer(path)`                 | `Promise<Writer>`. Opens a replacement write; parent directories must exist.                                                   |
| `info(path)`                   | `Promise<FileInfo>`. Returns file or directory metadata.                                                                       |
| `ls(path = "")`                | `Promise<FileInfo[]>`. Lists directory children, sorted by name. A file path returns that file's metadata as a one-item array. |
| `mkdir(path, parents = false)` | `Promise<void>`. Creates a directory. With `parents`, creates missing parents.                                                 |
| `rmdir(path)`                  | `Promise<void>`. Removes an empty directory.                                                                                   |
| `rmFile(path)`                 | `Promise<void>`. Removes a file.                                                                                               |
| `cpFile(src, dst)`             | `Promise<void>`. Reads the source into memory and replaces the destination.                                                    |
| `free()`                       | Releases the WASM filesystem object. Stored files remain.                                                                      |

### Reader

| Method           | Result and behavior                                                                                     |
| ---------------- | ------------------------------------------------------------------------------------------------------- |
| `read(size)`     | `Promise<Uint8Array>`. Reads up to `size` bytes and advances the cursor. Returns an empty array at EOF. |
| `seek(position)` | `Promise<number>`. Sets an absolute nonnegative byte position and returns it.                           |
| `close()`        | `Promise<void>`. Releases the browser file reference. Repeated close is allowed.                        |
| `free()`         | Releases the WASM reader object.                                                                        |

Offsets and read sizes must be finite safe integers; read sizes must be nonnegative. Reads and seeks after close fail.

### Writer

| Method        | Result and behavior                                                                                        |
| ------------- | ---------------------------------------------------------------------------------------------------------- |
| `write(data)` | `Promise<void>`. Appends a `Uint8Array` to the pending replacement.                                        |
| `commit()`    | `Promise<void>`. Publishes the replacement and closes the writer. An empty replacement truncates the file. |
| `abort()`     | `Promise<void>`. Discards uncommitted bytes. Safe after abort or commit.                                   |
| `free()`      | Releases the WASM writer object and schedules abort if unfinished.                                         |

Writes and commits after commit or abort fail. Dropping a JavaScript reference does not guarantee prompt cleanup; explicit abort is available for application cleanup.

### FileInfo and errors

`FileInfo` has `name: string`, `size: number`, and `type: "file" | "directory"`. File metadata from `info` and `ls` also includes `lastModified`, expressed as milliseconds since the Unix epoch. Directories have size zero and no modification timestamp.

Rejected I/O promises contain an `Error` with a string `code`. Mapped codes include `not_found`, `permission_denied`, `not_supported`, `not_a_directory`, `is_a_directory`, and `invalid`; other browser failures use the backend's fallback error classification. A missing WASM asset or module initialization failure may instead produce a loader error without a backend code.

## Rust

Browser exports are available for `wasm32-unknown-unknown`:

| API                                         | Contract                                                |
| ------------------------------------------- | ------------------------------------------------------- |
| `Opfs::new().await`                         | Returns `FsResult<Opfs>` for the current origin.        |
| `Opfs::writer(path).await`                  | Returns `FsResult<OpfsWriter>` for a replacement write. |
| `OpfsWriter::write(&mut self, bytes).await` | Writes a pending chunk.                                 |
| `OpfsWriter::commit(&mut self).await`       | Publishes and closes.                                   |
| `OpfsWriter::abort(&mut self).await`        | Discards pending bytes. `Drop` schedules this cleanup.  |

`Opfs` implements `fsspec_rs::AsyncFileSystem`, including `ls`, `info`, `mkdir`, `rmdir`, `rm_file`, `cp_file`, `cat_file`, and `pipe_file`. `open_async` returns `Box<dyn AsyncFsFile>` with `read`, `seek`, `info`, and `close`. The legacy `open` method returns `NotSupported` because it requires a blocking file handle.

Browser futures run on the owning JavaScript event loop and may be non-`Send`. Configure `fsspec_rs` with `default-features = false`; native Tokio runtime adapters are not used.

## Python

`OPFSFileSystem` implements `fsspec.AbstractFileSystem` and is registered under `opfs`. It provides `open`, `info`, `ls`, `mkdir`, `makedirs`, `rmdir`, `rm_file`, `rm`, `cp_file`, and `cat_file`, plus fsspec's derived operations. `local_root` is the native fallback directory or `None` in the browser. `close()` frees browser state or removes the native directory; the filesystem also supports a context manager.

Browser `open` supports read and replacement write modes, including text wrappers. Native fallback uses local file modes. Browser operations require Pyodide JSPI and a registered `fsspec_opfs_js` module; [Python setup](python.md) describes the entry point and module registration. Browser setup errors never trigger native fallback.
