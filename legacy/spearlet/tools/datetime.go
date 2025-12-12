package tools

import (
	"fmt"
	"time"

	core "github.com/lfedgeai/spear/spearlet/core"
)

var dtTools = []core.ToolRegistry{
	{
		ToolType:    core.ToolType_Builtin,
		Name:        "datetime",
		Id:          core.BuiltinToolID_Datetime,
		Description: "Get current date and time, including timezone information",
		Params:      map[string]core.ToolParam{},
		CbBuiltIn:   datetime,
	},
	{
		ToolType:    core.ToolType_Builtin,
		Name:        "sleep",
		Id:          core.BuiltinToolID_Sleep,
		Description: "Sleep for a specified number of seconds",
		Params: map[string]core.ToolParam{
			"seconds": {
				Ptype:       "integer",
				Description: "Number of seconds to sleep",
				Required:    true,
			},
		},
		CbBuiltIn: sleep,
	},
}

func sleep(inv *core.InvocationInfo, args interface{}) (interface{}, error) {
	secondsStr := args.(map[string]interface{})["seconds"]
	// it is either float64 or int
	seconds := int(secondsStr.(float64))
	time.Sleep(time.Duration(seconds) * time.Second)
	return fmt.Sprintf("Slept for %d seconds", seconds), nil
}

func datetime(inv *core.InvocationInfo, args interface{}) (interface{}, error) {
	return time.Now().Format(time.RFC3339), nil
}

func init() {
	for _, tool := range dtTools {
		core.RegisterBuiltinTool(tool)
	}
}
