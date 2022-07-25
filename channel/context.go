package channel

type HandlerContextBase interface {
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

type PipelineContext[H any, HandlerContext any] interface {
	AttachPipeline()
	DetachPipeline()

	SetNextIn(ctx PipelineContext[H, HandlerContext])
	SetNextOut(ctx PipelineContext[H, HandlerContext])

	GetDirection() HandlerDir
}

type InboundLink[In any] interface {
	Read(msg In)
	ReadEOF()
	ReadError(err error)

	TransportActive()
	TransportInactive()
}

type OutboundLink[Out any] interface {
	Write(msg Out)
	WriteError(err error)
	Close()
}
