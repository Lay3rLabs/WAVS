package types

import (
	"github.com/defiweb/go-eth/abi"
	"github.com/defiweb/go-eth/types"
)

// DataWithIdABI defines the ABI structure used for encoding return values
var DataWithIdABI = abi.MustParseStruct(`struct DataWithId { uint64 triggerId; bytes data; }`)

// DataWithID contains data to be sent back to the contract
type DataWithID struct {
	TriggerID uint64 `abi:"triggerId"`
	Data      []byte `abi:"data"`
}

// TriggerInfo contains all the data related to a trigger event from the contract
type TriggerInfo struct {
	TriggerID uint64        `abi:"triggerId"`
	Creator   types.Address `abi:"creator"`
	Data      []byte        `abi:"data"`
}
