__version__ = "0.1.0"

from fsspec import register_implementation

from .fs import OPFSFileSystem

register_implementation("opfs", OPFSFileSystem)
