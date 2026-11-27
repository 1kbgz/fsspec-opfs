import { test, expect } from "@playwright/test";

async function connect(page) {
  await page.goto("/dist/index.html");
  await page.evaluate(async () => {
    const { FileSystem } = await import("/dist/esm/index.js");
    window.fs = await FileSystem.connect();
  });
}

test("writes persist across reconnect and page reload", async ({ page }) => {
  await connect(page);
  await page.evaluate(async () => {
    await fs.mkdir("tables/day", true);
    await fs.pipeFile("opfs://tables/day/orders", new Uint8Array([1, 2, 3, 4]));
    fs.free();
  });
  await connect(page);
  expect(
    await page.evaluate(async () =>
      Array.from(await fs.catFile("tables/day/orders")),
    ),
  ).toEqual([1, 2, 3, 4]);
  expect(await page.evaluate(() => fs.info("tables/day/orders"))).toMatchObject(
    { name: "tables/day/orders", size: 4, type: "file" },
  );
  expect(await page.evaluate(() => fs.ls("tables/day"))).toHaveLength(1);
  expect(
    await page.evaluate(async () =>
      Array.from(await fs.catFile("tables/day/orders", -2)),
    ),
  ).toEqual([3, 4]);
  expect(
    await page.evaluate(async () =>
      Array.from(await fs.catFile("tables/day/orders", 3, 1)),
    ),
  ).toEqual([]);
});

test("async handles seek, stop at EOF, and close", async ({ page }) => {
  await connect(page);
  expect(
    await page.evaluate(async () => {
      await fs.pipeFile("read", new Uint8Array([10, 20, 30, 40]));
      const reader = await fs.open("read");
      const first = Array.from(await reader.read(2));
      await reader.seek(1);
      const rest = Array.from(await reader.read(100));
      await reader.seek(100);
      const eof = Array.from(await reader.read(1));
      await reader.close();
      let code;
      try {
        await reader.read(1);
      } catch (error) {
        code = error.code;
      }
      reader.free();
      return { first, rest, eof, code };
    }),
  ).toEqual({ first: [10, 20], rest: [20, 30, 40], eof: [], code: "invalid" });
});

test("abort and free discard replacement bytes; commit publishes chunks", async ({
  page,
}) => {
  await connect(page);
  expect(
    await page.evaluate(async () => {
      await fs.pipeFile("write", new Uint8Array([1, 2]));
      const writer = await fs.writer("write");
      await writer.write(new Uint8Array([9]));
      const before = Array.from(await fs.catFile("write"));
      await writer.abort();
      writer.free();
      const aborted = Array.from(await fs.catFile("write"));
      const dropped = await fs.writer("write");
      await dropped.write(new Uint8Array([8]));
      dropped.free();
      const afterFree = Array.from(await fs.catFile("write"));
      const committed = await fs.writer("write");
      await committed.write(new Uint8Array([3, 4]));
      await committed.write(new Uint8Array([5]));
      await committed.commit();
      committed.free();
      return {
        before,
        aborted,
        afterFree,
        committed: Array.from(await fs.catFile("write")),
      };
    }),
  ).toEqual({
    before: [1, 2],
    aborted: [1, 2],
    afterFree: [1, 2],
    committed: [3, 4, 5],
  });
});

test("copy, list and deletion enforce file and directory types", async ({
  page,
}) => {
  await connect(page);
  expect(
    await page.evaluate(async () => {
      await fs.mkdir("directory");
      await fs.pipeFile("directory/a", new Uint8Array([42]));
      await fs.cpFile("directory/a", "directory/b");
      const entries = await fs.ls("directory");
      const failures = [];
      for (const operation of [
        () => fs.catFile("missing"),
        () => fs.catFile("directory"),
        () => fs.writer("directory"),
        () => fs.rmFile("directory"),
        () => fs.rmdir("directory/a"),
        () => fs.rmdir("directory"),
        () => fs.mkdir("missing/child"),
        () => fs.catFile("../outside"),
        () => fs.catFile("directory/a", Number.NaN),
      ]) {
        try {
          await operation();
          failures.push("success");
        } catch (error) {
          failures.push(error.code);
        }
      }
      await fs.rmFile("directory/a");
      await fs.rmFile("directory/b");
      await fs.rmdir("directory");
      return {
        entries: entries.map((e) => e.name),
        failures,
        remaining: await fs.ls(),
      };
    }),
  ).toEqual({
    entries: ["directory/a", "directory/b"],
    failures: [
      "not_found",
      "is_a_directory",
      "is_a_directory",
      "is_a_directory",
      "not_a_directory",
      "invalid",
      "not_found",
      "invalid",
      "invalid",
    ],
    remaining: [],
  });
});

test("empty writes truncate; discarded new writes publish no bytes", async ({
  page,
}) => {
  await connect(page);
  expect(
    await page.evaluate(async () => {
      const writer = await fs.writer("new");
      await writer.write(new Uint8Array([1, 2, 3]));
      await writer.abort();
      writer.free();
      const aborted = Array.from(await fs.catFile("new"));
      await fs.pipeFile("new", new Uint8Array([9]));
      await fs.pipeFile("new", new Uint8Array());
      return { aborted, empty: Array.from(await fs.catFile("new")) };
    }),
  ).toEqual({ aborted: [], empty: [] });
});

test("read sizes reject negative and fractional values", async ({ page }) => {
  await connect(page);
  expect(
    await page.evaluate(async () => {
      await fs.pipeFile("read", new Uint8Array([1]));
      const reader = await fs.open("read");
      const errors = [];
      for (const size of [-1, 0.5, Number.NaN, 2 ** 32]) {
        try {
          await reader.read(size);
          errors.push("success");
        } catch (error) {
          errors.push(error.code);
        }
      }
      reader.free();
      return errors;
    }),
  ).toEqual(["invalid", "invalid", "invalid", "invalid"]);
});

test("concurrent connections share initialization and storage", async ({
  page,
}) => {
  await page.goto("/dist/index.html");
  expect(
    await page.evaluate(async () => {
      const { FileSystem } = await import("/dist/esm/index.js");
      const [first, second] = await Promise.all([
        FileSystem.connect(),
        FileSystem.connect(),
      ]);
      await first.pipeFile("shared", new Uint8Array([7, 8]));
      const data = Array.from(await second.catFile("shared"));
      first.free();
      second.free();
      return data;
    }),
  ).toEqual([7, 8]);
});
