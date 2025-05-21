package streamresource

import (
	"fmt"

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
	return fmt.Errorf("unsupported stream operation %d", op)
}

func (r *dummyStreamResource) Notification(sc core.StreamBiChannel,
	op stream.NotifyEventType,
	data []byte) error {
	return fmt.Errorf("unsupported stream notification %d", op)
}

func init() {
	core.RegisterStreamResource("dummy", NewDummyStreamResource())
}
