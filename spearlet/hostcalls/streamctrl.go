package hostcalls

import (
	"fmt"
	"math/rand"

	flatbuffers "github.com/google/flatbuffers/go"
	"github.com/lfedgeai/spear/pkg/spear/proto/stream"
	hcommon "github.com/lfedgeai/spear/spearlet/core"
	log "github.com/sirupsen/logrus"
)

func StreamCtrl(inv *hcommon.InvocationInfo,
	args []byte) ([]byte, error) {
	req := stream.GetRootAsStreamControlRequest(args, 0)
	if req == nil {
		return nil, fmt.Errorf("could not get StreamControlRequest")
	}

	taskStreamBiChannels, ok := inv.CommMgr.StreamBiChannels[inv.Task]
	if !ok {
		return nil, fmt.Errorf("could not get task streams")
	}
	switch req.Op() {
	case stream.StreamControlOpsNew:
		// gernerate a positive random int32 stream id
		streamId := rand.Int31n(1 << 30)
		// check if the stream id is already used
		if _, ok := taskStreamBiChannels[streamId]; ok {
			return nil, fmt.Errorf("stream id %d already used", streamId)
		}
		// create a new stream
		c := hcommon.NewStreamBiChannel(inv.Task,
			streamId)
		c.SetDataHandler(func(data []byte) {
			// send the data to the task
			log.Infof("stream data %d", streamId)
			// TODO: send the data to the task
			panic("not implemented")
		})
		inv.CommMgr.StreamBiChannels[inv.Task][streamId] = c
		builder := flatbuffers.NewBuilder(0)
		stream.StreamControlResponseStart(builder)
		stream.StreamControlResponseAddRequestId(builder, req.RequestId())
		stream.StreamControlResponseAddStreamId(builder, streamId)
		builder.Finish(stream.StreamControlResponseEnd(builder))
		return builder.FinishedBytes(), nil
	case stream.StreamControlOpsClose:
		streamId := req.StreamId()
		// check if the stream id is used
		if p, ok := taskStreamBiChannels[streamId]; !ok {
			return nil, fmt.Errorf("stream id %d not used", streamId)
		} else {
			// stop the stream channel
			p.Stop()
		}
		// close the stream
		delete(taskStreamBiChannels, streamId)
		builder := flatbuffers.NewBuilder(0)
		stream.StreamControlResponseStart(builder)
		stream.StreamControlResponseAddRequestId(builder, req.RequestId())
		stream.StreamControlResponseAddStreamId(builder, streamId)
		builder.Finish(stream.StreamControlResponseEnd(builder))
		return builder.FinishedBytes(), nil
	}
	return nil, fmt.Errorf("unsupported stream control operation %d", req.Op())
}
