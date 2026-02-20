pub mod option_const_hex {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(value: &Option<Vec<u8>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match value {
            Some(bytes) => const_hex::serialize(bytes, serializer),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Vec<u8>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<String>::deserialize(deserializer)?
            .map(|s| const_hex::decode(&s).map_err(serde::de::Error::custom))
            .transpose()
    }
}

#[cfg(test)]
mod test {
    #[test]
    fn test_option_const_hex() {
        use super::option_const_hex;
        use serde::{Deserialize, Serialize};

        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct TestStruct {
            #[serde(with = "option_const_hex")]
            data: Option<Vec<u8>>,
        }

        let original = TestStruct {
            data: Some(vec![1, 2, 3, 4, 5]),
        };

        let expected_str = const_hex::encode_prefixed(original.data.as_ref().unwrap());

        let serialized = serde_json::to_string(&original).unwrap();
        assert_eq!(serialized, format!("{{\"data\":\"{expected_str}\"}}"));

        let deserialized: TestStruct = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, original);

        let original_none = TestStruct { data: None };
        let serialized_none = serde_json::to_string(&original_none).unwrap();
        assert_eq!(serialized_none, r#"{"data":null}"#);

        let deserialized_none: TestStruct = serde_json::from_str(&serialized_none).unwrap();
        assert_eq!(deserialized_none, original_none);
    }
}
