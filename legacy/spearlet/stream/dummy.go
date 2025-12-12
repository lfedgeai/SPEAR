package stream

import (
	"fmt"

	"github.com/lfedgeai/spear/pkg/spear/proto/stream"
	"github.com/lfedgeai/spear/spearlet/core"
)

type dummyStreamFunction struct {
}

func NewDummyStreamFunction() core.StreamFunction {
	return &dummyStreamFunction{}
}

func (r *dummyStreamFunction) Name() string {
	return "dummy"
}

func (r *dummyStreamFunction) Operation(sc core.StreamBiChannel,
	op stream.OperationType,
	data []byte, final bool) error {
	sc.WriteNotificationToTask("op reply", stream.NotificationEventTypeCompleted,
		[]byte("dummy"), false)
	return nil
}

func (r *dummyStreamFunction) Notification(sc core.StreamBiChannel,
	op stream.NotificationEventType,
	data []byte, final bool) error {
	sc.WriteNotificationToTask("notification reply", stream.NotificationEventTypeCompleted,
		[]byte("dummy"), false)
	return nil
}

func (r *dummyStreamFunction) Raw(sc core.StreamBiChannel,
	data []byte, final bool) error {
	return fmt.Errorf("not implemented")
}

var (
	dummyStreamClass = core.NewStreamClass("dummy")
)

func init() {
	core.RegisterStreamClass(dummyStreamClass)
	if err := dummyStreamClass.RegisterStreamFunction(NewDummyStreamFunction()); err != nil {
		panic(err)
	}
}
