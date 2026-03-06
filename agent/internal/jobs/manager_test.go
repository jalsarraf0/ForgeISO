package jobs

import "testing"

func TestJobLifecycle(t *testing.T) {
	m := NewManager()
	job := m.Create(TypeBuild)
	if job.ID == "" {
		t.Fatal("expected job id")
	}

	ch, ok := m.Subscribe(job.ID)
	if !ok {
		t.Fatal("expected subscription")
	}

	m.AddLog(job.ID, "INFO", "started", false)
	m.AddLog(job.ID, "INFO", "done", true)

	count := 0
	for range ch {
		count++
	}

	if count != 2 {
		t.Fatalf("expected 2 log entries, got %d", count)
	}
}
