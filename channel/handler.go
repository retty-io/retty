package channel

type HandlerDir int

const (
	HandlerDirIn HandlerDir = iota
	HandlerDirOut
	HandlerDirBoth
)

type HandlerBase[Context any] interface {
	GetDirection() HandlerDir
	GetContext() Context
	AttachContext(Context)
	DetachContext()
}

type InboundHandler[Rin any, Rout any] interface {
	HandlerBase[InboundHandlerContext[Rout]]

	Read(ctx InboundHandlerContext[Rout], msg Rin)
	ReadEOF(ctx InboundHandlerContext[Rout])
	ReadError(ctx InboundHandlerContext[Rout], err error)

	TransportActive(ctx InboundHandlerContext[Rout])
	TransportInactive(ctx InboundHandlerContext[Rout])
}

type OutboundHandler[Win any, Wout any] interface {
	HandlerBase[OutboundHandlerContext[Wout]]

	Write(ctx OutboundHandlerContext[Wout], msg Win)
	WriteError(ctx OutboundHandlerContext[Wout], err error)
	Close(ctx OutboundHandlerContext[Wout])
}

type Handler[Rin any, Rout any, Win any, Wout any] interface {
	HandlerBase[HandlerContext[Rout, Wout]]

	Read(ctx HandlerContext[Rout, Wout], msg Rin)
	ReadEOF(ctx HandlerContext[Rout, Wout])
	ReadError(ctx HandlerContext[Rout, Wout], err error)

	TransportActive(ctx HandlerContext[Rout, Wout])
	TransportInactive(ctx HandlerContext[Rout, Wout])

	Write(ctx HandlerContext[Rout, Wout], msg Win)
	WriteError(ctx HandlerContext[Rout, Wout], err error)
	Close(ctx HandlerContext[Rout, Wout])
}

type HandlerAdapter[R any, W any] interface {
	Handler[R, R, W, W]
}
