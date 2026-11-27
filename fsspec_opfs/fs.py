"""OPFS through Pyodide, with isolated temporary storage on native Python."""

from __future__ import annotations

import os
import sys
import tempfile
import warnings
from importlib import import_module
from pathlib import Path
from typing import Any, Self

from fsspec.implementations.local import LocalFileSystem
from fsspec.spec import AbstractBufferedFile, AbstractFileSystem


class OPFSFileSystem(AbstractFileSystem):
    """Browser OPFS or a warned, per-instance temporary local filesystem.

    Browser calls require Pyodide JSPI and a promising Python entry point, such
    as ``runPythonAsync``. Register the JavaScript package as ``fsspec_opfs_js``
    with ``pyodide.registerJsModule`` before constructing this filesystem.
    """

    protocol = "opfs"
    root_marker = ""
    cachable = False

    def __init__(self, **kwargs: Any) -> None:
        super().__init__(**kwargs)
        self._closed = False
        self._temporary: tempfile.TemporaryDirectory[str] | None = None
        self._browser: Any = None
        self._ffi: Any = None
        self._local: LocalFileSystem | None = None
        if sys.platform == "emscripten":
            self._ffi = import_module("pyodide.ffi")
            self._check_sync()
            try:
                module = import_module("fsspec_opfs_js")
            except ImportError as error:
                raise RuntimeError("Register the fsspec-opfs JavaScript module with pyodide.registerJsModule('fsspec_opfs_js', module)") from error
            self._browser = self._ffi.run_sync(module.FileSystem.connect())
        else:
            warnings.warn(
                "OPFS requires browser Pyodide; using an isolated temporary local directory. Data is removed when this filesystem closes.",
                RuntimeWarning,
                stacklevel=2,
            )
            self._temporary = tempfile.TemporaryDirectory(prefix="fsspec-opfs-")
            self._local = LocalFileSystem(auto_mkdir=False)

    @classmethod
    def _strip_protocol(cls, path: Any) -> str:
        path = os.fspath(path)
        path = path.removeprefix("opfs://").removeprefix("opfs::").strip("/")
        if "\\" in path or "\0" in path or any(part in {".", ".."} for part in path.split("/")):
            raise ValueError("OPFS paths cannot contain dot components, backslashes, or NUL")
        return "/".join(part for part in path.split("/") if part)

    @property
    def local_root(self) -> str | None:
        """Temporary directory in native Python; None for browser OPFS."""
        return self._temporary.name if self._temporary is not None else None

    def _check_open(self) -> None:
        if self._closed:
            raise ValueError("OPFS filesystem is closed")

    def _check_sync(self) -> None:
        if not self._ffi.can_run_sync():
            raise RuntimeError("OPFS requires Pyodide JSPI; call Python through runPythonAsync or callPromising in a supporting browser")

    def _path(self, path: Any) -> str:
        self._check_open()
        path = self._strip_protocol(path)
        if self._temporary is None:
            return path
        root = Path(self._temporary.name).resolve()
        result = root.joinpath(path).resolve()
        if not result.is_relative_to(root):
            raise ValueError("OPFS path escapes its temporary directory")
        return str(result)

    def _call(self, target: Any, method: str, *args: Any) -> Any:
        self._check_open()
        self._check_sync()
        try:
            return self._ffi.run_sync(getattr(target, method)(*args))
        except self._ffi.JsException as error:
            code = getattr(error.js_error, "code", None)
            kind = {
                "not_found": FileNotFoundError,
                "already_exists": FileExistsError,
                "permission_denied": PermissionError,
                "is_a_directory": IsADirectoryError,
                "not_a_directory": NotADirectoryError,
                "not_supported": NotImplementedError,
                "invalid": ValueError,
            }.get(code, OSError)
            raise kind(str(error)) from error

    def info(self, path: Any, **kwargs: Any) -> dict[str, Any]:
        name = self._strip_protocol(path)
        path = self._path(name)
        if self._local is not None:
            info = self._local.info(path, **kwargs)
        else:
            info = self._call(self._browser, "info", path).to_py()
        return {**info, "name": name}

    def ls(self, path: Any, detail: bool = True, **kwargs: Any) -> list[Any]:
        path = self._path(path)
        if self._local is not None:
            assert self.local_root is not None
            root = Path(self.local_root).resolve()
            entries = []
            for info in self._local.ls(path, detail=True, **kwargs):
                name = Path(info["name"]).relative_to(root).as_posix()
                self._path(name)
                entries.append({**info, "name": name})
        else:
            entries = self._call(self._browser, "ls", path).to_py()
        entries.sort(key=lambda info: info["name"])
        return entries if detail else [info["name"] for info in entries]

    def mkdir(self, path: Any, create_parents: bool = True, **kwargs: Any) -> None:
        path = self._path(path)
        if self._local is not None:
            self._local.mkdir(path, create_parents=create_parents, **kwargs)
        else:
            self._call(self._browser, "mkdir", path, create_parents)

    def makedirs(self, path: Any, exist_ok: bool = False) -> None:
        self._path(path)
        if self.exists(path):
            if not exist_ok or not self.isdir(path):
                raise FileExistsError(path)
            return
        self.mkdir(path, create_parents=True)

    def rmdir(self, path: Any) -> None:
        if not self._strip_protocol(path):
            raise ValueError("Cannot remove the OPFS root")
        path = self._path(path)
        if self._local is not None:
            self._local.rmdir(path)
        else:
            self._call(self._browser, "rmdir", path)

    def rm_file(self, path: Any) -> None:
        path = self._path(path)
        if self._local is not None:
            self._local.rm_file(path)
        else:
            self._call(self._browser, "rmFile", path)

    def rm(self, path: Any, recursive: bool = False, maxdepth: int | None = None) -> None:
        for name in reversed(self.expand_path(path, recursive=recursive, maxdepth=maxdepth)):
            if self.isdir(name):
                self.rmdir(name)
            else:
                self.rm_file(name)

    def cp_file(self, path1: Any, path2: Any, **kwargs: Any) -> None:
        source, target = self._path(path1), self._path(path2)
        if self._local is not None:
            self._local.cp_file(source, target, **kwargs)
        else:
            self._call(self._browser, "cpFile", source, target)

    def cat_file(self, path: Any, start: int | None = None, end: int | None = None, **kwargs: Any) -> bytes:
        path = self._path(path)
        if self._local is not None:
            return self._local.cat_file(path, start=start, end=end, **kwargs)
        return bytes(self._call(self._browser, "catFile", path, start, end).to_py())

    def _open(
        self,
        path: Any,
        mode: str = "rb",
        block_size: int | None = None,
        autocommit: bool = True,
        cache_options: dict[str, Any] | None = None,
        **kwargs: Any,
    ) -> Any:
        path = self._path(path)
        if self._local is not None:
            file = self._local._open(path, mode=mode, block_size=block_size, autocommit=autocommit, cache_options=cache_options, **kwargs)
            file._opfs_owner = self
            return file
        if mode not in {"rb", "wb"}:
            raise NotImplementedError("Browser OPFS supports read and replacement write modes (rb/wb), not append or exclusive creation")
        return OPFSFile(self, path, mode=mode, block_size=block_size, autocommit=autocommit, cache_options=cache_options, **kwargs)

    def close(self) -> None:
        """Release browser state or delete the native temporary directory."""
        if self._closed:
            return
        if self._browser is not None:
            self._browser.free()
        if self._temporary is not None:
            self._temporary.cleanup()
        self._closed = True

    def __enter__(self) -> Self:
        self._check_open()
        return self

    def __exit__(self, *args: object) -> None:
        self.close()


class OPFSFile(AbstractBufferedFile):
    """Buffered fsspec file over the browser's reader or replacement writer."""

    def __init__(self, fs: OPFSFileSystem, path: str, **kwargs: Any) -> None:
        self._handle: Any = None
        super().__init__(fs, path, **kwargs)
        self._handle = fs._call(fs._browser, "open" if self.mode == "rb" else "writer", path)

    def _fetch_range(self, start: int, end: int) -> bytes:
        self.fs._call(self._handle, "seek", start)
        return bytes(self.fs._call(self._handle, "read", end - start).to_py())

    def _upload_chunk(self, final: bool = False) -> None:
        data = self.buffer.getvalue()
        if data:
            self.fs._call(self._handle, "write", self.fs._ffi.to_js(memoryview(data)))
        if final and self.autocommit:
            self.commit()

    def commit(self) -> None:
        if self.mode == "wb" and not self.closed and not self.forced:
            self.flush(force=True)
        if self._handle is not None and self.mode == "wb":
            self.fs._call(self._handle, "commit")
            self._handle.free()
            self._handle = None

    def __exit__(self, *args: object) -> None:
        if args and args[0] is not None:
            self.discard()
        else:
            self.close()

    def discard(self) -> None:
        self.closed = True
        if self._handle is not None:
            try:
                self.fs._call(self._handle, "close" if self.mode == "rb" else "abort")
            finally:
                self._handle.free()
                self._handle = None

    def close(self) -> None:
        if self.closed:
            return
        try:
            super().close()
        except BaseException:
            self.discard()
            raise
        if self.mode == "rb":
            self.discard()

    def __del__(self) -> None:
        # Finalizers cannot require a JSPI entry point. Rust Drop schedules abort.
        if getattr(self, "_handle", None) is not None:
            self._handle.free()
            self._handle = None
