use cosmwasm_std::{StdError, Uint256};

use crate::contracts::cosmwasm::service_manager::error::WavsEventError;

/// Emit this event when the service URI is updated
pub struct WavsServiceUriUpdatedEvent {
    pub service_uri: String,
}
impl WavsServiceUriUpdatedEvent {
    pub const EVENT_TYPE: &'static str = "wavs-service-uri-updated";
    pub const EVENT_ATTR_KEY_SERVICE_URI: &'static str = "service-uri";
}

/// Emit this event when the quorum threshold is updated
pub struct WavsQuorumThresholdUpdatedEvent {
    pub numerator: Uint256,
    pub denominator: Uint256,
}

impl WavsQuorumThresholdUpdatedEvent {
    pub const EVENT_TYPE: &'static str = "wavs-quorum-threshold-updated";
    pub const EVENT_ATTR_KEY_NUMERATOR: &'static str = "numerator";
    pub const EVENT_ATTR_KEY_DENOMINATOR: &'static str = "denominator";
}

impl From<WavsServiceUriUpdatedEvent> for cosmwasm_std::Event {
    fn from(src: WavsServiceUriUpdatedEvent) -> Self {
        cosmwasm_std::Event::new(WavsServiceUriUpdatedEvent::EVENT_TYPE).add_attribute(
            WavsServiceUriUpdatedEvent::EVENT_ATTR_KEY_SERVICE_URI,
            src.service_uri,
        )
    }
}

impl TryFrom<&cosmwasm_std::Event> for WavsServiceUriUpdatedEvent {
    type Error = WavsEventError;

    fn try_from(event: &cosmwasm_std::Event) -> Result<Self, Self::Error> {
        if event.ty != Self::EVENT_TYPE {
            return Err(WavsEventError::EventType {
                expected: Self::EVENT_TYPE.to_string(),
                found: event.ty.to_string(),
            });
        }
        let service_uri = event
            .attributes
            .iter()
            .find(|attr| attr.key == Self::EVENT_ATTR_KEY_SERVICE_URI)
            .map(|attr| attr.value.to_string())
            .ok_or_else(|| WavsEventError::MissingAttribute {
                event_type: Self::EVENT_TYPE.to_string(),
                attr_key: Self::EVENT_ATTR_KEY_SERVICE_URI.to_string(),
            })?;

        Ok(Self { service_uri })
    }
}

impl From<WavsQuorumThresholdUpdatedEvent> for cosmwasm_std::Event {
    fn from(src: WavsQuorumThresholdUpdatedEvent) -> Self {
        cosmwasm_std::Event::new(WavsQuorumThresholdUpdatedEvent::EVENT_TYPE)
            .add_attribute(
                WavsQuorumThresholdUpdatedEvent::EVENT_ATTR_KEY_NUMERATOR,
                src.numerator.to_string(),
            )
            .add_attribute(
                WavsQuorumThresholdUpdatedEvent::EVENT_ATTR_KEY_DENOMINATOR,
                src.denominator.to_string(),
            )
    }
}

impl TryFrom<&cosmwasm_std::Event> for WavsQuorumThresholdUpdatedEvent {
    type Error = WavsEventError;

    fn try_from(event: &cosmwasm_std::Event) -> Result<Self, Self::Error> {
        if event.ty != Self::EVENT_TYPE {
            return Err(WavsEventError::EventType {
                expected: Self::EVENT_TYPE.to_string(),
                found: event.ty.to_string(),
            });
        }

        let mut numerator: Option<Uint256> = None;
        let mut denominator: Option<Uint256> = None;

        for attr in &event.attributes {
            if attr.key == Self::EVENT_ATTR_KEY_NUMERATOR {
                numerator = Some(attr.value.parse().map_err(|err: StdError| {
                    WavsEventError::ParseAttribute {
                        event_type: Self::EVENT_TYPE.to_string(),
                        attr_key: attr.key.to_string(),
                        attr_value: attr.value.to_string(),
                        err: err.to_string(),
                    }
                })?);
            } else if attr.key == Self::EVENT_ATTR_KEY_DENOMINATOR {
                denominator = Some(attr.value.parse().map_err(|err: StdError| {
                    WavsEventError::ParseAttribute {
                        event_type: Self::EVENT_TYPE.to_string(),
                        attr_key: attr.key.to_string(),
                        attr_value: attr.value.to_string(),
                        err: err.to_string(),
                    }
                })?);
            }
        }

        match (numerator, denominator) {
            (Some(numerator), Some(denominator)) => Ok(Self {
                numerator,
                denominator,
            }),
            (None, None) => Err(WavsEventError::MissingAttributes {
                event_type: Self::EVENT_TYPE.to_string(),
                attr_keys: vec![
                    Self::EVENT_ATTR_KEY_NUMERATOR.to_string(),
                    Self::EVENT_ATTR_KEY_DENOMINATOR.to_string(),
                ],
            }),
            (None, _) => Err(WavsEventError::MissingAttribute {
                event_type: Self::EVENT_TYPE.to_string(),
                attr_key: Self::EVENT_ATTR_KEY_NUMERATOR.to_string(),
            }),
            (_, None) => Err(WavsEventError::MissingAttribute {
                event_type: Self::EVENT_TYPE.to_string(),
                attr_key: Self::EVENT_ATTR_KEY_DENOMINATOR.to_string(),
            }),
        }
    }
}
