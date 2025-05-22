package streamresource

import (
	"github.com/lfedgeai/spear/pkg/spear/proto/stream"
	"github.com/lfedgeai/spear/spearlet/core"
)

type dummyStreamResource struct {
}

func NewDummyStreamResource() core.StreamResource {
	return &dummyStreamResource{}
}

func (r *dummyStreamResource) Name() string {
	return "dummy"
}

func (r *dummyStreamResource) Operation(sc core.StreamBiChannel,
	op stream.OperationType,
	data []byte) error {
	sc.ReplyNotifyEvent("op reply", stream.NotifyEventTypeCompleted,
		[]byte("dummy"), false)
	return nil
}

func (r *dummyStreamResource) Notification(sc core.StreamBiChannel,
	op stream.NotifyEventType,
	data []byte) error {
	sc.ReplyNotifyEvent("notify reply", stream.NotifyEventTypeCompleted,
		[]byte("dummy"), false)
	return nil
}

func init() {
	core.RegisterStreamResource("dummy", NewDummyStreamResource())
}
