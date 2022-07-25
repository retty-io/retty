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
	next    Link

	pipeline *PipelineBase
	attached bool
}

func (ic *InboundContextImpl[In, Out, Context, H, Link]) GetHandler() H {
	return ic.handler
}

func (ic *InboundContextImpl[In, Out, Context, H, Link]) Initialize(pipeline *PipelineBase, handler H) {
	ic.pipeline = pipeline
	ic.handler = handler
}

///////////////////////////////////////////////////////////////////////////////
// impl PipelineContext
///////////////////////////////////////////////////////////////////////////////

func (ic *InboundContextImpl[In, Out, Context, H, Link]) AttachPipeline() {
	if ic.attached {
		ic.handler.AttachContext(ic.context)
		ic.attached = true
	}
}

func (ic *InboundContextImpl[In, Out, Context, H, Link]) DetachPipeline() {
	ic.attached = false
	ic.handler.DetachContext()
}

func (ic *InboundContextImpl[In, Out, Context, H, Link]) SetNextIn(ctx PipelineContext[H, Context]) {
	if ctx == nil {
		ic.next = nil
	} else {
		if next, ok := ctx.(Link); ok {
			ic.next = next
		} else {
			panic("inbound type mismatch")
		}
	}
}

func (ic *InboundContextImpl[In, Out, Context, H, Link]) SetNextOut(_ PipelineContext[H, Context]) {
	//Do nothing for InboundContextImpl
}

func (ic *InboundContextImpl[In, Out, Context, H, Link]) GetDirection() HandlerDir {
	return HandlerDirIn
}

///////////////////////////////////////////////////////////////////////////////
// impl HandlerContextBase
///////////////////////////////////////////////////////////////////////////////

func (ic *InboundContextImpl[In, Out, Context, H, Link]) GetPipeline() *PipelineBase {
	return ic.pipeline
}

//TODO:
//func (ic *InboundContextImpl[In, Out, Context, Handler, Link]) GetTransport() Transport {
//}

///////////////////////////////////////////////////////////////////////////////
// impl InboundHandlerContext
///////////////////////////////////////////////////////////////////////////////

func (ic *InboundContextImpl[In, Out, Context, H, Link]) FireRead(msg Out) {
	if ic.next != nil {
		ic.next.Read(msg)
	} else {
		log.Println("read reached end of pipeline")
	}
}

func (ic *InboundContextImpl[In, Out, Context, H, Link]) FireReadEOF() {
	if ic.next != nil {
		ic.next.ReadEOF()
	} else {
		log.Println("readEOF reached end of pipeline")
	}
}

func (ic *InboundContextImpl[In, Out, Context, H, Link]) FireReadError(err error) {
	if ic.next != nil {
		ic.next.ReadError(err)
	} else {
		log.Println("readError reached end of pipeline")
	}
}

func (ic *InboundContextImpl[In, Out, Context, H, Link]) FireTransportActive() {
	if ic.next != nil {
		ic.next.TransportActive()
	}
}

func (ic *InboundContextImpl[In, Out, Context, H, Link]) FireTransportInactive() {
	if ic.next != nil {
		ic.next.TransportInactive()
	}
}

///////////////////////////////////////////////////////////////////////////////
// impl InboundLink
///////////////////////////////////////////////////////////////////////////////

func (ic *InboundContextImpl[In, Out, Context, H, Link]) Read(msg In) {
	ic.handler.Read(ic, msg)
}

func (ic *InboundContextImpl[In, Out, Context, H, Link]) ReadEOF() {
	ic.handler.ReadEOF(ic)
}

func (ic *InboundContextImpl[In, Out, Context, H, Link]) ReadError(err error) {
	ic.handler.ReadError(ic, err)
}

func (ic *InboundContextImpl[In, Out, Context, H, Link]) TransportActive() {
	ic.handler.TransportActive(ic)
}

func (ic *InboundContextImpl[In, Out, Context, H, Link]) TransportInactive() {
	ic.handler.TransportInactive(ic)
}
