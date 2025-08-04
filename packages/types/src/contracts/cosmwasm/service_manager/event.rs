use cosmwasm_std::Uint256;

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
    type Error = cosmwasm_std::StdError;

    fn try_from(event: &cosmwasm_std::Event) -> Result<Self, Self::Error> {
        if event.ty != Self::EVENT_TYPE {
            return Err(cosmwasm_std::StdError::msg("Invalid event type"));
        }
        let service_uri = event
            .attributes
            .iter()
            .find(|attr| attr.key == Self::EVENT_ATTR_KEY_SERVICE_URI)
            .map(|attr| attr.value.to_string())
            .ok_or_else(|| cosmwasm_std::StdError::msg("Missing service URI attribute"))?;

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
    type Error = cosmwasm_std::StdError;

    fn try_from(event: &cosmwasm_std::Event) -> Result<Self, Self::Error> {
        if event.ty != Self::EVENT_TYPE {
            return Err(cosmwasm_std::StdError::msg("Invalid event type"));
        }

        let mut numerator: Option<Uint256> = None;
        let mut denominator: Option<Uint256> = None;

        for attr in &event.attributes {
            if attr.key == Self::EVENT_ATTR_KEY_NUMERATOR {
                numerator = Some(attr.value.parse()?);
            } else if attr.key == Self::EVENT_ATTR_KEY_DENOMINATOR {
                denominator = Some(attr.value.parse()?);
            }
        }

        match (numerator, denominator) {
            (Some(numerator), Some(denominator)) => Ok(Self {
                numerator,
                denominator,
            }),
            (None, None) => Err(cosmwasm_std::StdError::msg(
                "Missing numerator and denominator attributes",
            )),
            (None, _) => Err(cosmwasm_std::StdError::msg("Missing numerator attribute")),
            (_, None) => Err(cosmwasm_std::StdError::msg("Missing denominator attribute")),
        }
    }
}
