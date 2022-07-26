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

	pipeline *PipelineBase
	attached bool
}

func (ic *ContextImpl[Rin, Rout, Win, Wout, Context, H, ILink, OLink]) GetHandler() H {
	return ic.handler
}

func (ic *ContextImpl[Rin, Rout, Win, Wout, Context, H, ILink, OLink]) Initialize(pipeline *PipelineBase, handler H) {
	ic.pipeline = pipeline
	ic.handler = handler
}

///////////////////////////////////////////////////////////////////////////////
// impl PipelineContext
///////////////////////////////////////////////////////////////////////////////

func (ic *ContextImpl[Rin, Rout, Win, Wout, Context, H, ILink, OLink]) AttachPipeline() {
	if ic.attached {
		ic.handler.AttachContext(ic.context)
		ic.attached = true
	}
}

func (ic *ContextImpl[Rin, Rout, Win, Wout, Context, H, ILink, OLink]) DetachPipeline() {
	ic.attached = false
	ic.handler.DetachContext()
}

func (ic *ContextImpl[Rin, Rout, Win, Wout, Context, H, ILink, OLink]) SetNextIn(ctx PipelineContext[H, Context]) {
	if ctx == nil {
		ic.nextIn = nil
	} else {
		if next, ok := ctx.(ILink); ok {
			ic.nextIn = next
		} else {
			panic("inbound type mismatch")
		}
	}
}

func (ic *ContextImpl[Rin, Rout, Win, Wout, Context, H, ILink, OLink]) SetNextOut(ctx PipelineContext[H, Context]) {
	if ctx == nil {
		ic.nextOut = nil
	} else {
		if next, ok := ctx.(OLink); ok {
			ic.nextOut = next
		} else {
			panic("outbound type mismatch")
		}
	}
}

func (ic *ContextImpl[Rin, Rout, Win, Wout, Context, H, ILink, OLink]) GetDirection() HandlerDir {
	return HandlerDirIn
}

///////////////////////////////////////////////////////////////////////////////
// impl HandlerContextBase
///////////////////////////////////////////////////////////////////////////////

func (ic *ContextImpl[Rin, Rout, Win, Wout, Context, H, ILink, OLink]) GetPipeline() *PipelineBase {
	return ic.pipeline
}

//TODO:
//func (ic *ContextImpl[In, Out, Context, Handler, Link]) GetTransport() Transport {
//}

///////////////////////////////////////////////////////////////////////////////
// impl InboundHandlerContext
///////////////////////////////////////////////////////////////////////////////

func (ic *ContextImpl[Rin, Rout, Win, Wout, Context, H, ILink, OLink]) FireRead(msg Rout) {
	if ic.nextIn != nil {
		ic.nextIn.(ILink).Read(msg)
	} else {
		log.Println("read reached end of pipeline")
	}
}

func (ic *ContextImpl[Rin, Rout, Win, Wout, Context, H, ILink, OLink]) FireReadEOF() {
	if ic.nextIn != nil {
		ic.nextIn.(ILink).ReadEOF()
	} else {
		log.Println("readEOF reached end of pipeline")
	}
}

func (ic *ContextImpl[Rin, Rout, Win, Wout, Context, H, ILink, OLink]) FireReadError(err error) {
	if ic.nextIn != nil {
		ic.nextIn.(ILink).ReadError(err)
	} else {
		log.Println("readError reached end of pipeline")
	}
}

func (ic *ContextImpl[Rin, Rout, Win, Wout, Context, H, ILink, OLink]) FireTransportActive() {
	if ic.nextIn != nil {
		ic.nextIn.(ILink).TransportActive()
	}
}

func (ic *ContextImpl[Rin, Rout, Win, Wout, Context, H, ILink, OLink]) FireTransportInactive() {
	if ic.nextIn != nil {
		ic.nextIn.(ILink).TransportInactive()
	}
}

///////////////////////////////////////////////////////////////////////////////
// impl InboundLink
///////////////////////////////////////////////////////////////////////////////

func (ic *ContextImpl[Rin, Rout, Win, Wout, Context, H, ILink, OLink]) Read(msg Rin) {
	ic.handler.Read(ic, msg)
}

func (ic *ContextImpl[Rin, Rout, Win, Wout, Context, H, ILink, OLink]) ReadEOF() {
	ic.handler.ReadEOF(ic)
}

func (ic *ContextImpl[Rin, Rout, Win, Wout, Context, H, ILink, OLink]) ReadError(err error) {
	ic.handler.ReadError(ic, err)
}

func (ic *ContextImpl[Rin, Rout, Win, Wout, Context, H, ILink, OLink]) TransportActive() {
	ic.handler.TransportActive(ic)
}

func (ic *ContextImpl[Rin, Rout, Win, Wout, Context, H, ILink, OLink]) TransportInactive() {
	ic.handler.TransportInactive(ic)
}

///////////////////////////////////////////////////////////////////////////////
// impl OutboundHandlerContext
///////////////////////////////////////////////////////////////////////////////

func (ic *ContextImpl[Rin, Rout, Win, Wout, Context, H, ILink, OLink]) FireWrite(msg Wout) {
	if ic.nextOut != nil {
		ic.nextOut.(OLink).Write(msg)
	} else {
		log.Println("write reached end of pipeline")
	}
}

func (ic *ContextImpl[Rin, Rout, Win, Wout, Context, H, ILink, OLink]) FireClose() {
	if ic.nextOut != nil {
		ic.nextOut.(OLink).Close()
	} else {
		log.Println("close reached end of pipeline")
	}
}

func (ic *ContextImpl[Rin, Rout, Win, Wout, Context, H, ILink, OLink]) FireWriteError(err error) {
	if ic.nextOut != nil {
		ic.nextOut.(OLink).WriteError(err)
	} else {
		log.Println("writeError reached end of pipeline")
	}
}

///////////////////////////////////////////////////////////////////////////////
// impl OutboundLink
///////////////////////////////////////////////////////////////////////////////

func (ic *ContextImpl[Rin, Rout, Win, Wout, Context, H, ILink, OLink]) Write(msg Win) {
	ic.handler.Write(ic, msg)
}

func (ic *ContextImpl[Rin, Rout, Win, Wout, Context, H, ILink, OLink]) Close() {
	ic.handler.Close(ic)
}

func (ic *ContextImpl[Rin, Rout, Win, Wout, Context, H, ILink, OLink]) WriteError(err error) {
	ic.handler.WriteError(ic, err)
}
