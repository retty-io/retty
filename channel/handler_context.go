package channel

type PipelineGetter interface {
	GetPipeline() *PipelineBase
	//TODO: GetTransport() Transport
}

type InboundHandlerContext[In any] interface {
	FireRead(msg In)
	FireReadEOF()
	FireReadError(err error)

	FireTransportActive()
	FireTransportInactive()
}

type OutboundHandlerContext[Out any] interface {
	FireWrite(msg Out)
	FireWriteError(err error)
	FireClose()
}

type HandlerContext[In any, Out any] interface {
	InboundHandlerContext[In]
	OutboundHandlerContext[Out]
}
