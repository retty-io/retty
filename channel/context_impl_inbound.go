package channel

import "log"

type InboundContextImpl[
	In any,
	Out any,
	Context InboundHandlerContext[Out],
	H InboundHandler[In, Out],
	Link InboundLink[Out],
] struct {
	context Context
	handler H
	next    any //TODO: Link

	pipeline *PipelineBase[H, Context]
	attached bool
}

func (c *InboundContextImpl[In, Out, Context, H, Link]) GetHandler() H {
	return c.handler
}

func (c *InboundContextImpl[In, Out, Context, H, Link]) Initialize(pipeline *PipelineBase[H, Context], handler H) {
	c.pipeline = pipeline
	c.handler = handler
}

///////////////////////////////////////////////////////////////////////////////
// impl PipelineContext
///////////////////////////////////////////////////////////////////////////////

func (c *InboundContextImpl[In, Out, Context, H, Link]) AttachPipeline() {
	if c.attached {
		c.handler.AttachContext(c.context)
		c.attached = true
	}
}

func (c *InboundContextImpl[In, Out, Context, H, Link]) DetachPipeline() {
	c.attached = false
	c.handler.DetachContext()
}

func (c *InboundContextImpl[In, Out, Context, H, Link]) SetNextIn(ctx PipelineContext[H, Context]) {
	if ctx == nil {
		c.next = nil
	} else {
		if next, ok := ctx.(Link); ok {
			c.next = next
		} else {
			panic("inbound type mismatch")
		}
	}
}

func (c *InboundContextImpl[In, Out, Context, H, Link]) SetNextOut(_ PipelineContext[H, Context]) {
	//Do nothing for InboundContextImpl
}

func (c *InboundContextImpl[In, Out, Context, H, Link]) GetDirection() HandlerDir {
	return HandlerDirIn
}

///////////////////////////////////////////////////////////////////////////////
// impl HandlerContextBase
///////////////////////////////////////////////////////////////////////////////

func (c *InboundContextImpl[In, Out, Context, H, Link]) GetPipeline() *PipelineBase[H, Context] {
	return c.pipeline
}

//TODO:
//func (c *InboundContextImpl[In, Out, Context, Handler, Link]) GetTransport() Transport {
//}

///////////////////////////////////////////////////////////////////////////////
// impl InboundHandlerContext
///////////////////////////////////////////////////////////////////////////////

func (c *InboundContextImpl[In, Out, Context, H, Link]) FireRead(msg Out) {
	if next, ok := c.next.(Link); ok {
		next.Read(msg)
	} else {
		log.Println("read reached end of pipeline")
	}
}

func (c *InboundContextImpl[In, Out, Context, H, Link]) FireReadEOF() {
	if next, ok := c.next.(Link); ok {
		next.ReadEOF()
	} else {
		log.Println("readEOF reached end of pipeline")
	}
}

func (c *InboundContextImpl[In, Out, Context, H, Link]) FireReadError(err error) {
	if next, ok := c.next.(Link); ok {
		next.ReadError(err)
	} else {
		log.Println("readError reached end of pipeline")
	}
}

func (c *InboundContextImpl[In, Out, Context, H, Link]) FireTransportActive() {
	if next, ok := c.next.(Link); ok {
		next.TransportActive()
	}
}

func (c *InboundContextImpl[In, Out, Context, H, Link]) FireTransportInactive() {
	if next, ok := c.next.(Link); ok {
		next.TransportInactive()
	}
}

///////////////////////////////////////////////////////////////////////////////
// impl InboundLink
///////////////////////////////////////////////////////////////////////////////

func (c *InboundContextImpl[In, Out, Context, H, Link]) Read(msg In) {
	c.handler.Read(c, msg)
}

func (c *InboundContextImpl[In, Out, Context, H, Link]) ReadEOF() {
	c.handler.ReadEOF(c)
}

func (c *InboundContextImpl[In, Out, Context, H, Link]) ReadError(err error) {
	c.handler.ReadError(c, err)
}

func (c *InboundContextImpl[In, Out, Context, H, Link]) TransportActive() {
	c.handler.TransportActive(c)
}

func (c *InboundContextImpl[In, Out, Context, H, Link]) TransportInactive() {
	c.handler.TransportInactive(c)
}
