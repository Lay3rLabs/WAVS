pub trait HashMapLike<K, KRef, V> {
    fn get_key(&self, key: KRef) -> Option<&V>;
}

// probably a better way to define this... but whatever, works for our "hashmap" wit types
impl<V> HashMapLike<String, &str, V> for Vec<(String, V)> {
    fn get_key(&self, key: &str) -> Option<&V> {
        self.iter()
            .find_map(|(k, v)| if k == key { Some(v) } else { None })
    }
}
