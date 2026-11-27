#![cfg(all(target_arch = "wasm32", target_os = "unknown"))]

use fsspec_opfs::{Opfs, OpfsWriter};
use fsspec_rs::{AsyncFileSystem, AsyncFsFile, FileInfo, FsError, OperationError};
use js_sys::{Array, Object, Reflect};
use wasm_bindgen::prelude::*;

fn error(e: FsError) -> JsValue {
    let value = js_sys::Error::new(&e.to_string());
    Reflect::set(&value, &"code".into(), &e.kind().as_str().into()).unwrap();
    value.into()
}
fn operation_error(e: OperationError) -> JsValue {
    let value = js_sys::Error::new(&e.to_string());
    Reflect::set(&value, &"code".into(), &e.kind().as_str().into()).unwrap();
    value.into()
}
fn metadata(info: FileInfo) -> JsValue {
    let value = Object::new();
    Reflect::set(&value, &"name".into(), &info.name.into()).unwrap();
    Reflect::set(&value, &"size".into(), &JsValue::from_f64(info.size as f64)).unwrap();
    Reflect::set(&value, &"type".into(), &info.file_type.to_string().into()).unwrap();
    if let Some(time) = info.extra.get("last_modified_ms") {
        Reflect::set(
            &value,
            &"lastModified".into(),
            &JsValue::from_f64(time.parse().unwrap()),
        )
        .unwrap();
    }
    value.into()
}
fn offset(value: f64) -> Result<i64, JsValue> {
    if !value.is_finite() || value.fract() != 0.0 || value.abs() > 9_007_199_254_740_991.0 {
        return Err(error(FsError::InvalidArgument(
            "Offset must be a safe integer".into(),
        )));
    }
    Ok(value as i64)
}

#[wasm_bindgen]
pub struct FileSystem {
    inner: Opfs,
}

#[wasm_bindgen]
impl FileSystem {
    pub async fn connect() -> Result<FileSystem, JsValue> {
        Ok(Self {
            inner: Opfs::new().await.map_err(error)?,
        })
    }
    #[wasm_bindgen(js_name = catFile)]
    pub async fn cat_file(
        &self,
        path: &str,
        start: Option<f64>,
        end: Option<f64>,
    ) -> Result<Vec<u8>, JsValue> {
        self.inner
            .cat_file(
                path,
                start.map(offset).transpose()?,
                end.map(offset).transpose()?,
            )
            .await
            .map_err(error)
    }
    #[wasm_bindgen(js_name = pipeFile)]
    pub async fn pipe_file(&self, path: &str, data: &[u8]) -> Result<(), JsValue> {
        self.inner.pipe_file(path, data).await.map_err(error)
    }
    pub async fn info(&self, path: &str) -> Result<JsValue, JsValue> {
        Ok(metadata(self.inner.info(path).await.map_err(error)?))
    }
    pub async fn ls(&self, path: &str) -> Result<Array, JsValue> {
        Ok(self
            .inner
            .ls(path, true)
            .await
            .map_err(error)?
            .into_iter()
            .map(metadata)
            .collect())
    }
    pub async fn mkdir(&self, path: &str, parents: bool) -> Result<(), JsValue> {
        self.inner.mkdir(path, parents).await.map_err(error)
    }
    pub async fn rmdir(&self, path: &str) -> Result<(), JsValue> {
        self.inner.rmdir(path).await.map_err(error)
    }
    #[wasm_bindgen(js_name = rmFile)]
    pub async fn rm_file(&self, path: &str) -> Result<(), JsValue> {
        self.inner.rm_file(path).await.map_err(error)
    }
    #[wasm_bindgen(js_name = cpFile)]
    pub async fn cp_file(&self, src: &str, dst: &str) -> Result<(), JsValue> {
        self.inner.cp_file(src, dst).await.map_err(error)
    }
    pub async fn writer(&self, path: &str) -> Result<Writer, JsValue> {
        Ok(Writer {
            inner: self.inner.writer(path).await.map_err(error)?,
        })
    }
    pub async fn open(&self, path: &str) -> Result<Reader, JsValue> {
        Ok(Reader {
            inner: self.inner.open_async(path).await.map_err(operation_error)?,
        })
    }
}

#[wasm_bindgen]
pub struct Writer {
    inner: OpfsWriter,
}

#[wasm_bindgen]
impl Writer {
    pub async fn write(&mut self, data: &[u8]) -> Result<(), JsValue> {
        self.inner.write(data).await.map_err(error)
    }
    pub async fn commit(&mut self) -> Result<(), JsValue> {
        self.inner.commit().await.map_err(error)
    }
    pub async fn abort(&mut self) -> Result<(), JsValue> {
        self.inner.abort().await.map_err(error)
    }
}

#[wasm_bindgen]
pub struct Reader {
    inner: Box<dyn AsyncFsFile>,
}

#[wasm_bindgen]
impl Reader {
    pub async fn read(&mut self, size: f64) -> Result<Vec<u8>, JsValue> {
        let size = offset(size)?;
        let size = usize::try_from(size)
            .map_err(|_| error(FsError::InvalidArgument("Read size is out of range".into())))?;
        self.inner.read(size).await.map_err(operation_error)
    }
    pub async fn seek(&mut self, position: f64) -> Result<f64, JsValue> {
        let position = offset(position)?;
        if position < 0 {
            return Err(error(FsError::InvalidArgument(
                "Seek position must be nonnegative".into(),
            )));
        }
        self.inner
            .seek(std::io::SeekFrom::Start(position as u64))
            .await
            .map(|n| n as f64)
            .map_err(operation_error)
    }
    pub async fn close(&mut self) -> Result<(), JsValue> {
        self.inner.close().await.map_err(operation_error)
    }
}
