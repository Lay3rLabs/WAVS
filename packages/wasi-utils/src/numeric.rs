/// Macro to convert between Rust's native u128 and WIT tuple<u64, u64> representation.
///
/// Usage:
///   impl_u128_conversions!(my_bindings::exports::my_interface::U128);
///
/// This will implement From traits for bidirectional conversion between
/// the WIT-generated tuple type and Rust's native u128.
#[macro_export]
macro_rules! impl_u128_conversions {
    ($wit_type:ty) => {
        impl From<u128> for $wit_type {
            fn from(value: u128) -> Self {
                let low = value as u64; // Rust guarantees this is truncating to the least significant/lower 64 bits
                let high = (value >> 64) as u64;
                Self { value: (low, high) }
            }
        }

        impl From<$wit_type> for u128 {
            fn from(wrapper: $wit_type) -> Self {
                let (low, high) = wrapper.value;
                ((high as u128) << 64) | (low as u128)
            }
        }
    };
}

#[cfg(test)]
mod tests {
    // In theory, something like this would be better, but wit-bindgen only sees the remote type, not the local one.
    // which defeats the purpose of a local test
    //
    // and trying to generate bindings from the types-only crate:
    // 1. doesn't work because it has no world definition
    // 2. just punts the problem since we'd want to write unit tests for any of our wit types
    //
    // wit_bindgen::generate!({
    //     world: "wavs-world",
    //     path: "../../wit-definitions/operator/wit",
    //     //async: true,
    // });
    //
    // so, instead, for this case it's easy to see what the generated type would look like and just define it here

    #[derive(Debug, PartialEq, Copy, Clone)]
    struct FakeU128 {
        value: (u64, u64),
    }

    impl_u128_conversions!(FakeU128);

    #[test]
    fn test_u128_conversion_max() {
        let original: u128 = u128::MAX;
        let wit_repr: FakeU128 = original.into();
        assert_eq!(
            wit_repr,
            FakeU128 {
                value: (u64::MAX, u64::MAX)
            }
        );
        let converted_back: u128 = wit_repr.into();
        assert_eq!(original, converted_back);
    }

    #[test]
    fn test_u128_conversion_endianness() {
        // Test various bit patterns to ensure correct endianness handling
        let test_cases = [
            // Simple cases
            0u128,
            1u128,
            u64::MAX as u128,         // Only lower 64 bits set
            (u64::MAX as u128) << 64, // Only upper 64 bits set
            u128::MAX,
            // Specific bit patterns to test endianness
            0x0123456789ABCDEF_FEDCBA9876543210u128,
            0xFFFFFFFF00000000_00000000FFFFFFFFu128,
            0x0000000000000001_0000000000000000u128, // Bit 64 set
            0x8000000000000000_0000000000000000u128, // MSB set
            0x0000000000000000_8000000000000000u128, // Bit 63 set
        ];

        for &test_value in &test_cases {
            // Manual bit extraction (what the macro should do)
            let manual_low = test_value as u64; // Truncate to get lower 64 bits
            let manual_high = (test_value >> 64) as u64; // Shift right to get upper 64 bits

            // Use the macro to convert u128 -> WIT type
            let wit_value: FakeU128 = test_value.into();

            // Verify the macro extracted bits correctly
            assert_eq!(
                wit_value.value.0, manual_low,
                "Lower bits mismatch for {:#034x}. Expected: {:#018x}, Got: {:#018x}",
                test_value, manual_low, wit_value.value.0
            );
            assert_eq!(
                wit_value.value.1, manual_high,
                "Upper bits mismatch for {:#034x}. Expected: {:#018x}, Got: {:#018x}",
                test_value, manual_high, wit_value.value.1
            );

            // Manual bit reconstruction (what the reverse macro should do)
            let manual_reconstructed = ((manual_high as u128) << 64) | (manual_low as u128);

            // Use the macro to convert WIT type -> u128
            let macro_reconstructed: u128 = wit_value.into();

            // Verify both manual and macro reconstruction match original
            assert_eq!(
                manual_reconstructed, test_value,
                "Manual reconstruction failed for {:#034x}",
                test_value
            );
            assert_eq!(
                macro_reconstructed, test_value,
                "Macro reconstruction failed for {:#034x}",
                test_value
            );
            assert_eq!(
                manual_reconstructed, macro_reconstructed,
                "Manual and macro reconstruction disagree for {:#034x}",
                test_value
            );
        }
    }
}
