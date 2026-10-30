use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

pub type Store = Arc<Mutex<HashMap<Vec<u8>, Entry>>>;
pub struct Entry {
    pub value: Vec<u8>,
    pub expires_at: Option<SystemTime>
}

pub fn new_store() -> Store {
    Arc::new(Mutex::new(HashMap::new()))
}