package hostcalls

import (
	"fmt"

	flatbuffers "github.com/google/flatbuffers/go"

	"github.com/lfedgeai/spear/pkg/spear/proto/speech"
	"github.com/lfedgeai/spear/pkg/spear/proto/transform"
	helper "github.com/lfedgeai/spear/pkg/utils/protohelper"
	core "github.com/lfedgeai/spear/spearlet/core"
	oai "github.com/lfedgeai/spear/spearlet/hostcalls/openai"
	log "github.com/sirupsen/logrus"
)

func AudioASR(inv *core.InvocationInfo,
	args *transform.TransformRequest) ([]byte, error) {
	// verify the type of args is ASRRequest
	asrReq := speech.ASRRequest{}
	if err := helper.UnwrapTransformRequest(&asrReq, args); err != nil {
		return nil, fmt.Errorf("error unwrapping ASRRequest: %v", err)
	}

	req2 := &oai.OpenAISpeechToTextRequest{
		Model: string(asrReq.Model()),
		Audio: asrReq.AudioBytes(),
	}
	ep := core.GetAPIEndpointInfo(core.OpenAIFunctionTypeASR, req2.Model)
	if len(ep) == 0 {
		return nil, fmt.Errorf("error getting endpoint for model %s", req2.Model)
	}
	if len(ep) > 1 {
		log.Warn("multiple endpoints found for model %s, using first one", req2.Model)
	}

	req2.Model = ep[0].Model
	log.Infof("Using model %s", req2.Model)

	res, err := oai.OpenAISpeechToText(ep[0], req2)
	if err != nil {
		return nil, fmt.Errorf("error calling openai AudioASR: %v", err)
	}

	// convert to ASRResponse
	builder := flatbuffers.NewBuilder(0)
	textOff := builder.CreateString(res.Text)
	speech.ASRResponseStart(builder)
	speech.ASRResponseAddText(builder, textOff)
	asrOff := speech.ASRResponseEnd(builder)

	transform.TransformResponseStart(builder)
	transform.TransformResponseAddDataType(builder,
		transform.TransformResponse_Dataspear_proto_speech_ASRResponse)
	transform.TransformResponseAddData(builder, asrOff)
	builder.Finish(transform.TransformResponseEnd(builder))

	return builder.FinishedBytes(), nil
}

func speechToTextString(audio []byte, model string) (string, error) {
	req2 := &oai.OpenAISpeechToTextRequest{
		Model: model,
		Audio: audio,
	}
	ep := core.GetAPIEndpointInfo(core.OpenAIFunctionTypeASR, req2.Model)
	if len(ep) == 0 {
		return "", fmt.Errorf("error getting endpoint for model %s", req2.Model)
	}
	if len(ep) > 1 {
		log.Warn("multiple endpoints found for model %s, using first one", req2.Model)
	}

	res, err := oai.OpenAISpeechToText(ep[0], req2)
	if err != nil {
		return "", fmt.Errorf("error calling openai AudioASR: %v", err)
	}

	return res.Text, nil
}
