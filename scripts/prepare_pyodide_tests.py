"""Serve the built Python wheel and installed fsspec to browser tests offline."""

import zipfile
from pathlib import Path

import fsspec
from build_browser_wheel import build

root = Path(__file__).resolve().parent.parent
output = root / "js/dist/wheels"
build(output)
package = Path(fsspec.__file__).parent
with zipfile.ZipFile(output / "fsspec.zip", "w", zipfile.ZIP_DEFLATED) as archive:
    for source in package.rglob("*.py"):
        archive.write(source, str(source.relative_to(package.parent)))
