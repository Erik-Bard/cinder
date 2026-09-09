use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub type Store = Arc<Mutex<HashMap<Vec<u8>, Vec<u8>>>>;

pub fn new_store() -> Store {
    Arc::new(Mutex::new(HashMap::new()))
}