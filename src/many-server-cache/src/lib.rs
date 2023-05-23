use coset::CoseSign1;
use many_error::ManyError;
use many_protocol::ResponseMessage;
use many_server::RequestValidator;
use sha2::Digest;
use std::collections::HashSet;
use std::path::Path;
use std::sync::{Arc, RwLock};

/// Implement this trait to provide a cache backend for the cache validator.
pub trait RequestCacheBackend: Send + Sync {
    /// Returns true if the request was cached.
    fn has(&self, request: &[u8]) -> bool;

    /// Add the request to the cache. This cannot fail.
    fn put(&mut self, request: &[u8]);
}

impl RequestCacheBackend for () {
    fn has(&self, _request: &[u8]) -> bool {
        false
    }
    fn put(&mut self, _request: &[u8]) {}
}

impl<T: RequestCacheBackend + ?Sized> RequestCacheBackend for Arc<RwLock<T>> {
    fn has(&self, request: &[u8]) -> bool {
        self.read().unwrap().has(request)
    }

    fn put(&mut self, request: &[u8]) {
        self.write().unwrap().put(request)
    }
}

pub struct RequestCacheValidator<T: RequestCacheBackend> {
    backend: T,
}

unsafe impl<T: RequestCacheBackend + Send> Send for RequestCacheValidator<T> {}
unsafe impl<T: RequestCacheBackend + Sync> Sync for RequestCacheValidator<T> {}

impl<T: RequestCacheBackend> RequestCacheValidator<T> {
    pub fn new(backend: T) -> Self {
        Self { backend }
    }
}

impl<T: RequestCacheBackend> RequestValidator for RequestCacheValidator<T> {
    fn validate_envelope(&self, envelope: &CoseSign1) -> Result<(), ManyError> {
        let payload = envelope
            .payload
            .as_ref()
            .ok_or_else(ManyError::empty_envelope)?;
        let mut hasher = sha2::Sha512::default();
        hasher.update(payload);
        let hash = hasher.finalize();

        if self.backend.has(hash.as_ref()) {
            Err(ManyError::duplicated_message())
        } else {
            Ok(())
        }
    }

    fn message_executed(
        &mut self,
        envelope: &CoseSign1,
        _response: &ResponseMessage,
    ) -> Result<(), ManyError> {
        let payload = envelope
            .payload
            .as_ref()
            .ok_or_else(ManyError::empty_envelope)?;
        let mut hasher = sha2::Sha512::default();
        hasher.update(payload);
        let hash = hasher.finalize();
        self.backend.put(hash.as_ref());
        Ok(())
    }
}

pub struct RocksDbCacheBackend {
    db: rocksdb::DB,
}

impl RocksDbCacheBackend {
    pub fn new(path: impl AsRef<Path>) -> Self {
        let db = rocksdb::DB::open_default(path).unwrap();
        Self { db }
    }
}

impl RequestCacheBackend for RocksDbCacheBackend {
    fn has(&self, key: &[u8]) -> bool {
        self.db.get(key).unwrap().is_some()
    }
    fn put(&mut self, key: &[u8]) {
        let mut batch = rocksdb::WriteBatch::default();
        batch.put(key, b"");
        self.db.write(batch).unwrap();
    }
}

#[derive(Clone)]
pub struct SharedRocksDbCacheBackend {
    inner: Arc<RwLock<RocksDbCacheBackend>>,
}

impl SharedRocksDbCacheBackend {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            inner: Arc::new(RwLock::new(RocksDbCacheBackend::new(path))),
        }
    }
}

impl RequestCacheBackend for SharedRocksDbCacheBackend {
    fn has(&self, key: &[u8]) -> bool {
        self.inner.read().unwrap().has(key)
    }
    fn put(&mut self, key: &[u8]) {
        self.inner.write().unwrap().put(key)
    }
}

#[derive(Clone)]
pub struct InMemoryCacheBackend(HashSet<Vec<u8>>);

impl RequestCacheBackend for InMemoryCacheBackend {
    fn has(&self, request: &[u8]) -> bool {
        self.0.contains(request)
    }

    fn put(&mut self, request: &[u8]) {
        self.0.insert(request.to_vec());
    }
}
