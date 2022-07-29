package channel

import "time"

type PipelineManger[H any, Context any] interface {
	DeletePipeline(pipeline *PipelineBase[H, Context])
	RefreshTimeout()
	AdjustTimeout(duration time.Duration)
}

type PipelineBase[H any, Context any] struct {
	inCtxs  []PipelineContext[H, Context]
	outCtxs []PipelineContext[H, Context]

	manager PipelineManger[H, Context]
	//transport Transport
	//trnasportInfo TransportInfo
}

func (p *PipelineBase[H, Context]) SetPipelineManager(manager PipelineManger[H, Context]) {
	p.manager = manager
}

func (p *PipelineBase[H, Context]) GetPipelineManager() PipelineManger[H, Context] {
	return p.manager
}

func (p *PipelineBase[H, Context]) DeletePipeline() {
	if p.manager != nil {
		p.manager.DeletePipeline(p)
	}
}

/*TODO:
func (p *PipelineBase[H, Context]) SetTransport(transport Transport) {
	p.transport = transport
}

func (p *PipelineBase[H, Context]) GetTransport() Transport {
	return p.transport
}

  void setWriteFlags(folly::WriteFlags flags);
  folly::WriteFlags getWriteFlags();

  void setReadBufferSettings(uint64_t minAvailable, uint64_t allocationSize);
  std::pair<uint64_t, uint64_t> getReadBufferSettings();

  void setTransportInfo(std::shared_ptr<TransportInfo> tInfo);
  std::shared_ptr<TransportInfo> getTransportInfo();
*/

func (p *PipelineBase[H, Context]) AddHelper(ctx PipelineContext[H, Context], front bool) {
	dir := ctx.GetDirection()
	if dir == HandlerDirBoth || dir == HandlerDirIn {
		if front {
			p.inCtxs = append([]PipelineContext[H, Context]{ctx}, p.inCtxs...)
		} else {
			p.inCtxs = append(p.inCtxs, ctx)
		}
	}
	if dir == HandlerDirBoth || dir == HandlerDirOut {
		if front {
			p.outCtxs = append([]PipelineContext[H, Context]{ctx}, p.outCtxs...)
		} else {
			p.outCtxs = append(p.outCtxs, ctx)
		}
	}
}

func (p *PipelineBase[H, Context]) AddBack(handler H) {

}

func (p *PipelineBase[H, Context]) AddFront(handler H) {

}

func (p *PipelineBase[H, Context]) Remove(handler H) {

}

func (p *PipelineBase[H, Context]) RemoveBack() {

}

func (p *PipelineBase[H, Context]) RemoveFront() {

}

/*func (p *PipelineBase[H, Context]) NumHandlers() int {

}*/
