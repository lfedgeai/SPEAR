use crate::spearlet::execution::ExecutionError;
use std::collections::HashMap;

pub type HttpCallResult = (i32, Vec<u8>, HashMap<String, String>);

pub trait SpearHostApi: Send + Sync {
    fn log(&self, level: &str, message: &str);
    fn time_now_ms(&self) -> u64;
    fn random_bytes(&self, len: usize) -> Vec<u8>;
    fn get_env(&self, key: &str) -> Option<String>;
    fn http_call(
        &self,
        method: &str,
        url: &str,
        headers: HashMap<String, String>,
        body: Vec<u8>,
    ) -> Result<HttpCallResult, ExecutionError>;
    fn put_result(
        &self,
        task_id: &str,
        data: Vec<u8>,
        metadata: HashMap<String, String>,
    ) -> Result<String, ExecutionError>;
    fn get_object(&self, id: &str) -> Result<Vec<u8>, ExecutionError>;
    fn put_object(&self, name: &str, bytes: Vec<u8>) -> Result<String, ExecutionError>;
}
