use std::io::SeekFrom;

use fsspec_rs::{
    AsyncFileSystem, AsyncFsFile, FileInfo, FsError, FsFile, FsResult, IoFuture, OpenMode,
    OpenOptions,
};
use js_sys::{Array, Reflect, Uint8Array};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(module = "/src/opfs.js")]
extern "C" {
    #[wasm_bindgen(catch)]
    async fn root() -> Result<JsValue, JsValue>;
    #[wasm_bindgen(catch)]
    async fn info(root: &JsValue, path: &str) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(catch)]
    async fn list(root: &JsValue, path: &str) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(catch)]
    async fn mkdir(root: &JsValue, path: &str, parents: bool) -> Result<(), JsValue>;
    #[wasm_bindgen(catch)]
    async fn remove(root: &JsValue, path: &str, directory: bool) -> Result<(), JsValue>;
    #[wasm_bindgen(catch)]
    async fn file(root: &JsValue, path: &str) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(catch)]
    async fn read(file: &JsValue, start: f64, end: f64) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(js_name = writerState)]
    fn writer_state() -> JsValue;
    #[wasm_bindgen(catch)]
    async fn begin(root: &JsValue, path: &str, state: &JsValue) -> Result<(), JsValue>;
    #[wasm_bindgen(catch)]
    async fn write(state: &JsValue, data: &[u8]) -> Result<(), JsValue>;
    #[wasm_bindgen(catch)]
    async fn commit(state: &JsValue) -> Result<(), JsValue>;
    #[wasm_bindgen(catch)]
    async fn abort(state: &JsValue) -> Result<(), JsValue>;
    fn discard(state: &JsValue);
}

fn property(value: &JsValue, key: &str) -> JsValue {
    Reflect::get(value, &key.into()).unwrap_or(JsValue::UNDEFINED)
}

fn browser_error(error: JsValue) -> FsError {
    let name = property(&error, "name").as_string().unwrap_or_default();
    let message = property(&error, "message")
        .as_string()
        .unwrap_or_else(|| format!("{error:?}"));
    match name.as_str() {
        "NotFoundError" => FsError::NotFound(message),
        "NotAllowedError" | "SecurityError" => FsError::PermissionDenied(message),
        "NotSupportedError" => FsError::NotSupported(message),
        "TypeMismatchError" | "NotADirectoryError" => FsError::NotADirectory(message),
        "IsADirectoryError" => FsError::IsADirectory(message),
        "TypeError" | "InvalidStateError" | "InvalidModificationError" => {
            FsError::InvalidArgument(message)
        }
        _ => FsError::Other(format!("{name}: {message}")),
    }
}

fn path(path: &str) -> FsResult<String> {
    crate::path::normalize(path).map_err(|message| FsError::InvalidArgument(message.into()))
}

fn metadata(value: JsValue) -> FileInfo {
    let name = property(&value, "name").as_string().unwrap_or_default();
    if property(&value, "directory").as_bool() == Some(true) {
        FileInfo::directory(name)
    } else {
        let mut info = FileInfo::file(
            name,
            property(&value, "size").as_f64().unwrap_or_default() as u64,
        );
        if let Some(modified) = property(&value, "modified").as_f64() {
            info.extra
                .insert("last_modified_ms".into(), modified.to_string());
        }
        info
    }
}

/// Browser-owned filesystem rooted at the current origin's private directory.
#[derive(Clone)]
pub struct Opfs {
    root: JsValue,
}

impl Opfs {
    pub async fn new() -> FsResult<Self> {
        Ok(Self {
            root: root().await.map_err(browser_error)?,
        })
    }

    /// Open a replacement writer. Only commit publishes buffered bytes.
    /// Creating a new path may make an empty file visible before commit.
    pub async fn writer(&self, name: &str) -> FsResult<OpfsWriter> {
        let name = path(name)?;
        let writer = OpfsWriter {
            state: writer_state(),
        };
        begin(&self.root, &name, &writer.state)
            .await
            .map_err(browser_error)?;
        Ok(writer)
    }
}

/// A streaming replacement write. Dropping it schedules abort, never commit.
pub struct OpfsWriter {
    state: JsValue,
}

impl OpfsWriter {
    pub async fn write(&mut self, data: &[u8]) -> FsResult<()> {
        write(&self.state, data).await.map_err(browser_error)
    }
    /// Once close starts, dropping this future cannot guarantee cancellation.
    pub async fn commit(&mut self) -> FsResult<()> {
        commit(&self.state).await.map_err(browser_error)
    }
    pub async fn abort(&mut self) -> FsResult<()> {
        abort(&self.state).await.map_err(browser_error)
    }
}

impl Drop for OpfsWriter {
    fn drop(&mut self) {
        discard(&self.state);
    }
}

impl AsyncFileSystem for Opfs {
    fn protocol(&self) -> &[&str] {
        &["opfs"]
    }
    async fn ls(&self, name: &str, _detail: bool) -> FsResult<Vec<FileInfo>> {
        let result = list(&self.root, &path(name)?)
            .await
            .map_err(browser_error)?;
        Ok(Array::from(&result).iter().map(metadata).collect())
    }
    async fn info(&self, name: &str) -> FsResult<FileInfo> {
        Ok(metadata(
            info(&self.root, &path(name)?)
                .await
                .map_err(browser_error)?,
        ))
    }
    async fn mkdir(&self, name: &str, parents: bool) -> FsResult<()> {
        mkdir(&self.root, &path(name)?, parents)
            .await
            .map_err(browser_error)
    }
    async fn rm_file(&self, name: &str) -> FsResult<()> {
        remove(&self.root, &path(name)?, false)
            .await
            .map_err(browser_error)
    }
    async fn rmdir(&self, name: &str) -> FsResult<()> {
        remove(&self.root, &path(name)?, true)
            .await
            .map_err(browser_error)
    }
    async fn open(
        &self,
        _: &str,
        _: OpenMode,
        _: Option<OpenOptions>,
    ) -> FsResult<Box<dyn FsFile>> {
        Err(FsError::NotSupported(
            "OPFS requires asynchronous file handles".into(),
        ))
    }
    async fn open_async(
        &self,
        name: &str,
    ) -> Result<Box<dyn AsyncFsFile>, fsspec_rs::OperationError> {
        let name = path(name)?;
        let value = file(&self.root, &name).await.map_err(browser_error)?;
        let size = property(&value, "size").as_f64().unwrap_or_default() as u64;
        Ok(Box::new(OpfsFile {
            file: Some(value),
            info: FileInfo::file(name, size),
            position: 0,
        }))
    }
    async fn cat_file(
        &self,
        name: &str,
        start: Option<i64>,
        end: Option<i64>,
    ) -> FsResult<Vec<u8>> {
        let value = file(&self.root, &path(name)?)
            .await
            .map_err(browser_error)?;
        let size = property(&value, "size").as_f64().unwrap_or_default();
        // Blob.slice implements the same clamped, negative-from-end range semantics.
        let result = read(
            &value,
            start.unwrap_or(0) as f64,
            end.map_or(size, |v| v as f64),
        )
        .await
        .map_err(browser_error)?;
        Ok(Uint8Array::new(&result).to_vec())
    }
    async fn pipe_file(&self, name: &str, data: &[u8]) -> FsResult<()> {
        let mut writer = self.writer(name).await?;
        writer.write(data).await?;
        writer.commit().await
    }
    async fn cp_file(&self, src: &str, dst: &str) -> FsResult<()> {
        let bytes = self.cat_file(src, None, None).await?;
        self.pipe_file(dst, &bytes).await
    }
}

struct OpfsFile {
    file: Option<JsValue>,
    info: FileInfo,
    position: u64,
}

impl AsyncFsFile for OpfsFile {
    fn read(&mut self, count: usize) -> IoFuture<'_, Vec<u8>> {
        Box::pin(async move {
            let value = self
                .file
                .as_ref()
                .ok_or_else(|| FsError::InvalidArgument("File is closed".into()))?;
            let end = self
                .position
                .saturating_add(count as u64)
                .min(self.info.size);
            let bytes = read(value, self.position as f64, end as f64)
                .await
                .map_err(browser_error)?;
            let bytes = Uint8Array::new(&bytes).to_vec();
            self.position += bytes.len() as u64;
            Ok(bytes)
        })
    }
    fn seek(&mut self, position: SeekFrom) -> IoFuture<'_, u64> {
        Box::pin(async move {
            if self.file.is_none() {
                return Err(FsError::InvalidArgument("File is closed".into()).into());
            }
            let position = match position {
                SeekFrom::Start(n) => i128::from(n),
                SeekFrom::Current(n) => i128::from(self.position) + i128::from(n),
                SeekFrom::End(n) => i128::from(self.info.size) + i128::from(n),
            };
            if !(0..=9_007_199_254_740_991).contains(&position) {
                return Err(
                    FsError::InvalidArgument("Seek position is out of range".into()).into(),
                );
            }
            self.position = position as u64;
            Ok(self.position)
        })
    }
    fn info(&self) -> IoFuture<'_, FileInfo> {
        Box::pin(async { Ok(self.info.clone()) })
    }
    fn close(&mut self) -> IoFuture<'_, ()> {
        self.file = None;
        Box::pin(async { Ok(()) })
    }
}
