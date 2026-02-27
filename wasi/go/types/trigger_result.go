package types

import (
	wavs "github.com/Lay3rLabs/wavs-wasi/go/wavs/worker/layer-trigger-world"
	"go.bytecodealliance.org/cm"
)

// TriggerResult is the return type for wavs trigger world Run function
// an alias of wavs Exports Run() result type
type TriggerResult = cm.Result[wavs.OptionWasmResponseShape, cm.Option[wavs.WasmResponse], string]

// Ok (alias) returns an Ok response of type Some
func Ok(resp []byte, ordering cm.Option[uint64]) TriggerResult {
	return cm.OK[TriggerResult](cm.Some(wavs.WasmResponse{
		Payload:  cm.NewList(&resp[0], len(resp)),
		Ordering: ordering,
	}))
}

// OkNone (alias) returns an Ok response of type None
func OkNone() TriggerResult {
	return cm.OK[TriggerResult](cm.None[wavs.WasmResponse]())
}

// Err (alias) returns an Err response of type string
func Err(err string) TriggerResult {
	return cm.Err[TriggerResult](err)
}
