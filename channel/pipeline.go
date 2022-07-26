package channel

import "time"

type PipelineManger interface {
	DeletePipeline(pipeline *PipelineBase)
	RefreshTimeout()
	AdjustTimeout(duration time.Duration)
}

type PipelineBase struct {
}
