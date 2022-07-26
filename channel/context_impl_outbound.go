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

	pipeline *PipelineBase
	attached bool
}

func (ic *OutboundContextImpl[In, Out, Context, H, Link]) GetHandler() H {
	return ic.handler
}

func (ic *OutboundContextImpl[In, Out, Context, H, Link]) Initialize(pipeline *PipelineBase, handler H) {
	ic.pipeline = pipeline
	ic.handler = handler
}

///////////////////////////////////////////////////////////////////////////////
// impl PipelineContext
///////////////////////////////////////////////////////////////////////////////

func (ic *OutboundContextImpl[In, Out, Context, H, Link]) AttachPipeline() {
	if ic.attached {
		ic.handler.AttachContext(ic.context)
		ic.attached = true
	}
}

func (ic *OutboundContextImpl[In, Out, Context, H, Link]) DetachPipeline() {
	ic.attached = false
	ic.handler.DetachContext()
}

func (ic *OutboundContextImpl[In, Out, Context, H, Link]) SetNextIn(_ PipelineContext[H, Context]) {
	//Do nothing for OutboundContextImpl
}

func (ic *OutboundContextImpl[In, Out, Context, H, Link]) SetNextOut(ctx PipelineContext[H, Context]) {
	if ctx == nil {
		ic.next = nil
	} else {
		if next, ok := ctx.(Link); ok {
			ic.next = next
		} else {
			panic("outbound type mismatch")
		}
	}
}

func (ic *OutboundContextImpl[In, Out, Context, H, Link]) GetDirection() HandlerDir {
	return HandlerDirOut
}

///////////////////////////////////////////////////////////////////////////////
// impl HandlerContextBase
///////////////////////////////////////////////////////////////////////////////

func (ic *OutboundContextImpl[In, Out, Context, H, Link]) GetPipeline() *PipelineBase {
	return ic.pipeline
}

//TODO:
//func (ic *OutboundContextImpl[In, Out, Context, Handler, Link]) GetTransport() Transport {
//}

///////////////////////////////////////////////////////////////////////////////
// impl OutboundHandlerContext
///////////////////////////////////////////////////////////////////////////////

func (ic *OutboundContextImpl[In, Out, Context, H, Link]) FireWrite(msg Out) {
	if next, ok := ic.next.(Link); ok {
		next.Write(msg)
	} else {
		log.Println("write reached end of pipeline")
	}
}

func (ic *OutboundContextImpl[In, Out, Context, H, Link]) FireClose() {
	if next, ok := ic.next.(Link); ok {
		next.Close()
	} else {
		log.Println("close reached end of pipeline")
	}
}

func (ic *OutboundContextImpl[In, Out, Context, H, Link]) FireWriteError(err error) {
	if next, ok := ic.next.(Link); ok {
		next.WriteError(err)
	} else {
		log.Println("writeError reached end of pipeline")
	}
}

///////////////////////////////////////////////////////////////////////////////
// impl OutboundLink
///////////////////////////////////////////////////////////////////////////////

func (ic *OutboundContextImpl[In, Out, Context, H, Link]) Write(msg In) {
	ic.handler.Write(ic, msg)
}

func (ic *OutboundContextImpl[In, Out, Context, H, Link]) Close() {
	ic.handler.Close(ic)
}

func (ic *OutboundContextImpl[In, Out, Context, H, Link]) WriteError(err error) {
	ic.handler.WriteError(ic, err)
}
