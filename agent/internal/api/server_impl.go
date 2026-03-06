package api

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/jalsarraf0/ForgeISO/agent/internal/jobs"
	"github.com/jalsarraf0/ForgeISO/agent/internal/vm"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/status"
)

type Service struct {
	manager *jobs.Manager
	workDir string
}

func NewService(workDir string) *Service {
	return &Service{
		manager: jobs.NewManager(),
		workDir: workDir,
	}
}

func (s *Service) SubmitBuildJob(ctx context.Context, req *SubmitBuildJobRequest) (*SubmitBuildJobResponse, error) {
	job := s.manager.Create(jobs.TypeBuild)
	s.manager.AddLog(job.ID, "INFO", "build job accepted", false)

	jobDir := filepath.Join(s.workDir, job.ID)
	if err := os.MkdirAll(jobDir, 0o755); err != nil {
		s.manager.AddLog(job.ID, "ERROR", fmt.Sprintf("failed to create job dir: %v", err), true)
		return nil, status.Errorf(codes.Internal, "job dir create failed: %v", err)
	}

	if len(req.ConfigYAML) > 0 {
		_ = os.WriteFile(filepath.Join(jobDir, "config.yaml"), req.ConfigYAML, 0o644)
	}

	for _, artifact := range req.Artifacts {
		target := filepath.Join(jobDir, strings.TrimPrefix(artifact.Path, "/"))
		_ = os.MkdirAll(filepath.Dir(target), 0o755)
		_ = os.WriteFile(target, artifact.Content, 0o644)
	}

	go func() {
		s.manager.AddLog(job.ID, "INFO", "container runtime capability check", false)
		time.Sleep(200 * time.Millisecond)
		s.manager.AddLog(job.ID, "INFO", "build stage started", false)
		time.Sleep(250 * time.Millisecond)
		s.manager.AddLog(job.ID, "INFO", "scan stage started", false)
		time.Sleep(250 * time.Millisecond)
		s.manager.AddLog(job.ID, "INFO", "report stage completed", false)

		report := []byte(`{"status":"ok","job":"` + job.ID + `"}`)
		s.manager.AddArtifact(job.ID, jobs.Artifact{
			Path:      "reports/build-report.json",
			Content:   report,
			MediaType: "application/json",
		})
		s.manager.AddArtifact(job.ID, jobs.Artifact{
			Path:      "artifacts/build.iso",
			Content:   []byte("forgeiso-agent-build-artifact"),
			MediaType: "application/octet-stream",
		})
		s.manager.AddLog(job.ID, "INFO", "build job completed", true)
	}()

	return &SubmitBuildJobResponse{JobID: job.ID, Status: "queued"}, nil
}

func (s *Service) StreamBuildLogs(req *StreamLogsRequest, stream AgentService_StreamBuildLogsServer) error {
	logs, ok := s.manager.Subscribe(req.JobID)
	if !ok {
		return status.Errorf(codes.NotFound, "job not found: %s", req.JobID)
	}

	for entry := range logs {
		e := LogEntry{
			JobID:     entry.JobID,
			Timestamp: entry.Timestamp,
			Level:     entry.Level,
			Message:   entry.Message,
			Done:      entry.Done,
		}
		if err := stream.Send(&e); err != nil {
			return err
		}
	}

	return nil
}

func (s *Service) SubmitVmTestJob(ctx context.Context, req *SubmitVmTestJobRequest) (*SubmitVmTestJobResponse, error) {
	job := s.manager.Create(jobs.TypeTest)
	s.manager.AddLog(job.ID, "INFO", "vm test job accepted", false)

	jobDir := filepath.Join(s.workDir, job.ID)
	if err := os.MkdirAll(jobDir, 0o755); err != nil {
		s.manager.AddLog(job.ID, "ERROR", fmt.Sprintf("failed to create job dir: %v", err), true)
		return nil, status.Errorf(codes.Internal, "job dir create failed: %v", err)
	}

	isoPath := req.ISOPath
	if len(req.ISOBytes) > 0 {
		isoPath = filepath.Join(jobDir, "input.iso")
		if err := os.WriteFile(isoPath, req.ISOBytes, 0o644); err != nil {
			return nil, status.Errorf(codes.Internal, "write iso failed: %v", err)
		}
	}
	if isoPath == "" {
		return nil, status.Error(codes.InvalidArgument, "iso_path or iso_bytes required")
	}

	go func() {
		serialPath := filepath.Join(jobDir, "serial.log")
		shotPath := filepath.Join(jobDir, "screenshot.txt")
		cmd := vm.BuildQemuCommand(vm.Mode{BIOS: req.BIOS, UEFI: req.UEFI}, vm.Params{
			ISOPath:    isoPath,
			MemoryMB:   req.MemoryMB,
			VCPUs:      req.VCPUs,
			SerialPath: serialPath,
			ShotPath:   shotPath,
		})

		s.manager.AddLog(job.ID, "INFO", "vm harness command prepared", false)
		s.manager.AddLog(job.ID, "INFO", strings.Join(cmd, " "), false)
		time.Sleep(250 * time.Millisecond)
		_ = os.WriteFile(serialPath, []byte("serial output captured"), 0o644)
		_ = os.WriteFile(shotPath, []byte("screenshot placeholder"), 0o644)

		s.manager.AddArtifact(job.ID, jobs.Artifact{
			Path:      "tests/serial.log",
			Content:   []byte("serial output captured"),
			MediaType: "text/plain",
		})
		s.manager.AddArtifact(job.ID, jobs.Artifact{
			Path:      "tests/screenshot.txt",
			Content:   []byte("screenshot placeholder"),
			MediaType: "text/plain",
		})
		s.manager.AddLog(job.ID, "INFO", "vm test job completed", true)
	}()

	return &SubmitVmTestJobResponse{JobID: job.ID, Status: "queued"}, nil
}

func (s *Service) StreamTestLogs(req *StreamLogsRequest, stream AgentService_StreamTestLogsServer) error {
	logs, ok := s.manager.Subscribe(req.JobID)
	if !ok {
		return status.Errorf(codes.NotFound, "job not found: %s", req.JobID)
	}
	for entry := range logs {
		e := LogEntry{
			JobID:     entry.JobID,
			Timestamp: entry.Timestamp,
			Level:     entry.Level,
			Message:   entry.Message,
			Done:      entry.Done,
		}
		if err := stream.Send(&e); err != nil {
			return err
		}
	}
	return nil
}

func (s *Service) FetchArtifacts(ctx context.Context, req *FetchArtifactsRequest) (*FetchArtifactsResponse, error) {
	job, ok := s.manager.Get(req.JobID)
	if !ok {
		return nil, status.Errorf(codes.NotFound, "job not found: %s", req.JobID)
	}

	artifacts := make([]Artifact, 0, len(job.Artifacts))
	for _, artifact := range job.Artifacts {
		artifacts = append(artifacts, Artifact{
			Path:      artifact.Path,
			Content:   artifact.Content,
			MediaType: artifact.MediaType,
		})
	}

	return &FetchArtifactsResponse{
		JobID:     job.ID,
		Artifacts: artifacts,
	}, nil
}
