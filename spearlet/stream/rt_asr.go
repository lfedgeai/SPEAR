package stream

import (
	"fmt"

	"github.com/lfedgeai/spear/pkg/spear/proto/stream"
	"github.com/lfedgeai/spear/spearlet/core"
)

type rtASRStreamFunction struct {
}

func NewRtASRStreamFunction() core.StreamFunction {
	return &rtASRStreamFunction{}
}

func (r *rtASRStreamFunction) Name() string {
	return "rt-asr"
}

func (r *rtASRStreamFunction) Operation(sc core.StreamBiChannel,
	op stream.OperationType,
	data []byte, final bool) error {
	// sc.WriteNotificationToTask("op reply", stream.NotificationEventTypeCompleted,
	// 	[]byte("dummy"), false)
	return fmt.Errorf("not implemented")
}

func (r *rtASRStreamFunction) Notification(sc core.StreamBiChannel,
	op stream.NotificationEventType,
	data []byte, final bool) error {
	// sc.WriteNotificationToTask("notification reply", stream.NotificationEventTypeCompleted,
	// 	[]byte("dummy"), false)
	return fmt.Errorf("not implemented")
}

func (r *rtASRStreamFunction) Raw(sc core.StreamBiChannel,
	data []byte, final bool) error {
	return fmt.Errorf("not implemented")
}

var (
	rtASRStreamClass = core.NewStreamClass("rt-asr")
)

func init() {
	core.RegisterStreamClass(rtASRStreamClass)
	if err := rtASRStreamClass.RegisterStreamFunction(NewRtASRStreamFunction()); err != nil {
		panic(err)
	}
}
