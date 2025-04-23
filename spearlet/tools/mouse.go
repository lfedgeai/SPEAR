package tools

import (
	"time"

	"github.com/go-vgo/robotgo"
	core "github.com/lfedgeai/spear/spearlet/core"
)

var mouseTools = []core.ToolRegistry{
	{
		ToolType:    core.ToolType_Builtin,
		Name:        "mouse_right_click",
		Id:          core.BuiltinToolID_MouseRightClick,
		Description: `Right click the mouse at the current location.`,
		Params:      map[string]core.ToolParam{},
		CbBuiltIn: func(inv *core.InvocationInfo, args interface{}) (interface{}, error) {
			robotgo.Toggle("right")
			time.Sleep(300 * time.Millisecond)
			robotgo.Toggle("right", "up")
			return "Right click successful", nil
		},
	},
	{
		ToolType:    core.ToolType_Builtin,
		Name:        "mouse_left_click",
		Id:          core.BuiltinToolID_MouseLeftClick,
		Description: `Left click the mouse at the current location.`,
		Params:      map[string]core.ToolParam{},
		CbBuiltIn: func(inv *core.InvocationInfo, args interface{}) (interface{}, error) {
			robotgo.Toggle("left")
			time.Sleep(300 * time.Millisecond)
			robotgo.Toggle("left", "up")
			return "Left click successful", nil
		},
	},
}

func init() {
	for _, tool := range mouseTools {
		core.RegisterBuiltinTool(tool)
	}
}
