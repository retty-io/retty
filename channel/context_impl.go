package channel

import "log"

type ContextImpl[
	Rin any,
	Rout any,
	Win any,
	Wout any,
	Context HandlerContext[Rout, Wout],
	H Handler[Rin, Rout, Win, Wout],
	ILink InboundLink[Rout],
	OLink OutboundLink[Wout],
] struct {
	context Context
	handler H
	nextIn  any //TODO: ILink ?
	nextOut any //TODO: OLink?

	pipeline *PipelineBase[H, Context]
	attached bool
}

func (c *ContextImpl[Rin, Rout, Win, Wout, Context, H, ILink, OLink]) GetHandler() H {
	return c.handler
}

func (c *ContextImpl[Rin, Rout, Win, Wout, Context, H, ILink, OLink]) Initialize(pipeline *PipelineBase[H, Context], handler H) {
	c.pipeline = pipeline
	c.handler = handler
}

///////////////////////////////////////////////////////////////////////////////
// impl PipelineContext
///////////////////////////////////////////////////////////////////////////////

func (c *ContextImpl[Rin, Rout, Win, Wout, Context, H, ILink, OLink]) AttachPipeline() {
	if c.attached {
		c.handler.AttachContext(c.context)
		c.attached = true
	}
}

func (c *ContextImpl[Rin, Rout, Win, Wout, Context, H, ILink, OLink]) DetachPipeline() {
	c.attached = false
	c.handler.DetachContext()
}

func (c *ContextImpl[Rin, Rout, Win, Wout, Context, H, ILink, OLink]) SetNextIn(ctx PipelineContext[H, Context]) {
	if ctx == nil {
		c.nextIn = nil
	} else {
		if next, ok := ctx.(ILink); ok {
			c.nextIn = next
		} else {
			panic("inbound type mismatch")
		}
	}
}

func (c *ContextImpl[Rin, Rout, Win, Wout, Context, H, ILink, OLink]) SetNextOut(ctx PipelineContext[H, Context]) {
	if ctx == nil {
		c.nextOut = nil
	} else {
		if next, ok := ctx.(OLink); ok {
			c.nextOut = next
		} else {
			panic("outbound type mismatch")
		}
	}
}

func (c *ContextImpl[Rin, Rout, Win, Wout, Context, H, ILink, OLink]) GetDirection() HandlerDir {
	return HandlerDirIn
}

///////////////////////////////////////////////////////////////////////////////
// impl HandlerContextBase
///////////////////////////////////////////////////////////////////////////////

func (c *ContextImpl[Rin, Rout, Win, Wout, Context, H, ILink, OLink]) GetPipeline() *PipelineBase[H, Context] {
	return c.pipeline
}

//TODO:
//func (c *ContextImpl[In, Out, Context, Handler, Link]) GetTransport() Transport {
//}

///////////////////////////////////////////////////////////////////////////////
// impl InboundHandlerContext
///////////////////////////////////////////////////////////////////////////////

func (c *ContextImpl[Rin, Rout, Win, Wout, Context, H, ILink, OLink]) FireRead(msg Rout) {
	if next, ok := c.nextIn.(ILink); ok {
		next.Read(msg)
	} else {
		log.Println("read reached end of pipeline")
	}
}

func (c *ContextImpl[Rin, Rout, Win, Wout, Context, H, ILink, OLink]) FireReadEOF() {
	if next, ok := c.nextIn.(ILink); ok {
		next.ReadEOF()
	} else {
		log.Println("readEOF reached end of pipeline")
	}
}

func (c *ContextImpl[Rin, Rout, Win, Wout, Context, H, ILink, OLink]) FireReadError(err error) {
	if next, ok := c.nextIn.(ILink); ok {
		next.ReadError(err)
	} else {
		log.Println("readError reached end of pipeline")
	}
}

func (c *ContextImpl[Rin, Rout, Win, Wout, Context, H, ILink, OLink]) FireTransportActive() {
	if next, ok := c.nextIn.(ILink); ok {
		next.TransportActive()
	}
}

func (c *ContextImpl[Rin, Rout, Win, Wout, Context, H, ILink, OLink]) FireTransportInactive() {
	if next, ok := c.nextIn.(ILink); ok {
		next.TransportInactive()
	}
}

///////////////////////////////////////////////////////////////////////////////
// impl InboundLink
///////////////////////////////////////////////////////////////////////////////

func (c *ContextImpl[Rin, Rout, Win, Wout, Context, H, ILink, OLink]) Read(msg Rin) {
	c.handler.Read(c, msg)
}

func (c *ContextImpl[Rin, Rout, Win, Wout, Context, H, ILink, OLink]) ReadEOF() {
	c.handler.ReadEOF(c)
}

func (c *ContextImpl[Rin, Rout, Win, Wout, Context, H, ILink, OLink]) ReadError(err error) {
	c.handler.ReadError(c, err)
}

func (c *ContextImpl[Rin, Rout, Win, Wout, Context, H, ILink, OLink]) TransportActive() {
	c.handler.TransportActive(c)
}

func (c *ContextImpl[Rin, Rout, Win, Wout, Context, H, ILink, OLink]) TransportInactive() {
	c.handler.TransportInactive(c)
}

///////////////////////////////////////////////////////////////////////////////
// impl OutboundHandlerContext
///////////////////////////////////////////////////////////////////////////////

func (c *ContextImpl[Rin, Rout, Win, Wout, Context, H, ILink, OLink]) FireWrite(msg Wout) {
	if next, ok := c.nextOut.(OLink); ok {
		next.Write(msg)
	} else {
		log.Println("write reached end of pipeline")
	}
}

func (c *ContextImpl[Rin, Rout, Win, Wout, Context, H, ILink, OLink]) FireClose() {
	if next, ok := c.nextOut.(OLink); ok {
		next.Close()
	} else {
		log.Println("close reached end of pipeline")
	}
}

func (c *ContextImpl[Rin, Rout, Win, Wout, Context, H, ILink, OLink]) FireWriteError(err error) {
	if next, ok := c.nextOut.(OLink); ok {
		next.WriteError(err)
	} else {
		log.Println("writeError reached end of pipeline")
	}
}

///////////////////////////////////////////////////////////////////////////////
// impl OutboundLink
///////////////////////////////////////////////////////////////////////////////

func (c *ContextImpl[Rin, Rout, Win, Wout, Context, H, ILink, OLink]) Write(msg Win) {
	c.handler.Write(c, msg)
}

func (c *ContextImpl[Rin, Rout, Win, Wout, Context, H, ILink, OLink]) Close() {
	c.handler.Close(c)
}

func (c *ContextImpl[Rin, Rout, Win, Wout, Context, H, ILink, OLink]) WriteError(err error) {
	c.handler.WriteError(c, err)
}
