package tools

import (
	"fmt"
	"os"

	core "github.com/lfedgeai/spear/spearlet/core"
	"github.com/twilio/twilio-go"

	twilioApi "github.com/twilio/twilio-go/rest/api/v2010"
)

var (
	twilioAccountSid = os.Getenv("TWILIO_ACCOUNT_SID")
	twilioApiSecret  = os.Getenv("TWILIO_AUTH_TOKEN")
	twilioFrom       = os.Getenv("TWILIO_FROM")
)

var phoneTools = []core.ToolRegistry{
	{
		ToolType:    core.ToolType_Builtin,
		Name:        "phone_call",
		Id:          core.BuiltinToolID_PhoneCall,
		Description: "Call a phone number and play a message",
		Params: map[string]core.ToolParam{
			"phone_number": {
				Ptype:       "string",
				Description: "Phone number to send SMS to",
				Required:    true,
			},
			"message": {
				Ptype:       "string",
				Description: "Message to send, in TwiML format",
				Required:    true,
			},
		},
		CbBuiltIn: func(inv *core.InvocationInfo, args interface{}) (interface{}, error) {
			if twilioAccountSid == "" || twilioApiSecret == "" {
				return nil, fmt.Errorf("twilio credentials not set")
			}
			client := twilio.NewRestClientWithParams(twilio.ClientParams{
				Username: twilioAccountSid,
				Password: twilioApiSecret,
			})
			params := &twilioApi.CreateCallParams{}
			params.SetTo(args.(map[string]interface{})["phone_number"].(string))
			params.SetFrom(twilioFrom)
			params.SetTwiml(args.(map[string]interface{})["message"].(string))
			_, err := client.Api.CreateCall(params)
			if err != nil {
				return nil, err
			}
			return fmt.Sprintf("Call to %s successful", args.(map[string]interface{})["phone_number"].(string)), nil
		},
	},
}

func init() {
	for _, tool := range phoneTools {
		core.RegisterBuiltinTool(tool)
	}
}
