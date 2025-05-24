package core

import (
	"fmt"
	"time"

	"github.com/lfedgeai/spear/pkg/spear/proto/transport"
	"github.com/lfedgeai/spear/spearlet/task"
	log "github.com/sirupsen/logrus"
)

type HostCall struct {
	NameID  transport.Method
	Handler HostCallHandler
}

// invokation info
type InvocationInfo struct {
	Task     task.Task
	CommMgr  *CommunicationManager
	RespChan chan task.Message // channel to send response to client during streaming
}

type RespChanData struct {
	Resp    *transport.TransportResponse
	InvInfo *InvocationInfo
}

type ReqChanData struct {
	Req     *transport.TransportRequest
	InvInfo *InvocationInfo
}

type SignalCallbacks map[transport.Signal]func(task.Task, []byte) error

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
