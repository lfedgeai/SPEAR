package payload

const (
	HostCallVectorStoreCreate = "vectorstore.create"
	HostCallVectorStoreInsert = "vectorstore.insert"
	HostCallVectorStoreSearch = "vectorstore.search"
	HostCallVectorStoreDelete = "vectorstore.delete"

	HostCallMessagePassingRegister   = "messagepassing.register"
	HostCallMessagePassingUnregister = "messagepassing.unregister"
	HostCallMessagePassingLookup     = "messagepassing.lookup"
	HostCallMessagePassingSend       = "messagepassing.send"

	HostCallTransform              = "transform"
	HostCallToolNew                = "tool.new"
	HostCallToolsetNew             = "toolset.new"
	HostCallToolsetInstallBuiltins = "toolset.install.builtins"
)
