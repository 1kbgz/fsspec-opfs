"""Build a pure Python wheel with browser assets and no native extension."""

import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path


def build(output: Path) -> None:
    root = Path(__file__).resolve().parent.parent
    if not (root / "fsspec_opfs/extension/pkg/fsspec_opfs_bg.wasm").is_file():
        raise RuntimeError("Build browser assets with pnpm --dir js build before building the browser wheel")
    with tempfile.TemporaryDirectory(prefix="fsspec-opfs-wheel-") as directory:
        stage = Path(directory)
        for name in ("pyproject.toml", "README.md", "LICENSE"):
            shutil.copy(root / name, stage / name)
        shutil.copytree(
            root / "fsspec_opfs",
            stage / "fsspec_opfs",
            ignore=shutil.ignore_patterns("*.so", "*.dll", "*.dylib", "*.pyd", "__pycache__"),
        )
        env = {**os.environ, "SKIP_HATCH_RUST": "1", "SKIP_HATCH_JS": "1"}
        subprocess.run(
            [sys.executable, "-m", "build", "--wheel", "--no-isolation", "--outdir", str(output.resolve()), str(stage)], env=env, check=True
        )


if __name__ == "__main__":
    build(Path(sys.argv[1]) if len(sys.argv) > 1 else Path("dist/browser"))
