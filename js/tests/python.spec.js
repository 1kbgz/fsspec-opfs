import { test, expect } from "@playwright/test";
import { readdirSync } from "node:fs";

const wheel = readdirSync(new URL("../dist/wheels/", import.meta.url)).find(
  (name) => name.endsWith(".whl"),
);

async function loadPython(page, register = true) {
  await page.goto("/");
  await page.evaluate(
    async ({ wheel, register }) => {
      const { loadPyodide } = await import("/node_modules/pyodide/pyodide.mjs");
      const pyodide = await loadPyodide({
        indexURL: "/node_modules/pyodide/",
      });
      const site = pyodide.runPython("import site; site.getsitepackages()[0]");
      for (const name of ["fsspec.zip", wheel]) {
        const response = await fetch(`/dist/wheels/${name}`);
        if (!response.ok) throw new Error(`Cannot load ${name}`);
        pyodide.unpackArchive(await response.arrayBuffer(), "zip", {
          extractDir: site,
        });
      }
      if (register) {
        pyodide.registerJsModule(
          "fsspec_opfs_js",
          await import("/dist/cdn/index.js"),
        );
      }
      globalThis.pyodide = pyodide;
    },
    { wheel, register },
  );
}

test("Python fsspec reads, streams writes, and persists alongside JavaScript", async ({
  page,
}) => {
  await loadPython(page);
  await page.evaluate(async () => {
    await pyodide.runPythonAsync(`
import fsspec
fs = fsspec.filesystem("opfs")
assert fs.local_root is None
fs.makedirs("python", exist_ok=True)
with fs.open("opfs://python/data", "wb", block_size=2) as file:
    file.write(b"abc")
    file.write(b"def")
assert fs.cat_file("python/data", start=1, end=4) == b"bcd"
with fs.open("python/data", "rb", block_size=2) as file:
    assert file.read(2) == b"ab"
    file.seek(-2, 2)
    assert file.read() == b"ef"
with fs.open("python/text", "w") as file:
    file.write("hello")
assert fs.cat_file("python/text") == b"hello"
assert fs.ls("python", detail=False) == ["python/data", "python/text"]
fs.cp_file("python/data", "python/copy")
fs.rm_file("python/copy")
try:
    fs.cat_file("missing")
except FileNotFoundError:
    pass
else:
    raise AssertionError("missing file did not fail")
fs.close()
`);
    const { FileSystem } = await import("/dist/cdn/index.js");
    const fs = await FileSystem.connect();
    if (
      new TextDecoder().decode(await fs.catFile("python/data")) !== "abcdef"
    ) {
      throw new Error("Python and JavaScript did not share OPFS");
    }
    fs.free();
  });
  await page.reload();
  await loadPython(page);
  expect(
    await page.evaluate(() =>
      pyodide.runPythonAsync(
        'import fsspec; fs = fsspec.filesystem("opfs"); fs.cat_file("python/data").decode()',
      ),
    ),
  ).toBe("abcdef");
});

test("Python transactions discard changes and invalid paths and modes fail", async ({
  page,
}) => {
  await loadPython(page);
  await page.evaluate(async () => {
    await pyodide.runPythonAsync(`
import fsspec
fs = fsspec.filesystem("opfs")
fs.pipe_file("transaction", b"old")
try:
    with fs.transaction:
        fs.pipe_file("transaction", b"new")
        raise RuntimeError("abort")
except RuntimeError:
    pass
assert fs.cat_file("transaction") == b"old"
with fs.transaction:
    fs.pipe_file("transaction", b"new")
assert fs.cat_file("transaction") == b"new"
try:
    with fs.open("transaction", "wb", block_size=1) as file:
        file.write(b"discard")
        raise RuntimeError("abort")
except RuntimeError:
    pass
assert fs.cat_file("transaction") == b"new"
for path in ["../bad", "opfs://dir/../bad", "dir/./bad"]:
    try:
        fs.pipe_file(path, b"bad")
    except ValueError:
        pass
    else:
        raise AssertionError(path)
for mode in ["ab", "xb", "r+b"]:
    try:
        fs.open("transaction", mode)
    except NotImplementedError:
        pass
    else:
        raise AssertionError(mode)
fs.makedirs("tree/child")
fs.pipe_file("tree/child/file", b"x")
fs.rm("tree", recursive=True)
assert not fs.exists("tree")
fs.close()
`);
  });
});

test("Python gives setup errors without native temporary-storage fallback", async ({
  page,
}) => {
  await loadPython(page, false);
  expect(
    await page.evaluate(async () => {
      try {
        await pyodide.runPythonAsync(
          'import fsspec; fsspec.filesystem("opfs")',
        );
      } catch (error) {
        return error.message;
      }
    }),
  ).toContain("registerJsModule");
  expect(
    await page.evaluate(() => {
      try {
        pyodide.runPython('import fsspec; fsspec.filesystem("opfs")');
      } catch (error) {
        return error.message;
      }
    }),
  ).toContain("JSPI");
});
