function error(name, message) {
  return new DOMException(message, name);
}

async function directory(root, path, create = false) {
  let handle = root;
  for (const name of path.split("/").filter(Boolean)) {
    handle = await handle.getDirectoryHandle(name, { create });
  }
  return handle;
}

async function parent(root, path) {
  const names = path.split("/");
  const name = names.pop();
  if (!name) throw error("InvalidStateError", "A file or directory name is required");
  return [await directory(root, names.join("/")), name];
}

async function entry(root, path) {
  if (!path) return root;
  const [dir, name] = await parent(root, path);
  try {
    return await dir.getFileHandle(name);
  } catch (e) {
    if (e.name !== "TypeMismatchError") throw e;
    return dir.getDirectoryHandle(name);
  }
}

async function metadata(handle, path) {
  if (handle.kind === "directory") return { name: path, size: 0, directory: true };
  const file = await handle.getFile();
  return { name: path, size: file.size, directory: false, modified: file.lastModified };
}

export async function root() {
  if (!globalThis.navigator?.storage?.getDirectory) {
    throw error("NotSupportedError", "OPFS requires a supported browser and secure context");
  }
  return navigator.storage.getDirectory();
}

export async function info(root, path) {
  return metadata(await entry(root, path), path);
}

export async function list(root, path) {
  const handle = await entry(root, path);
  if (handle.kind !== "directory") return [await metadata(handle, path)];
  const entries = [];
  for await (const [name, child] of handle.entries()) {
    entries.push(await metadata(child, path ? `${path}/${name}` : name));
  }
  return entries.sort((a, b) => a.name.localeCompare(b.name));
}

export async function mkdir(root, path, parents) {
  if (!path) return;
  if (parents) {
    await directory(root, path, true);
    return;
  }
  const [dir, name] = await parent(root, path);
  await dir.getDirectoryHandle(name, { create: true });
}

export async function remove(root, path, isDirectory) {
  const handle = await entry(root, path);
  if ((handle.kind === "directory") !== isDirectory) {
    throw error(isDirectory ? "NotADirectoryError" : "IsADirectoryError", path);
  }
  const [dir, name] = await parent(root, path);
  await dir.removeEntry(name);
}

export async function file(root, path) {
  const handle = await entry(root, path);
  if (handle.kind === "directory") throw error("IsADirectoryError", path);
  return handle.getFile();
}

export async function read(file, start, end) {
  return new Uint8Array(await file.slice(start, end).arrayBuffer());
}

export function writerState() {
  return { stream: null, cancelled: false };
}

export async function begin(root, path, state) {
  const [dir, name] = await parent(root, path);
  let handle;
  try {
    handle = await dir.getFileHandle(name, { create: true });
  } catch (e) {
    if (e.name === "TypeMismatchError") throw error("IsADirectoryError", path);
    throw e;
  }
  state.stream = await handle.createWritable();
  if (state.cancelled) {
    await state.stream.abort();
    throw error("AbortError", "Writer was discarded");
  }
}

export async function write(state, data) {
  if (state.cancelled || !state.stream) throw error("InvalidStateError", "Writer is closed");
  // The borrowed WASM view can be detached by another allocation while I/O waits.
  await state.stream.write(data.slice());
}

export async function commit(state) {
  if (state.cancelled || !state.stream) throw error("InvalidStateError", "Writer is closed");
  await state.stream.close();
  state.stream = null;
  state.cancelled = true;
}

export async function abort(state) {
  state.cancelled = true;
  if (state.stream) {
    await state.stream.abort();
    state.stream = null;
  }
}

export function discard(state) {
  // Drop cannot await, but cleanup continues on the owning browser event loop.
  void abort(state).catch(() => {});
}
