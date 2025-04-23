package common

import (
	"fmt"
	"sync"
	"time"

	flatbuffers "github.com/google/flatbuffers/go"
	"github.com/lfedgeai/spear/pkg/spear/proto/transport"
	"github.com/lfedgeai/spear/pkg/utils/protohelper"
	"github.com/lfedgeai/spear/spearlet/task"
	log "github.com/sirupsen/logrus"
)

type HostCall struct {
	NameID  transport.Method
	Handler HostCallHandler
}

// invokation info
type InvocationInfo struct {
	Task    task.Task
	CommMgr *CommunicationManager
}

type RespChanData struct {
	Resp    *transport.TransportResponse
	InvInfo *InvocationInfo
}

type ReqChanData struct {
	Req     *transport.TransportRequest
	InvInfo *InvocationInfo
}

type SignalCallbacks map[transport.Signal]func([]byte) error

// communication manager for hostcalls and guest responses
type CommunicationManager struct {
	respCh chan *RespChanData // incoming responses
	reqCh  chan *ReqChanData  // incoming requests
	outCh  map[task.Task]chan task.Message

	pendingRequests   map[int64]*requestCallback
	pendingRequestsMu sync.RWMutex

	taskSigCallbacks   map[task.Task]SignalCallbacks
	taskSigCallbacksMu sync.RWMutex
}

type HostCallHandler func(inv *InvocationInfo, args []byte) ([]byte, error)

type HostCalls struct {
	// map of hostcalls
	HCMap   map[transport.Method]HostCallHandler
	CommMgr *CommunicationManager
}

var ResponseTimeout = 5 * time.Minute

func NewHostCalls(commMgr *CommunicationManager) *HostCalls {
	return &HostCalls{
		HCMap:   make(map[transport.Method]HostCallHandler),
		CommMgr: commMgr,
	}
}

func (h *HostCalls) RegisterHostCall(hc *HostCall) error {
	nameId := hc.NameID
	handler := hc.Handler
	log.Debugf("Registering hostcall: %v", nameId)
	if _, ok := h.HCMap[nameId]; ok {
		return fmt.Errorf("hostcall already registered: %v", nameId)
	}
	h.HCMap[nameId] = handler
	return nil
}

func (h *HostCalls) Run() {
	for {
		entry := h.CommMgr.GetIncomingRequest()
		req := entry.Req
		inv := entry.InvInfo
		if handler, ok := h.HCMap[req.Method()]; ok {
			result, err := handler(inv, req.RequestBytes())
			if err != nil {
				log.Errorf("Error executing hostcall: %v", err)
				if err := h.CommMgr.SendOutgoingRPCResponseError(inv.Task, req.Id(), -1,
					err.Error()); err != nil {
					log.Errorf("Error sending response: %v", err)
				}
			} else {
				// send success response
				log.Infof("Hostcall success: %v, ID %d", req.Method(), req.Id())
				if err := h.CommMgr.SendOutgoingRPCResponse(inv.Task, req.Id(),
					result); err != nil {
					log.Errorf("Error sending response: %v", err)
				}
			}
		} else {
			log.Errorf("Hostcall not found: %v", req.Method())
			if err := h.CommMgr.SendOutgoingRPCResponseError(inv.Task, req.Id(), 2,
				"method not found"); err != nil {
				log.Errorf("Error sending response: %v", err)
			}
		}
	}
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
	}
}

func (c *CommunicationManager) InstallToTask(t task.Task) error {
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
	if err := cb(sig.PayloadBytes()); err != nil {
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

type ResquestCallback func(resp *transport.TransportResponse) error

type requestCallback struct {
	cb        ResquestCallback
	autoClear bool
	ts        time.Time
}

func (c *CommunicationManager) RegisterTaskSignalCallback(t task.Task,
	sig transport.Signal, cb func([]byte) error) {
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
