package core

import (
	"fmt"

	flatbuffers "github.com/google/flatbuffers/go"
	"github.com/lfedgeai/spear/pkg/spear/proto/stream"
	"github.com/lfedgeai/spear/spearlet/task"
	log "github.com/sirupsen/logrus"
)

var (
	globalStreamResources = make(map[string]StreamResource)
)

type SessionID int

type StreamResource interface {
	Name() string
	Operation(sc StreamBiChannel, op stream.OperationType, data []byte) error
	Notification(sc StreamBiChannel, op stream.NotifyEventType, data []byte) error
}

type streamResource struct {
	name string
}

func NewStreamResource(name string) StreamResource {
	return &streamResource{
		name: name,
	}
}

func (r *streamResource) Name() string {
	return r.name
}

func (r *streamResource) Operation(sc StreamBiChannel,
	op stream.OperationType,
	data []byte) error {
	return fmt.Errorf("unsupported stream operation %d", op)
}

func (r *streamResource) Notification(sc StreamBiChannel,
	op stream.NotifyEventType,
	data []byte) error {
	return fmt.Errorf("unsupported stream notification %d", op)
}

func RegisterStreamResource(name string, r StreamResource) {
	if _, ok := globalStreamResources[name]; ok {
		panic(fmt.Sprintf("stream resource \"%s\" already registered", name))
	}
	globalStreamResources[name] = r
}

func UnregisterStreamResource(name string) {
	if _, ok := globalStreamResources[name]; !ok {
		panic(fmt.Sprintf("stream resource \"%s\" not registered", name))
	}
	delete(globalStreamResources, name)
}

func GetStreamResource(name string) StreamResource {
	if r, ok := globalStreamResources[name]; ok {
		return r
	}
	panic(fmt.Sprintf("stream resource \"%s\" not registered", name))
}

type StreamEventType int

const (
	StreamEventTypeOperation StreamEventType = iota
	StreamEventTypeNotification
)

type StreamBiChannel interface {
	StreamId() int32
	AddRequestStreamData(data []byte)
	ReplyNotifyEvent(resource string, ty stream.NotifyEventType,
		data []byte, final bool)
	ReplyOperationEvent(resource string, ty stream.OperationType,
		data []byte, final bool)
	SetDataHandler(handler func(data []byte))
	Stop()
}

type streamChannel struct {
	task      task.Task
	streamId  int32
	reqCh     chan []byte
	respCh    chan []byte
	respSeqId int64
	stopCh    chan struct{}
	handler   func(data []byte)
}

func NewStreamBiChannel(t task.Task, streamId int32) StreamBiChannel {
	res := &streamChannel{
		task:      t,
		streamId:  streamId,
		reqCh:     make(chan []byte, 128),
		respCh:    make(chan []byte, 128),
		respSeqId: 0,
		stopCh:    make(chan struct{}),
		handler:   nil,
	}

	go res.reqChanEventWorker()
	go res.respChanEventWorker()

	return res
}

func (p *streamChannel) StreamId() int32 {
	return p.streamId
}

func (p *streamChannel) AddRequestStreamData(data []byte) {
	p.reqCh <- data
}

func (p *streamChannel) ReplyNotifyEvent(resource string, ty stream.NotifyEventType,
	data []byte, final bool) {
	// put data inside a streamdata and send it to the respCh
	builder := flatbuffers.NewBuilder(0)
	resOff := builder.CreateString(resource)
	dataOff := builder.CreateByteVector(data)

	stream.StreamNotifyEventStart(builder)
	stream.StreamNotifyEventAddType(builder, ty)
	stream.StreamNotifyEventAddResource(builder, resOff)
	stream.StreamNotifyEventAddData(builder, dataOff)
	stream.StreamNotifyEventAddLength(builder, int32(len(data)))
	sneOff := stream.StreamNotifyEventEnd(builder)

	stream.StreamDataStart(builder)
	stream.StreamDataAddDataType(builder, stream.StreamDataWrapperStreamNotifyEvent)
	stream.StreamDataAddData(builder, sneOff)
	stream.StreamDataAddStreamId(builder, p.streamId)
	stream.StreamDataAddFinal(builder, final)
	stream.StreamDataAddSequenceId(builder, p.respSeqId)
	builder.Finish(stream.StreamDataEnd(builder))
	p.respCh <- builder.FinishedBytes()

	// increment the sequence id
	p.respSeqId++
}

func (p *streamChannel) ReplyOperationEvent(resource string, ty stream.OperationType,
	data []byte, final bool) {
	// put data inside a streamdata and send it to the respCh
	builder := flatbuffers.NewBuilder(0)
	resOff := builder.CreateString(resource)
	dataOff := builder.CreateByteVector(data)

	stream.StreamOperationEventStart(builder)
	stream.StreamOperationEventAddOp(builder, ty)
	stream.StreamOperationEventAddResource(builder, resOff)
	stream.StreamOperationEventAddData(builder, dataOff)
	stream.StreamOperationEventAddLength(builder, int32(len(data)))
	sneOff := stream.StreamOperationEventEnd(builder)

	stream.StreamDataStart(builder)
	stream.StreamDataAddDataType(builder, stream.StreamDataWrapperStreamOperationEvent)
	stream.StreamDataAddData(builder, sneOff)
	stream.StreamDataAddStreamId(builder, p.streamId)
	stream.StreamDataAddFinal(builder, final)
	stream.StreamDataAddSequenceId(builder, p.respSeqId)
	builder.Finish(stream.StreamDataEnd(builder))
	p.respCh <- builder.FinishedBytes()

	// increment the sequence id
	p.respSeqId++
}

func (p *streamChannel) respChanEventWorker() {
	respCh := p.respCh
	for {
		select {
		case <-p.stopCh:
			return
		case data := <-respCh:
			if p.handler != nil {
				log.Infof("stream channel reply %d data %d", p.streamId, len(data))
				p.handler(data)
			}
		}
	}
}

func (p *streamChannel) reqChanEventWorker() {
	for {
		select {
		case <-p.stopCh:
			return
		case data := <-p.reqCh:
			// process the request
			streamData := stream.GetRootAsStreamData(data, 0)
			if streamData == nil {
				fmt.Printf("failed to get stream data\n")
				continue
			}
			dataType := streamData.DataType()
			if dataType == stream.StreamDataWrapperStreamNotifyEvent {
				tbl := flatbuffers.Table{}
				if !streamData.Data(&tbl) {
					log.Errorf("failed to get stream notify event")
					continue
				}
				notify := stream.StreamNotifyEvent{}
				notify.Init(tbl.Bytes, tbl.Pos)
				resource := string(notify.Resource())
				notifyType := notify.Type()
				res := GetStreamResource(resource)
				if res == nil {
					log.Errorf("failed to get stream resource %s", resource)
					continue
				}
				if err := res.Notification(p, notifyType,
					notify.DataBytes()); err != nil {
					log.Errorf("failed to process stream notify event: %v",
						err)
				}
			} else if dataType == stream.StreamDataWrapperStreamOperationEvent {
				tbl := flatbuffers.Table{}
				if !streamData.Data(&tbl) {
					log.Errorf("failed to get stream operation event")
					continue
				}
				op := stream.StreamOperationEvent{}
				op.Init(tbl.Bytes, tbl.Pos)
				resource := string(op.Resource())
				opType := op.Op()
				res := GetStreamResource(resource)
				if res == nil {
					log.Errorf("failed to get stream resource %s", resource)
					continue
				}
				if err := res.Operation(p, opType, op.DataBytes()); err != nil {
					log.Errorf("failed to process stream operation event: %v",
						err)
				}
			} else {
				log.Errorf("unsupported stream data type %d",
					streamData.DataType())
				continue
			}
		}
	}
}

// set the function that will be called when new stream data is available
func (p *streamChannel) SetDataHandler(handler func(data []byte)) {
	log.Infof("set stream %d data handler", p.streamId)
	p.handler = handler
}

func (p *streamChannel) Stop() {
	close(p.stopCh)
	<-p.stopCh
	p.respCh = nil
	p.stopCh = nil
}
