import sys
from pathlib import Path

import fsspec
import pytest

from fsspec_opfs import OPFSFileSystem

pytestmark = pytest.mark.skipif(sys.platform == "emscripten", reason="Native fallback tests; browser behavior is covered by Playwright/Pyodide")


@pytest.fixture
def fs():
    with pytest.warns(RuntimeWarning, match="temporary local directory"):
        instance = fsspec.filesystem("opfs")
    with instance:
        yield instance


def test_registration_and_local_filesystem_operations(fs):
    assert isinstance(fs, OPFSFileSystem)
    fs.makedirs("opfs://data/nested")
    with fs.open("opfs://data/nested/message.txt", "w") as file:
        file.write("hello")
    assert fs.cat_file("data/nested/message.txt", start=1, end=4) == b"ell"
    with fs.open("data/nested/message.txt", "rb") as file:
        file.seek(-2, 2)
        assert file.read() == b"lo"
    assert fs.info("/data/nested/message.txt")["name"] == "data/nested/message.txt"
    assert fs.ls("data/nested", detail=False) == ["data/nested/message.txt"]
    fs.cp_file("data/nested/message.txt", "data/copy.txt")
    assert fs.find("data") == ["data/copy.txt", "data/nested/message.txt"]
    fs.rm("data", recursive=True)
    assert fs.ls("") == []


def test_instances_are_isolated_and_close_removes_only_owned_directory(fs):
    fs.pipe_file("secret", b"first")
    with pytest.warns(RuntimeWarning):
        other = fsspec.filesystem("opfs")
    with other:
        assert fs is not other
        assert fs.local_root != other.local_root
        assert not other.exists("secret")
        other.pipe_file("secret", b"second")
        root = Path(other.local_root)
    assert not root.exists()
    assert fs.cat_file("secret") == b"first"
    other.close()
    with pytest.raises(ValueError, match="closed"):
        other.ls("")


@pytest.mark.parametrize("path", ["..", "data/../outside", "opfs://../outside", "./file", "data\\file", "data\0file"])
def test_paths_cannot_escape_temporary_root(fs, path):
    with pytest.raises(ValueError):
        fs.open(path, "wb")
    with pytest.raises(ValueError):
        fs.info(path)
    with pytest.raises(ValueError):
        fs.mkdir(path)


def test_absolute_looking_paths_stay_under_root(fs):
    fs.makedirs("/tmp/example", exist_ok=True)
    fs.pipe_file("opfs:///tmp/example/value", b"inside")
    assert Path(fs.local_root, "tmp/example/value").read_bytes() == b"inside"
    assert fs.cat_file("tmp//example/value") == b"inside"
    with pytest.raises(ValueError, match="root"):
        fs.rmdir("")


def test_symlinks_cannot_escape_temporary_root(fs, tmp_path):
    (tmp_path / "secret").write_bytes(b"outside")
    try:
        Path(fs.local_root, "escape").symlink_to(tmp_path, target_is_directory=True)
    except OSError as error:
        pytest.skip(f"symlinks unavailable: {error}")
    for method, args in [
        (fs.cat_file, ("escape/secret",)),
        (fs.pipe_file, ("escape/new", b"bad")),
        (fs.rm_file, ("escape/secret",)),
        (fs.cp_file, ("escape/secret", "copy")),
        (fs.ls, ("",)),
    ]:
        with pytest.raises(ValueError, match="escapes"):
            method(*args)
    assert (tmp_path / "secret").read_bytes() == b"outside"
    assert not (tmp_path / "new").exists()


def test_native_transaction_can_commit_or_discard(fs):
    with fs.transaction:
        fs.pipe_file("committed", b"yes")
    assert fs.cat_file("committed") == b"yes"
    with pytest.raises(RuntimeError), fs.transaction:
        fs.pipe_file("discarded", b"no")
        raise RuntimeError("abort")
    assert not fs.exists("discarded")
