/// common tests for any castorage implementation
pub mod castorage {
    use crate::storage::{CAStorage, CAStorageError};
    use wavs_types::Digest;

    pub fn test_set_and_get<S: CAStorage>(store: S) {
        let data = b"hello world";
        let digest = store.set_data(data).unwrap();
        let loaded = store.get_data(&digest).unwrap();
        assert_eq!(data, loaded.as_slice());

        let missing = Digest::new(b"missing");
        let err = store.get_data(&missing).unwrap_err();
        assert!(matches!(err, CAStorageError::NotFound(_)));
    }

    pub fn test_reset<S: CAStorage>(store: S) {
        let data = b"hello world";
        let digest = store.set_data(data).unwrap();
        store.reset().unwrap();
        let err = store.get_data(&digest).unwrap_err();
        assert!(matches!(err, CAStorageError::NotFound(_)));
    }

    pub fn test_multiple_keys<S: CAStorage>(store: S) {
        let data1 = b"hello world";
        let data2 = b"hello mom";

        // store two different data blobs
        let digest1 = store.set_data(data1).unwrap();
        let loaded1 = store.get_data(&digest1).unwrap();
        assert_eq!(data1, loaded1.as_slice());

        let digest2 = store.set_data(data2).unwrap();
        let loaded2 = store.get_data(&digest2).unwrap();
        assert_eq!(data2, loaded2.as_slice());

        // they have different keys
        assert_ne!(digest1, digest2);
        // we can still load the first one
        let loaded1 = store.get_data(&digest1).unwrap();
        assert_eq!(data1, loaded1.as_slice());
    }

    pub fn test_list_digests<S: CAStorage>(store: S) {
        let data1 = b"hello world";
        let data2 = b"hello mom";

        // store two different data blobs
        let digest1 = store.set_data(data1).unwrap();
        let digest2 = store.set_data(data2).unwrap();

        // they have different keys
        assert_ne!(digest1, digest2);

        // we can list the digests (sort both as order is not defined)
        let mut expected = vec![digest1, digest2];
        expected.sort();
        let mut digests = store
            .digests()
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        digests.sort();
        assert_eq!(expected, digests);
    }
}
