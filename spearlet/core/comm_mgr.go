package common

import (
	"fmt"
	"sync"
	"time"

	flatbuffers "github.com/google/flatbuffers/go"
	"github.com/lfedgeai/spear/pkg/spear/proto/stream"
	"github.com/lfedgeai/spear/pkg/spear/proto/transport"
	"github.com/lfedgeai/spear/pkg/utils/protohelper"
	"github.com/lfedgeai/spear/spearlet/task"
	log "github.com/sirupsen/logrus"
)

type ResquestCallback func(resp *transport.TransportResponse) error

type requestCallback struct {
	cb        ResquestCallback
	autoClear bool
	ts        time.Time
}

// communication manager for hostcalls and guest responses
type CommunicationManager struct {
	respCh chan *RespChanData // incoming responses
	reqCh  chan *ReqChanData  // incoming requests
	outCh  map[task.Task]chan task.Message

	pendingRequests   map[int64]*requestCallback
	pendingRequestsMu sync.RWMutex

	taskSigCallbacks   map[task.Task]SignalCallbacks
	taskSigCallbacksMu sync.RWMutex

	StreamBiChannels map[task.Task]map[int32]StreamBiChannel
}

func NewCommunicationManager() *CommunicationManager {
	return &CommunicationManager{
		respCh: make(chan *RespChanData, 1024),
		reqCh:  make(chan *ReqChanData, 1024),
		outCh:  make(map[task.Task]chan task.Message),

		pendingRequests:   make(map[int64]*requestCallback),
		pendingRequestsMu: sync.RWMutex{},

		taskSigCallbacks:   make(map[task.Task]SignalCallbacks),
		taskSigCallbacksMu: sync.RWMutex{},

		StreamBiChannels: make(map[task.Task]map[int32]StreamBiChannel),
	}
}

func (c *CommunicationManager) InitializeTaskData(t task.Task) error {
	if t == nil {
		log.Errorf("task is nil")
		return fmt.Errorf("task is nil")
	}

	// check in and out channel
	in, out, err := t.CommChannels()
	if err != nil {
		log.Errorf("Error getting communication channels: %v", err)
		return err
	}

	c.outCh[t] = in

	go func() {
		for msg := range out {
			// process message
			transRaw := transport.GetRootAsTransportMessageRaw(msg, 0)
			if transRaw == nil {
				log.Errorf("Error getting transport message raw")
				continue
			}
			if transRaw.DataType() == transport.TransportMessageRaw_DataTransportRequest {
				err := c.doResponse(t, transRaw)
				if err != nil {
					log.Errorf("Error processing response: %v", err)
				}
			} else if transRaw.DataType() == transport.TransportMessageRaw_DataTransportResponse {
				err := c.doRequest(t, transRaw)
				if err != nil {
					log.Errorf("Error processing request: %v", err)
				}
			} else if transRaw.DataType() == transport.TransportMessageRaw_DataTransportSignal {
				err := c.doSignal(t, transRaw)
				if err != nil {
					log.Errorf("Error processing signal: %v", err)
				}
			} else {
				log.Errorf("Invalid transport message type: %d", transRaw.DataType())
			}
		}
	}()

	c.StreamBiChannels[t] = make(map[int32]StreamBiChannel)

	t.RegisterOnFinish(func(t task.Task) {
		c.CleanupTask(t)
	})

	return nil
}

func (c *CommunicationManager) doResponse(t task.Task, transportRaw *transport.TransportMessageRaw) error {
	inv := InvocationInfo{
		Task:    t,
		CommMgr: c,
	}
	// request
	req := transport.TransportRequest{}
	// convert to transport request
	reqTbl := &flatbuffers.Table{}
	if !transportRaw.Data(reqTbl) {
		return fmt.Errorf("error getting transport request table")
	}
	req.Init(reqTbl.Bytes, reqTbl.Pos)
	log.Debugf("Hostcall received request: %d", req.Method())
	c.reqCh <- &ReqChanData{
		Req:     &req,
		InvInfo: &inv,
	}
	return nil
}

func (c *CommunicationManager) doRequest(t task.Task, transportRaw *transport.TransportMessageRaw) error {
	inv := InvocationInfo{
		Task:    t,
		CommMgr: c,
	}
	resp := transport.TransportResponse{}
	// convert to transport response
	respTbl := &flatbuffers.Table{}
	if !transportRaw.Data(respTbl) {
		return fmt.Errorf("error getting transport response table")
	}
	resp.Init(respTbl.Bytes, respTbl.Pos)
	log.Debugf("Hostcall received response: %d", resp.Id())
	go func() {
		// check if it is response to a pending request
		c.pendingRequestsMu.RLock()
		entry, ok := c.pendingRequests[resp.Id()]
		c.pendingRequestsMu.RUnlock()
		if ok {
			cb := entry.cb
			if err := cb(&resp); err != nil {
				log.Errorf("Error handling response: %v", err)
			}
			if entry.autoClear {
				c.pendingRequestsMu.Lock()
				delete(c.pendingRequests, resp.Id())
				c.pendingRequestsMu.Unlock()
			}
			return
		}

		// this is when we receive a response that is not a pending request
		c.respCh <- &RespChanData{
			Resp:    &resp,
			InvInfo: &inv,
		}
	}()
	return nil
}

func (c *CommunicationManager) doSignal(t task.Task, transportRaw *transport.TransportMessageRaw) error {
	sig := transport.TransportSignal{}
	sigTbl := &flatbuffers.Table{}
	if !transportRaw.Data(sigTbl) {
		return fmt.Errorf("error getting transport signal table")
	}
	sig.Init(sigTbl.Bytes, sigTbl.Pos)
	log.Debugf("Platform received signal: %s", sig.Method().String())
	// check if we have a callback for this signal
	c.taskSigCallbacksMu.RLock()
	if _, ok := c.taskSigCallbacks[t]; !ok {
		c.taskSigCallbacksMu.RUnlock()
		return fmt.Errorf("no signal callbacks registered for task: %v", t.Name())
	}
	if _, ok := c.taskSigCallbacks[t][sig.Method()]; !ok {
		c.taskSigCallbacksMu.RUnlock()
		return fmt.Errorf("no signal callback registered for task: %v, signal: %v", t.Name(),
			sig.Method())
	}
	cb := c.taskSigCallbacks[t][sig.Method()]
	c.taskSigCallbacksMu.RUnlock()
	// call the callback
	if err := cb(t, sig.PayloadBytes()); err != nil {
		return fmt.Errorf("error handling signal: %v", err)
	}
	return nil
}

func (c *CommunicationManager) GetIncomingRequest() *ReqChanData {
	return <-c.reqCh
}

func (c *CommunicationManager) GetIncomingResponse() *RespChanData {
	return <-c.respCh
}

func (c *CommunicationManager) SendOutgoingRPCResponseError(t task.Task, id int64, code int,
	msg string) error {
	resp := protohelper.CreateErrorTransportResponse(id, code, msg)
	if resp == nil {
		return fmt.Errorf("error creating response")
	}
	data, err := protohelper.TransportResponseToRaw(resp)
	if err != nil {
		return err
	}
	c.outCh[t] <- data
	return nil
}

func (c *CommunicationManager) SendOutgoingRPCResponse(t task.Task, id int64,
	result []byte) error {
	raw, err := protohelper.RPCBufferResponseToRaw(id, result)
	if err != nil {
		return err
	}

	c.outCh[t] <- raw
	return nil
}

func (c *CommunicationManager) RegisterTaskSignalCallback(t task.Task,
	sig transport.Signal, cb func(task.Task, []byte) error) {
	c.taskSigCallbacksMu.Lock()
	defer c.taskSigCallbacksMu.Unlock()
	if t == nil {
		log.Errorf("task is nil")
		return
	}
	if _, ok := c.taskSigCallbacks[t]; !ok {
		c.taskSigCallbacks[t] = make(SignalCallbacks)
	}
	c.taskSigCallbacks[t][sig] = cb
}

func (c *CommunicationManager) UnregisterTaskSignalCallback(t task.Task,
	sig transport.Signal) {
	c.taskSigCallbacksMu.Lock()
	defer c.taskSigCallbacksMu.Unlock()
	if t == nil {
		log.Errorf("task is nil")
		return
	}
	if _, ok := c.taskSigCallbacks[t]; !ok {
		return
	}
	if _, ok := c.taskSigCallbacks[t][sig]; !ok {
		return
	}
	delete(c.taskSigCallbacks[t], sig)
	if len(c.taskSigCallbacks[t]) == 0 {
		delete(c.taskSigCallbacks, t)
	}
}

func (c *CommunicationManager) SendOutgoingRPCSignal(t task.Task, signal transport.Signal,
	data []byte) error {
	data, err := protohelper.RPCSignalToRaw(signal, data)
	if err != nil {
		return err
	}

	c.outCh[t] <- data
	return nil
}

// req_buffer is
func (c *CommunicationManager) SendOutgoingRPCRequestCallback(t task.Task, id int64,
	method transport.Method,
	req_buffer []byte, cb func(*transport.TransportResponse) error) error {
	if len(req_buffer) == 0 {
		return fmt.Errorf("request is nil")
	}

	data, err := protohelper.RPCBufferResquestToRaw(id, method, req_buffer)
	if err != nil {
		return err
	}

	c.outCh[t] <- data
	c.pendingRequestsMu.Lock()
	c.pendingRequests[id] = &requestCallback{
		cb:        cb,
		autoClear: true,
		ts:        time.Now(),
	}
	c.pendingRequestsMu.Unlock()
	return nil
}

// users need to specify the id in the request
// req_buffer is the serialized transport.TransportRequest
func (c *CommunicationManager) SendOutgoingRPCRequest(t task.Task, method transport.Method,
	req_buffer []byte) (*transport.TransportResponse, error) {
	ch := make(chan *transport.TransportResponse, 1)
	errCh := make(chan error, 1)

	req := transport.GetRootAsTransportRequest(req_buffer, 0)
	if req == nil {
		return nil, fmt.Errorf("error getting transport request")
	}

	if err := c.SendOutgoingRPCRequestCallback(t, int64(t.NextRequestID()), method, req_buffer,
		func(resp *transport.TransportResponse) error {
			if resp.Code() != 0 {
				errCh <- fmt.Errorf("error response: %d, %s", resp.Code(), string(resp.Message()))
			} else {
				ch <- resp
			}
			return nil
		}); err != nil {
		return nil, err
	}

	select {
	case resp := <-ch:
		return resp, nil
	case err := <-errCh:
		return nil, err
	case <-time.After(ResponseTimeout):
		return nil, fmt.Errorf("timeout")
	}
}

func (c *CommunicationManager) SendOutgoingNotifyEvent(t task.Task, resource string, etype stream.NotifyEventType,
	data []byte, final bool) error {
	if etype == stream.NotifyEventTypeError {
		return fmt.Errorf("error notify event type")
	}
	builder := flatbuffers.NewBuilder(0)
	resourceOff := builder.CreateString(resource)
	dataOff := builder.CreateByteVector(data)

	stream.StreamNotifyEventStart(builder)
	stream.StreamNotifyEventAddType(builder, etype)
	stream.StreamNotifyEventAddResource(builder, resourceOff)
	stream.StreamNotifyEventAddData(builder, dataOff)
	notifyOff := stream.StreamNotifyEventEnd(builder)

	stream.StreamDataStart(builder)
	stream.StreamDataAddData(builder, notifyOff)
	stream.StreamDataAddDataType(builder, stream.StreamDataWrapperStreamNotifyEvent)
	stream.StreamDataAddFinal(builder, final)

	builder.Finish(notifyOff)

	if err := c.SendOutgoingRPCSignal(t, transport.SignalStreamData, builder.FinishedBytes()); err != nil {
		return err
	}
	log.Debugf("Send stream notify event: %s, %d", resource, etype)
	return nil
}

func (c *CommunicationManager) CleanupTask(t task.Task) {
	c.cleanupOutCh(t)
	c.cleanupTaskSignalCallbacks(t)
	c.cleanupStreamBiChannels(t)
}

func (c *CommunicationManager) cleanupStreamBiChannels(t task.Task) {
	if _, ok := c.StreamBiChannels[t]; !ok {
		return
	}
	for streamId, p := range c.StreamBiChannels[t] {
		p.Stop()
		delete(c.StreamBiChannels[t], streamId)
	}
	if len(c.StreamBiChannels[t]) == 0 {
		delete(c.StreamBiChannels, t)
	}
}

func (c *CommunicationManager) cleanupTaskSignalCallbacks(t task.Task) {
	c.taskSigCallbacksMu.Lock()
	defer c.taskSigCallbacksMu.Unlock()
	if _, ok := c.taskSigCallbacks[t]; !ok {
		return
	}
	delete(c.taskSigCallbacks, t)
}

func (c *CommunicationManager) cleanupOutCh(t task.Task) {
	if _, ok := c.outCh[t]; !ok {
		return
	}
	close(c.outCh[t])
	delete(c.outCh, t)
}
