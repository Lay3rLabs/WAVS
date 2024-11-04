use std::sync::LazyLock;

use layer_climb::prelude::*;

pub static MOCK_TASK_QUEUE_ADDRESS: LazyLock<Address> = LazyLock::new(|| {
    // randomly generated
    // Mnemonic: slide pilot laundry assault mouse rice march mammal cost ranch pipe list feel sea aerobic lottery soul lazy flush ozone clerk apple gadget harbor
    Address::new_cosmos_string("layer18cpv22kxz9g7yljyvh309vd7al5qx40av3edkt", None).unwrap()
});
