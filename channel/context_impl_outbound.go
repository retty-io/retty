package channel

import "log"

type OutboundContextImpl[
	In any,
	Out any,
	Context OutboundHandlerContext[Out],
	H OutboundHandler[In, Out],
	Link OutboundLink[Out],
] struct {
	context Context
	handler H
	next    any //TODO: Link

	pipeline *PipelineBase[H, Context]
	attached bool
}

func (c *OutboundContextImpl[In, Out, Context, H, Link]) GetHandler() H {
	return c.handler
}

func (c *OutboundContextImpl[In, Out, Context, H, Link]) Initialize(pipeline *PipelineBase[H, Context], handler H) {
	c.pipeline = pipeline
	c.handler = handler
}

///////////////////////////////////////////////////////////////////////////////
// impl PipelineContext
///////////////////////////////////////////////////////////////////////////////

func (c *OutboundContextImpl[In, Out, Context, H, Link]) AttachPipeline() {
	if c.attached {
		c.handler.AttachContext(c.context)
		c.attached = true
	}
}

func (c *OutboundContextImpl[In, Out, Context, H, Link]) DetachPipeline() {
	c.attached = false
	c.handler.DetachContext()
}

func (c *OutboundContextImpl[In, Out, Context, H, Link]) SetNextIn(_ PipelineContext[H, Context]) {
	//Do nothing for OutboundContextImpl
}

func (c *OutboundContextImpl[In, Out, Context, H, Link]) SetNextOut(ctx PipelineContext[H, Context]) {
	if ctx == nil {
		c.next = nil
	} else {
		if next, ok := ctx.(Link); ok {
			c.next = next
		} else {
			panic("outbound type mismatch")
		}
	}
}

func (c *OutboundContextImpl[In, Out, Context, H, Link]) GetDirection() HandlerDir {
	return HandlerDirOut
}

///////////////////////////////////////////////////////////////////////////////
// impl HandlerContextBase
///////////////////////////////////////////////////////////////////////////////

func (c *OutboundContextImpl[In, Out, Context, H, Link]) GetPipeline() *PipelineBase[H, Context] {
	return c.pipeline
}

//TODO:
//func (c *OutboundContextImpl[In, Out, Context, Handler, Link]) GetTransport() Transport {
//}

///////////////////////////////////////////////////////////////////////////////
// impl OutboundHandlerContext
///////////////////////////////////////////////////////////////////////////////

func (c *OutboundContextImpl[In, Out, Context, H, Link]) FireWrite(msg Out) {
	if next, ok := c.next.(Link); ok {
		next.Write(msg)
	} else {
		log.Println("write reached end of pipeline")
	}
}

func (c *OutboundContextImpl[In, Out, Context, H, Link]) FireClose() {
	if next, ok := c.next.(Link); ok {
		next.Close()
	} else {
		log.Println("close reached end of pipeline")
	}
}

func (c *OutboundContextImpl[In, Out, Context, H, Link]) FireWriteError(err error) {
	if next, ok := c.next.(Link); ok {
		next.WriteError(err)
	} else {
		log.Println("writeError reached end of pipeline")
	}
}

///////////////////////////////////////////////////////////////////////////////
// impl OutboundLink
///////////////////////////////////////////////////////////////////////////////

func (c *OutboundContextImpl[In, Out, Context, H, Link]) Write(msg In) {
	c.handler.Write(c, msg)
}

func (c *OutboundContextImpl[In, Out, Context, H, Link]) Close() {
	c.handler.Close(c)
}

func (c *OutboundContextImpl[In, Out, Context, H, Link]) WriteError(err error) {
	c.handler.WriteError(c, err)
}
