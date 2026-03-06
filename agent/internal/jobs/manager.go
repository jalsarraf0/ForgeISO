package jobs

import (
	"fmt"
	"sync"
	"time"
)

type Type string

const (
	TypeBuild Type = "build"
	TypeTest  Type = "test"
)

type Job struct {
	ID         string
	Type       Type
	CreatedAt  time.Time
	Done       bool
	Logs       []LogEntry
	Artifacts  []Artifact
	subscribers []chan LogEntry
}

type LogEntry struct {
	JobID     string
	Timestamp string
	Level     string
	Message   string
	Done      bool
}

type Artifact struct {
	Path      string
	Content   []byte
	MediaType string
}

type Manager struct {
	mu   sync.RWMutex
	jobs map[string]*Job
}

func NewManager() *Manager {
	return &Manager{jobs: make(map[string]*Job)}
}

func (m *Manager) Create(t Type) *Job {
	m.mu.Lock()
	defer m.mu.Unlock()

	id := fmt.Sprintf("job-%d", time.Now().UnixNano())
	job := &Job{
		ID:        id,
		Type:      t,
		CreatedAt: time.Now().UTC(),
		Logs:      make([]LogEntry, 0, 32),
		Artifacts: make([]Artifact, 0, 8),
	}
	m.jobs[id] = job
	return job
}

func (m *Manager) Get(id string) (*Job, bool) {
	m.mu.RLock()
	defer m.mu.RUnlock()
	j, ok := m.jobs[id]
	return j, ok
}

func (m *Manager) AddLog(id string, level string, message string, done bool) {
	m.mu.Lock()
	defer m.mu.Unlock()

	job, ok := m.jobs[id]
	if !ok {
		return
	}

	entry := LogEntry{
		JobID:     id,
		Timestamp: time.Now().UTC().Format(time.RFC3339),
		Level:     level,
		Message:   message,
		Done:      done,
	}

	job.Logs = append(job.Logs, entry)
	for _, ch := range job.subscribers {
		select {
		case ch <- entry:
		default:
		}
	}

	if done {
		job.Done = true
		for _, ch := range job.subscribers {
			close(ch)
		}
		job.subscribers = nil
	}
}

func (m *Manager) AddArtifact(id string, artifact Artifact) {
	m.mu.Lock()
	defer m.mu.Unlock()
	if job, ok := m.jobs[id]; ok {
		job.Artifacts = append(job.Artifacts, artifact)
	}
}

func (m *Manager) Subscribe(id string) (<-chan LogEntry, bool) {
	m.mu.Lock()
	defer m.mu.Unlock()

	job, ok := m.jobs[id]
	if !ok {
		return nil, false
	}

	ch := make(chan LogEntry, 128)
	for _, entry := range job.Logs {
		ch <- entry
	}
	if job.Done {
		close(ch)
		return ch, true
	}

	job.subscribers = append(job.subscribers, ch)
	return ch, true
}
