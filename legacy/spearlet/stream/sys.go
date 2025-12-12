package stream

import (
	"fmt"

	log "github.com/sirupsen/logrus"

	"github.com/lfedgeai/spear/pkg/spear/proto/stream"
	"github.com/lfedgeai/spear/spearlet/core"
	"github.com/lfedgeai/spear/spearlet/task"
)

const (
	SysIOStreamFunctionName = "io"
	SysIOStreamClassName    = "sys"
)

type sysIOStreamFunction struct {
}

func NewSysIOStreamFunction() core.StreamFunction {
	return &sysIOStreamFunction{}
}

func (r *sysIOStreamFunction) Name() string {
	return SysIOStreamFunctionName
}

func (r *sysIOStreamFunction) Operation(sc core.StreamBiChannel,
	op stream.OperationType,
	data []byte, final bool) error {
	return fmt.Errorf("not implemented")
}

func (r *sysIOStreamFunction) Notification(sc core.StreamBiChannel,
	op stream.NotificationEventType,
	data []byte, final bool) error {
	return fmt.Errorf("not implemented")
}

func (r *sysIOStreamFunction) Raw(sc core.StreamBiChannel,
	data []byte, final bool) error {
	if sc.StreamId() != 0 {
		return fmt.Errorf("sysio stream id is not 0")
	}
	inv := sc.GetInvocationInfo()
	if inv == nil {
		panic("invocation info is nil")
	}
	resp := inv.RespChan
	if resp == nil {
		panic("response channel is nil")
	}

	if len(data) > 0 {
		resp <- task.Message(data)
	} else {
		log.Debugf("raw data is empty")
	}
	if final {
		log.Debugf("sysio stream ended")
		close(resp)
	}
	return nil
}

var (
	sysStreamClass = core.NewStreamClass(SysIOStreamClassName)
)

func init() {
	core.RegisterStreamClass(sysStreamClass)
	if err := sysStreamClass.RegisterStreamFunction(NewSysIOStreamFunction()); err != nil {
		panic(err)
	}
}
