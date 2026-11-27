import init, {
  FileSystem as NativeFileSystem,
} from "../../dist/pkg/fsspec_opfs.js";
export { Reader, Writer } from "../../dist/pkg/fsspec_opfs.js";

export interface FileInfo {
  name: string;
  size: number;
  type: "file" | "directory";
  lastModified?: number;
}

let initialization: ReturnType<typeof init> | undefined;

/** OPFS belongs to the current origin and browser profile. */
export class FileSystem {
  private constructor(private readonly inner: NativeFileSystem) {}

  static async connect(): Promise<FileSystem> {
    initialization ??= init({
      module_or_path: new URL("../pkg/fsspec_opfs_bg.wasm", import.meta.url),
    }).catch((error) => {
      initialization = undefined;
      throw error;
    });
    await initialization;
    return new FileSystem(await NativeFileSystem.connect());
  }

  catFile(path: string, start?: number, end?: number): Promise<Uint8Array> {
    return this.inner.catFile(path, start, end);
  }
  pipeFile(path: string, data: Uint8Array): Promise<void> {
    return this.inner.pipeFile(path, data);
  }
  info(path: string): Promise<FileInfo> {
    return this.inner.info(path);
  }
  ls(path = ""): Promise<FileInfo[]> {
    return this.inner.ls(path);
  }
  mkdir(path: string, parents = false): Promise<void> {
    return this.inner.mkdir(path, parents);
  }
  rmdir(path: string): Promise<void> {
    return this.inner.rmdir(path);
  }
  rmFile(path: string): Promise<void> {
    return this.inner.rmFile(path);
  }
  cpFile(src: string, dst: string): Promise<void> {
    return this.inner.cpFile(src, dst);
  }
  writer(path: string) {
    return this.inner.writer(path);
  }
  open(path: string) {
    return this.inner.open(path);
  }
  free(): void {
    this.inner.free();
  }
}
