package types

import (
	"encoding/binary"
	"encoding/hex"

	"github.com/defiweb/go-eth/abi"
	"github.com/defiweb/go-eth/types"
)

// DecodeTriggerInfo decodes raw log data into a structured TriggerInfo
func DecodeTriggerInfo(rawLog []byte) TriggerInfo {
	sections := ChunkBytes(rawLog, 32)

	// TriggerID from section 4 (last 8 bytes of the 32-byte chunk)
	triggerIdSection := sections[3]
	triggerID := binary.BigEndian.Uint64(triggerIdSection[len(triggerIdSection)-8:])

	// Creator address from section 5 (last 20 bytes of the 32-byte chunk)
	creatorSection := sections[4]
	creatorAddress := types.MustAddressFromBytes(creatorSection[12:32]) // Ethereum addresses are 20 bytes

	// Data length from section 7 (last byte of the 32-byte chunk)
	dataLengthSection := sections[6]
	dataLength := int(dataLengthSection[31])

	// Actual data from section 8 (first dataLength bytes)
	dataSection := sections[7]
	data := make([]byte, dataLength)
	copy(data, dataSection[:dataLength])

	return TriggerInfo{
		TriggerID: triggerID,
		Creator:   creatorAddress,
		Data:      data,
	}
}

// EncodeTriggerOutput abi encodes the output of the computation to be sent back to the Ethereum contract

func EncodeTriggerOutput(trigger_id uint64, output []byte) []byte {
	/*
		for trigger_id of 1 and output of `test-data`, the proper solidity encoding is:

		Offset to the start of the struct (32 bytes)
		0x0000000000000000000000000000000000000000000000000000000000000020
		triggerId: 1 (uint64)
		0000000000000000000000000000000000000000000000000000000000000001
		Offset to the dynamic bytes field (64 bytes from struct start)
		0000000000000000000000000000000000000000000000000000000000000040
		Length of bytes array: 9
		0000000000000000000000000000000000000000000000000000000000000009
		Content: "test-data" with padding
		746573742d646174610000000000000000000000000000000000000000000000

		this is not how the abi encoding library works, so manually prepend the offset bytes to the data.
		verified by console.log in the solidity contract of some encoded test data, then cross comparing
		to this functions output.
	*/

	bz := abi.MustEncodeValue(DataWithIdABI, DataWithID{
		TriggerID: trigger_id,
		Data:      output,
	})

	offsetBytes, _ := hex.DecodeString("0000000000000000000000000000000000000000000000000000000000000020")
	return append(offsetBytes, bz...)
}
