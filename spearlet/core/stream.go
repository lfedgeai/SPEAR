package core

import (
	"fmt"
	"sync"

	flatbuffers "github.com/google/flatbuffers/go"
	"github.com/lfedgeai/spear/pkg/spear/proto/stream"
	"github.com/lfedgeai/spear/pkg/spear/proto/transport"
	log "github.com/sirupsen/logrus"
)

var (
	globalStreamClasses = make(map[string]StreamClass)
)

type StreamClass interface {
	Name() string
	GetStreamFunction(name string) StreamFunction
	RegisterStreamFunction(r StreamFunction) error
	UnregisterStreamFunction(name string) error
}

type StreamFunction interface {
	Name() string
	Operation(sc StreamBiChannel, op stream.OperationType, data []byte, final bool) error
	Notification(sc StreamBiChannel, op stream.NotificationEventType, data []byte, final bool) error
	Raw(sc StreamBiChannel, data []byte, final bool) error
}

type streamClass struct {
	name string

	functions map[string]StreamFunction
}

func NewStreamClass(name string) StreamClass {
	return &streamClass{
		name:      name,
		functions: make(map[string]StreamFunction),
	}
}

func (r *streamClass) Name() string {
	return r.name
}

func (r *streamClass) GetStreamFunction(name string) StreamFunction {
	if f, ok := r.functions[name]; ok {
		return f
	}
	return nil
}

func (r *streamClass) RegisterStreamFunction(f StreamFunction) error {
	if _, ok := r.functions[f.Name()]; ok {
		return fmt.Errorf("stream function \"%s\" already registered", f.Name())
	}
	r.functions[f.Name()] = f
	return nil
}

func (r *streamClass) UnregisterStreamFunction(name string) error {
	if _, ok := r.functions[name]; !ok {
		return fmt.Errorf("stream function \"%s\" not registered", name)
	}
	delete(r.functions, name)
	return nil
}

type streamFunction struct {
	name string
}

func RegisterStreamClass(r StreamClass) {
	if _, ok := globalStreamClasses[r.Name()]; ok {
		panic(fmt.Sprintf("stream class \"%s\" already registered", r.Name()))
	}
	globalStreamClasses[r.Name()] = r
}

func UnregisterStreamClass(name string) {
	if _, ok := globalStreamClasses[name]; !ok {
		panic(fmt.Sprintf("stream class \"%s\" not registered", name))
	}
	delete(globalStreamClasses, name)
}

type StreamEventType int

const (
	StreamEventTypeOperation StreamEventType = iota
	StreamEventTypeNotification
)

type StreamBiChannel interface {
	StreamId() int32
	GetInvocationInfo() *InvocationInfo
	Stop()

	WriteStreamDataForHost(data []byte)
	WriteNotificationToTask(name string, ty stream.NotificationEventType,
		data []byte, final bool)
	WriteOperationToTask(name string, ty stream.OperationType,
		data []byte, final bool)
	WriteRawToTask(data []byte, final bool)

	Flush() error
}

type streamChannel struct {
	invInfo  *InvocationInfo
	streamId int32

	reqCh  chan []byte // requests from the task
	respCh chan []byte // responses to the task
	respWg sync.WaitGroup

	respSeqId int64
	stopCh    chan struct{}
	handler   func(data []byte)
	class     StreamClass
}

func NewStreamBiChannel(inv *InvocationInfo, streamId int32, className string) (StreamBiChannel, error) {
	if inv == nil {
		return nil, fmt.Errorf("invocation info is nil")
	}
	if inv.CommMgr == nil {
		return nil, fmt.Errorf("communication manager is nil")
	}
	if inv.Task == nil {
		return nil, fmt.Errorf("task is nil")
	}
	res := &streamChannel{
		invInfo:   inv,
		streamId:  streamId,
		reqCh:     make(chan []byte, 128),
		respCh:    make(chan []byte, 128),
		respWg:    sync.WaitGroup{},
		respSeqId: 0,
		stopCh:    make(chan struct{}),
	}

	if class, ok := globalStreamClasses[className]; ok {
		res.class = class
	} else {
		return nil, fmt.Errorf("failed to get stream class \"%s\"", className)
	}

	res.handler = func(data []byte) {
		if err := res.invInfo.CommMgr.SendOutgoingRPCSignal(
			res.invInfo.Task,
			transport.SignalStreamData,
			data,
		); err != nil {
			log.Errorf("failed to send stream data %d: %v",
				streamId, err)
		}
	}

	go res.reqChanEventWorker()
	go res.respChanEventWorker()

	return res, nil
}

func (p *streamChannel) GetInvocationInfo() *InvocationInfo {
	return p.invInfo
}

func (p *streamChannel) StreamId() int32 {
	return p.streamId
}

func (p *streamChannel) WriteStreamDataForHost(data []byte) {
	if p.reqCh == nil {
		panic("stream channel is nil")
	}
	p.reqCh <- data
}

func (p *streamChannel) WriteNotificationToTask(name string, ty stream.NotificationEventType,
	data []byte, final bool) {
	// put data inside a streamdata and send it to the respCh
	builder := flatbuffers.NewBuilder(0)
	resOff := builder.CreateString(name)
	dataOff := builder.CreateByteVector(data)

	stream.StreamNotificationEventStart(builder)
	stream.StreamNotificationEventAddType(builder, ty)
	stream.StreamNotificationEventAddName(builder, resOff)
	stream.StreamNotificationEventAddData(builder, dataOff)
	stream.StreamNotificationEventAddLength(builder, int32(len(data)))
	sneOff := stream.StreamNotificationEventEnd(builder)

	stream.StreamDataStart(builder)
	stream.StreamDataAddDataType(builder, stream.StreamDataWrapperStreamNotificationEvent)
	stream.StreamDataAddData(builder, sneOff)
	stream.StreamDataAddStreamId(builder, p.streamId)
	stream.StreamDataAddFinal(builder, final)
	stream.StreamDataAddSequenceId(builder, p.respSeqId)
	builder.Finish(stream.StreamDataEnd(builder))
	p.respWg.Add(1)
	p.respCh <- builder.FinishedBytes()

	// increment the sequence id
	p.respSeqId++
}

func (p *streamChannel) WriteOperationToTask(name string, ty stream.OperationType,
	data []byte, final bool) {
	// put data inside a streamdata and send it to the respCh
	builder := flatbuffers.NewBuilder(0)
	resOff := builder.CreateString(name)
	dataOff := builder.CreateByteVector(data)

	stream.StreamOperationEventStart(builder)
	stream.StreamOperationEventAddOp(builder, ty)
	stream.StreamOperationEventAddName(builder, resOff)
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
	p.respWg.Add(1)
	p.respCh <- builder.FinishedBytes()

	// increment the sequence id
	p.respSeqId++
}

func (p *streamChannel) WriteRawToTask(data []byte, final bool) {
	// put data inside a streamdata and send it to the respCh
	builder := flatbuffers.NewBuilder(0)
	dataOff := builder.CreateByteVector(data)

	stream.StreamRawDataStart(builder)
	stream.StreamRawDataAddData(builder, dataOff)
	stream.StreamRawDataAddLength(builder, int32(len(data)))
	srOff := stream.StreamRawDataEnd(builder)

	stream.StreamDataStart(builder)
	stream.StreamDataAddDataType(builder, stream.StreamDataWrapperStreamRawData)
	stream.StreamDataAddData(builder, srOff)
	stream.StreamDataAddStreamId(builder, p.streamId)
	stream.StreamDataAddFinal(builder, final)
	stream.StreamDataAddSequenceId(builder, p.respSeqId)
	builder.Finish(stream.StreamDataEnd(builder))
	p.respWg.Add(1)
	p.respCh <- builder.FinishedBytes()

	// increment the sequence id
	p.respSeqId++
}

func (p *streamChannel) Flush() error {
	if p.respCh == nil {
		return fmt.Errorf("stream channel is stopped")
	}
	// wait for all responses to be processed
	p.respWg.Wait()
	return nil
}

func (p *streamChannel) respChanEventWorker() {
	respCh := p.respCh
	for {
		select {
		case <-p.stopCh:
			return
		case data := <-respCh:
			if p.handler != nil {
				p.handler(data)
			}
			p.respWg.Done()
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
			if dataType == stream.StreamDataWrapperStreamNotificationEvent {
				tbl := flatbuffers.Table{}
				if !streamData.Data(&tbl) {
					log.Errorf("failed to get stream notification event")
					continue
				}
				notification := stream.StreamNotificationEvent{}
				notification.Init(tbl.Bytes, tbl.Pos)
				name := string(notification.Name())
				notificationType := notification.Type()
				final := streamData.Final()
				res := p.class.GetStreamFunction(name)
				if res == nil {
					log.Errorf("failed to get stream function %s", name)
					continue
				}
				if err := res.Notification(p, notificationType,
					notification.DataBytes(), final); err != nil {
					log.Errorf("failed to process stream notification event: %v",
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
				name := string(op.Name())
				opType := op.Op()
				final := streamData.Final()
				res := p.class.GetStreamFunction(name)
				if res == nil {
					log.Errorf("failed to get stream function %s", name)
					continue
				}
				if err := res.Operation(p, opType, op.DataBytes(), final); err != nil {
					log.Errorf("failed to process stream operation event: %v",
						err)
				}
			} else if dataType == stream.StreamDataWrapperStreamRawData {
				tbl := flatbuffers.Table{}
				if !streamData.Data(&tbl) {
					log.Errorf("failed to get stream raw data")
					continue
				}
				rawData := stream.StreamRawData{}
				rawData.Init(tbl.Bytes, tbl.Pos)
				final := streamData.Final()
				res := p.class.GetStreamFunction("io")
				if res == nil {
					log.Errorf("failed to get stream function io")
					continue
				}
				if rawData.Length() == 0 {
					if err := res.Raw(p, []byte{}, final); err != nil {
						log.Errorf("failed to process stream raw data: %v",
							err)
					}
				} else {
					if err := res.Raw(p, rawData.DataBytes(), final); err != nil {
						log.Errorf("failed to process stream raw data: %v",
							err)
					}
				}
			} else {
				log.Errorf("unsupported stream data type %d",
					streamData.DataType())
				continue
			}
		}
	}
}

func (p *streamChannel) Stop() {
	close(p.stopCh)
	<-p.stopCh
	p.respCh = nil
	p.stopCh = nil
}
