use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::{self, Debug, Display, Formatter};
use utoipa::ToSchema;

/// A newtype that wraps a `[u8; N]` using const generics.
/// and is serialized as a `0x` prefixed hex string.
#[derive(Clone, PartialEq, Eq, Hash, Copy, ToSchema)]
pub struct ByteArray<const N: usize>([u8; N]);

impl<const N: usize> ByteArray<N> {
    pub fn new(data: [u8; N]) -> Self {
        ByteArray(data)
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.0
    }

    pub fn into_inner(self) -> [u8; N] {
        self.0
    }

    pub fn len(&self) -> usize {
        N
    }

    pub fn is_empty(&self) -> bool {
        N == 0
    }
}

impl<const N: usize> Display for ByteArray<N> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        // Encode the byte array as hex using `const_hex::encode`.
        let hex_string = const_hex::encode(self.0);
        write!(f, "0x{}", hex_string)
    }
}

impl<const N: usize> Debug for ByteArray<N> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(self, f)
    }
}

impl<const N: usize> Serialize for ByteArray<N> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Serialize the hex string
        let hex_string = self.to_string();
        serializer.serialize_str(&hex_string)
    }
}

impl<'de, const N: usize> Deserialize<'de> for ByteArray<N> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Deserialize as a string, then decode that string from hex.
        let s = String::deserialize(deserializer)?;
        let bytes = const_hex::decode(&s).map_err(|e| serde::de::Error::custom(e.to_string()))?;

        // Ensure the decoded bytes have the correct length.
        let array: [u8; N] = bytes
            .try_into()
            .map_err(|_| serde::de::Error::custom("invalid hex length"))?;

        Ok(ByteArray(array))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display() {
        let data = ByteArray::<4>([0xDE, 0xAD, 0xBE, 0xEF]);
        assert_eq!(format!("{}", data), "0xdeadbeef");
    }

    #[test]
    fn test_serde() {
        let data = ByteArray::<4>([0xDE, 0xAD, 0xBE, 0xEF]);

        // Test serialization
        let serialized = serde_json::to_string(&data).unwrap();
        // Expect a JSON string: "deadbeef"
        assert_eq!(serialized, "\"0xdeadbeef\"");

        // Test deserialization
        let deserialized: ByteArray<4> = serde_json::from_str("\"0xdeadbeef\"").unwrap();
        assert_eq!(deserialized.0, [0xDE, 0xAD, 0xBE, 0xEF]);
    }

    #[test]
    fn test_debug() {
        let data = ByteArray::<4>([0xDE, 0xAD, 0xBE, 0xEF]);
        assert_eq!(format!("{:?}", data), "0xdeadbeef");
        assert_eq!(format!("{:#?}", data), "0xdeadbeef");
    }
}
