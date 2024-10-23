/// common tests for any castorage implementation
pub mod castorage {
    use crate::storage::{CAStorage, CAStorageError};

    pub fn test_set_and_get<S: CAStorage>(mut store: S) {
        let data = b"hello world";
        let digest = store.set_data(data).unwrap();
        let loaded = store.get_data(&digest).unwrap();
        assert_eq!(data, loaded.as_slice());
    } 

    pub fn test_reset<S: CAStorage>(mut store: S) {
        let data = b"hello world";
        let digest = store.set_data(data).unwrap();
        store.reset().unwrap();
        let err = store.get_data(&digest).unwrap_err();
        assert!(matches!(err, CAStorageError::NotFound(_)));
    } 
}