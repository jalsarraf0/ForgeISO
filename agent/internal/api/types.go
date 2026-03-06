package api

type BuildInputArtifact struct {
	Path    string `json:"path"`
	Content []byte `json:"content"`
}

type SubmitBuildJobRequest struct {
	RequestID string               `json:"request_id"`
	ConfigYAML []byte              `json:"config_yaml"`
	Artifacts []BuildInputArtifact `json:"artifacts"`
}

type SubmitBuildJobResponse struct {
	JobID  string `json:"job_id"`
	Status string `json:"status"`
}

type SubmitVmTestJobRequest struct {
	RequestID string `json:"request_id"`
	ISOBytes  []byte `json:"iso_bytes"`
	ISOPath   string `json:"iso_path"`
	BIOS      bool   `json:"bios"`
	UEFI      bool   `json:"uefi"`
	MemoryMB  uint32 `json:"memory_mb"`
	VCPUs     uint32 `json:"vcpus"`
}

type SubmitVmTestJobResponse struct {
	JobID  string `json:"job_id"`
	Status string `json:"status"`
}

type StreamLogsRequest struct {
	JobID string `json:"job_id"`
}

type LogEntry struct {
	JobID     string `json:"job_id"`
	Timestamp string `json:"timestamp"`
	Level     string `json:"level"`
	Message   string `json:"message"`
	Done      bool   `json:"done"`
}

type FetchArtifactsRequest struct {
	JobID string `json:"job_id"`
}

type Artifact struct {
	Path      string `json:"path"`
	Content   []byte `json:"content"`
	MediaType string `json:"media_type"`
}

type FetchArtifactsResponse struct {
	JobID     string     `json:"job_id"`
	Artifacts []Artifact `json:"artifacts"`
}
