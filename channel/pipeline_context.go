package channel

import "log"

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

type InboundContextImpl[Rin any, Rout any, Context InboundHandlerContext[Rout], H InboundHandler[Rin, Rout], Link InboundLink[Rout]] struct {
	context Context
	handler H
	next    Link

	pipeline *PipelineBase
	attached bool
}

func (ic *InboundContextImpl[Rin, Rout, Context, H, Link]) GetHandler() H {
	return ic.handler
}

func (ic *InboundContextImpl[Rin, Rout, Context, H, Link]) Initialize(pipeline *PipelineBase, handler H) {
	ic.pipeline = pipeline
	ic.handler = handler
}

///////////////////////////////////////////////////////////////////////////////
// impl PipelineContext
///////////////////////////////////////////////////////////////////////////////

func (ic *InboundContextImpl[Rin, Rout, Context, H, Link]) AttachPipeline() {
	if ic.attached {
		ic.handler.AttachContext(ic.context)
		ic.attached = true
	}
}

func (ic *InboundContextImpl[Rin, Rout, Context, H, Link]) DetachPipeline() {
	ic.attached = false
	ic.handler.DetachContext()
}

func (ic *InboundContextImpl[Rin, Rout, Context, H, Link]) SetNextIn(ctx PipelineContext[H, Context]) {
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

func (ic *InboundContextImpl[Rin, Rout, Context, H, Link]) SetNextOut(_ PipelineContext[H, Context]) {
	//Do nothing for InboundContextImpl
}

func (ic *InboundContextImpl[Rin, Rout, Context, H, Link]) GetDirection() HandlerDir {
	return HandlerDirIn
}

///////////////////////////////////////////////////////////////////////////////
// impl PipelineGetter
///////////////////////////////////////////////////////////////////////////////

func (ic *InboundContextImpl[Rin, Rout, Context, H, Link]) GetPipeline() *PipelineBase {
	return ic.pipeline
}

//TODO:
//func (ic *InboundContextImpl[Rin, Rout, Context, Handler, Link]) GetTransport() Transport {
//}

///////////////////////////////////////////////////////////////////////////////
// impl InboundHandlerContext
///////////////////////////////////////////////////////////////////////////////

func (ic *InboundContextImpl[Rin, Rout, Context, H, Link]) FireRead(msg Rout) {
	if ic.next != nil {
		ic.next.Read(msg)
	} else {
		log.Println("read reached end of pipeline")
	}
}

func (ic *InboundContextImpl[Rin, Rout, Context, H, Link]) FireReadEOF() {
	if ic.next != nil {
		ic.next.ReadEOF()
	} else {
		log.Println("readEOF reached end of pipeline")
	}
}

func (ic *InboundContextImpl[Rin, Rout, Context, H, Link]) FireReadError(err error) {
	if ic.next != nil {
		ic.next.ReadError(err)
	} else {
		log.Println("readError reached end of pipeline")
	}
}

func (ic *InboundContextImpl[Rin, Rout, Context, H, Link]) FireTransportActive() {
	if ic.next != nil {
		ic.next.TransportActive()
	}
}

func (ic *InboundContextImpl[Rin, Rout, Context, H, Link]) FireTransportInactive() {
	if ic.next != nil {
		ic.next.TransportInactive()
	}
}

///////////////////////////////////////////////////////////////////////////////
// impl InboundLink
///////////////////////////////////////////////////////////////////////////////

func (ic *InboundContextImpl[Rin, Rout, Context, H, Link]) Read(msg Rin) {
	ic.handler.Read(ic, msg)
}

func (ic *InboundContextImpl[Rin, Rout, Context, H, Link]) ReadEOF() {
	ic.handler.ReadEOF(ic)
}

func (ic *InboundContextImpl[Rin, Rout, Context, H, Link]) ReadError(err error) {
	ic.handler.ReadError(ic, err)
}

func (ic *InboundContextImpl[Rin, Rout, Context, H, Link]) TransportActive() {
	ic.handler.TransportActive(ic)
}

func (ic *InboundContextImpl[Rin, Rout, Context, H, Link]) TransportInactive() {
	ic.handler.TransportInactive(ic)
}
