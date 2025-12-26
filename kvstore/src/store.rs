
use std::collections::HashMap;
use std::sync::{Arc,Mutex};

#[derive(Clone)]
pub struct KvStore{
      pub inner : Arc<Mutex<HashMap<Vec<u8>,Vec<u8>>>>
}

impl KvStore{
      pub fn new()->Self{
            Self { inner: Arc::new(Mutex::new(HashMap::new())) }
      }

      pub fn put(&self,key : Vec<u8>, value : Vec<u8>){
            self.inner.lock().unwrap().insert(key, value);
      }

       pub fn get(&self, key: &Vec<u8>) -> Option<Vec<u8>> {
            self.inner.lock().unwrap().get(key).cloned()
      }   

      pub fn delete(&self,key : &Vec<u8>){
            self.inner.lock().unwrap().remove(key);
      }
}