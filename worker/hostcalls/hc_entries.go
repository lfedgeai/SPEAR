package hostcalls

import (
	"github.com/lfedgeai/spear/pkg/rpc/payload"
	hostcalls "github.com/lfedgeai/spear/worker/hostcalls/common"
)

var Hostcalls = []*hostcalls.HostCall{
	{
		Name:    payload.HostCallTransform,
		Handler: Transform,
	},
	{
		Name:    payload.HostCallTransformConfig,
		Handler: TransformConfig,
	},
	{
		Name:    payload.HostCallToolNew,
		Handler: NewTool,
	},
	{
		Name:    payload.HostCallToolsetNew,
		Handler: NewToolset,
	},
	{
		Name:    payload.HostCallToolsetInstallBuiltins,
		Handler: ToolsetInstallBuiltins,
	},
	// // chat operations
	// {
	// 	Name:    transform.HostCallChatCompletion,
	// 	Handler: ChatCompletionWithTools,
	// },
	// // text to speech operations
	// {
	// 	Name:    openai.HostCallTextToSpeech,
	// 	Handler: openaihc.TextToSpeech,
	// },
	// // image generation operations
	// {
	// 	Name:    openai.HostCallImageGeneration,
	// 	Handler: openaihc.ImageGeneration,
	// },
	// // embeddings operations
	// {
	// 	Name:    openai.HostCallEmbeddings,
	// 	Handler: openaihc.Embeddings,
	// },
	// vector store operations
	{
		Name:    payload.HostCallVectorStoreCreate,
		Handler: VectorStoreCreate,
	},
	{
		Name:    payload.HostCallVectorStoreDelete,
		Handler: VectorStoreDelete,
	},
	{
		Name:    payload.HostCallVectorStoreInsert,
		Handler: VectorStoreInsert,
	},
	{
		Name:    payload.HostCallVectorStoreSearch,
		Handler: VectorStoreSearch,
	},
	// message passing operations
	{
		Name:    payload.HostCallMessagePassingRegister,
		Handler: MessagePassingRegister,
	},
	{
		Name:    payload.HostCallMessagePassingUnregister,
		Handler: MessagePassingUnregister,
	},
	{
		Name:    payload.HostCallMessagePassingLookup,
		Handler: MessagePassingLookup,
	},
	{
		Name:    payload.HostCallMessagePassingSend,
		Handler: MessagePassingSend,
	},
	// input operations
	{
		Name:    payload.HostCallInput,
		Handler: Input,
	},
	// speak operations
	{
		Name:    payload.HostCallSpeak,
		Handler: Speak,
	},
	// record operations
	{
		Name:    payload.HostCallRecord,
		Handler: Record,
	},
}
