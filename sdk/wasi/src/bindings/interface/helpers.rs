// Helper to generate all the back and forth From<Into> conversions
// thanks ChatGPT!

#[macro_export]
macro_rules! generate_struct_impls {
    // -------------------------------------------------------------------------
    // Entry point: we match a bracketed list of pairs `[ (T1, U1), (T2, U2), ... ]`
    // plus one or more field idents `field1, field2, ...`.
    // We forward that to @munch_pairs, passing the entire set of pairs
    // plus a separate set of fields.
    // -------------------------------------------------------------------------
    (
        [ $( ($TypeA:ty, $TypeB:ty) ),+ $(,)? ],
        $( $field:ident ),+ $(,)?
    ) => {
        $crate::generate_struct_impls!(
            @munch_pairs
            ( $( ($TypeA, $TypeB), )+ )  // The list of pairs
            => ( $( $field ),+ )        // The list of fields
        );
    };

    // -------------------------------------------------------------------------
    // @munch_pairs: we either have:
    //   1) No more pairs ( ) => (fields...) : we do nothing
    //   2) A head pair `(TypeA, TypeB),` and possibly more pairs in `$( $tail )*`
    //      => (fields...) : we generate the 2 impls, then recurse.
    // -------------------------------------------------------------------------

    // 1) No more pairs? Then do nothing.
    (@munch_pairs () => ( $( $field:ident ),+ )) => {
        // No pairs left, we're done!
    };

    // 2) We have at least one pair (TypeA, TypeB).
    //    Generate 2 impl blocks for that pair, copying the named fields.
    //    Then recurse to handle the tail.
    (@munch_pairs ( ($TypeA:ty, $TypeB:ty), $( $tail:tt )* )
        => ( $( $field:ident ),+ )
    ) => {
        impl From<$TypeA> for $TypeB {
            fn from(src: $TypeA) -> Self {
                Self {
                    $( $field: src.$field ),+
                }
            }
        }

        impl From<$TypeB> for $TypeA {
            fn from(src: $TypeB) -> Self {
                Self {
                    $( $field: src.$field ),+
                }
            }
        }

        // Now recurse on the rest of the pairs
        $crate::generate_struct_impls!(@munch_pairs ( $( $tail )* ) => ( $( $field ),+ ));
    };
}

#[macro_export]
macro_rules! generate_contract_struct_impls {
    ($Type:ident, [ $( $field:ident ),+ $(,)? ]) => {
        $crate::generate_struct_impls!(
            [
                ($Type, super::worlds::cosmos_contract_event::$Type),
                ($Type, super::worlds::eth_contract_event::$Type),
                ($Type, super::worlds::any_contract_event::$Type),
            ],
            $( $field ),+
        );
    };
}

#[macro_export]
macro_rules! generate_any_enum_impls {
    ($FromType:ty, $ToType:ty) => {
        impl From<$FromType> for $ToType {
            fn from(src: $FromType) -> Self {
                type Alias = $FromType;
                match src {
                    Alias::Cosmos(inner) => Self::Cosmos(inner.into()),
                    Alias::Eth(inner) => Self::Eth(inner.into()),
                }
            }
        }
        impl From<$ToType> for $FromType {
            fn from(src: $ToType) -> Self {
                type Alias = $ToType;
                match src {
                    Alias::Cosmos(inner) => Self::Cosmos(inner.into()),
                    Alias::Eth(inner) => Self::Eth(inner.into()),
                }
            }
        }
    };
}

#[macro_export]
macro_rules! generate_contract_enum_impls {
    ($Ident:ident) => {
        $crate::generate_any_enum_impls!($Ident, super::worlds::cosmos_contract_event::$Ident);
        $crate::generate_any_enum_impls!($Ident, super::worlds::eth_contract_event::$Ident);
        $crate::generate_any_enum_impls!($Ident, super::worlds::any_contract_event::$Ident);
    };
}

#[macro_export]
macro_rules! generate_chain_configs_impls {
    ($FromType:ident, $ToType:ty) => {
        impl From<$FromType> for $ToType {
            fn from(src: $FromType) -> Self {
                Self {
                    eth: src
                        .eth
                        .into_iter()
                        .map(|(key, config)| (key.clone(), config.into()))
                        .collect(),
                    cosmos: src
                        .cosmos
                        .into_iter()
                        .map(|(key, config)| (key.clone(), config.into()))
                        .collect(),
                }
            }
        }
        impl From<$ToType> for $FromType {
            fn from(src: $ToType) -> Self {
                Self {
                    eth: src
                        .eth
                        .into_iter()
                        .map(|(key, config)| (key.clone(), config.into()))
                        .collect(),
                    cosmos: src
                        .cosmos
                        .into_iter()
                        .map(|(key, config)| (key.clone(), config.into()))
                        .collect(),
                }
            }
        }
    };
}

#[macro_export]
macro_rules! generate_contract_chain_configs_impls {
    ($Type:ident) => {
        $crate::generate_chain_configs_impls!($Type, super::worlds::cosmos_contract_event::$Type);
        $crate::generate_chain_configs_impls!($Type, super::worlds::eth_contract_event::$Type);
        $crate::generate_chain_configs_impls!($Type, super::worlds::any_contract_event::$Type);
    };
}
