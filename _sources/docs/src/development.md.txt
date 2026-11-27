# How to build and test changes

Install Rust, Node.js, pnpm, and Python 3.11 or newer before running these commands from the repository root.

## Build browser assets and run browser tests

```bash
rustup target add wasm32-unknown-unknown
pnpm --dir js install --frozen-lockfile
pnpm --dir js setup
pnpm --dir js build
pnpm --dir js exec playwright install chromium
python -m pip install build hatchling hatch-js hatch-rs fsspec
pnpm --dir js test
```

The browser suite serves the built assets locally and exercises real OPFS in Chromium, including Python fsspec calls in Pyodide. Test preparation builds the pure Python wheel and copies the installed fsspec sources so the browser tests run without downloading Python packages. Rebuild after changing Rust or TypeScript source. `js/dist/pkg` contains the WASM binary and browser support code; `js/dist/esm` and `js/dist/cdn` contain JavaScript entry points.

## Run Rust checks

Install the template's native test and coverage tools, then run its test targets:

```bash
make -C rust requirements
make -C rust lint
make -C rust test
make -C rust coverage
```

Native tests cover path normalization and rejection of invalid components. `cargo llvm-cov nextest` generates JUnit and coverage reports using `.config/nextest.toml`. Browser filesystem behavior is covered by Playwright. The lint target checks both native and WASM Rust builds.

## Build documentation

```bash
python -m pip install "yardang[themes]" mdformat mdformat-myst mdformat-tables codespell
yardang build
make lint-docs
python -m http.server 8001 --directory docs/html
```

Open `http://localhost:8001` to inspect the generated site. Documentation sources are listed in `[tool.yardang]` in `pyproject.toml`. The API pages are maintained manually, so Python auto-API generation is disabled and a native extension build is unnecessary for a local docs build.

After a successful main-branch build, the template's Publish Docs workflow publishes `docs/html` to `gh-pages`. GitHub Pages must use the `gh-pages` branch root as its publishing source. A manual workflow dispatch builds the Markdown documentation directly.
