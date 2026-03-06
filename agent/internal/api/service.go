package api

import (
	"context"

	"google.golang.org/grpc"
)

type AgentServiceServer interface {
	SubmitBuildJob(context.Context, *SubmitBuildJobRequest) (*SubmitBuildJobResponse, error)
	StreamBuildLogs(*StreamLogsRequest, AgentService_StreamBuildLogsServer) error
	SubmitVmTestJob(context.Context, *SubmitVmTestJobRequest) (*SubmitVmTestJobResponse, error)
	StreamTestLogs(*StreamLogsRequest, AgentService_StreamTestLogsServer) error
	FetchArtifacts(context.Context, *FetchArtifactsRequest) (*FetchArtifactsResponse, error)
}

type AgentService_StreamBuildLogsServer interface {
	Send(*LogEntry) error
	grpc.ServerStream
}

type AgentService_StreamTestLogsServer interface {
	Send(*LogEntry) error
	grpc.ServerStream
}

func RegisterAgentServiceServer(s grpc.ServiceRegistrar, srv AgentServiceServer) {
	s.RegisterService(&AgentService_ServiceDesc, srv)
}

var AgentService_ServiceDesc = grpc.ServiceDesc{
	ServiceName: "forgeiso.agent.v1.AgentService",
	HandlerType: (*AgentServiceServer)(nil),
	Methods: []grpc.MethodDesc{
		{
			MethodName: "SubmitBuildJob",
			Handler:    _AgentService_SubmitBuildJob_Handler,
		},
		{
			MethodName: "SubmitVmTestJob",
			Handler:    _AgentService_SubmitVmTestJob_Handler,
		},
		{
			MethodName: "FetchArtifacts",
			Handler:    _AgentService_FetchArtifacts_Handler,
		},
	},
	Streams: []grpc.StreamDesc{
		{
			StreamName:    "StreamBuildLogs",
			Handler:       _AgentService_StreamBuildLogs_Handler,
			ServerStreams: true,
		},
		{
			StreamName:    "StreamTestLogs",
			Handler:       _AgentService_StreamTestLogs_Handler,
			ServerStreams: true,
		},
	},
	Metadata: "proto/forgeiso/agent/v1/agent.proto",
}

func _AgentService_SubmitBuildJob_Handler(
	srv any,
	ctx context.Context,
	dec func(any) error,
	interceptor grpc.UnaryServerInterceptor,
) (any, error) {
	in := new(SubmitBuildJobRequest)
	if err := dec(in); err != nil {
		return nil, err
	}
	if interceptor == nil {
		return srv.(AgentServiceServer).SubmitBuildJob(ctx, in)
	}
	info := &grpc.UnaryServerInfo{
		Server:     srv,
		FullMethod: "/forgeiso.agent.v1.AgentService/SubmitBuildJob",
	}
	handler := func(ctx context.Context, req any) (any, error) {
		return srv.(AgentServiceServer).SubmitBuildJob(ctx, req.(*SubmitBuildJobRequest))
	}
	return interceptor(ctx, in, info, handler)
}

func _AgentService_SubmitVmTestJob_Handler(
	srv any,
	ctx context.Context,
	dec func(any) error,
	interceptor grpc.UnaryServerInterceptor,
) (any, error) {
	in := new(SubmitVmTestJobRequest)
	if err := dec(in); err != nil {
		return nil, err
	}
	if interceptor == nil {
		return srv.(AgentServiceServer).SubmitVmTestJob(ctx, in)
	}
	info := &grpc.UnaryServerInfo{
		Server:     srv,
		FullMethod: "/forgeiso.agent.v1.AgentService/SubmitVmTestJob",
	}
	handler := func(ctx context.Context, req any) (any, error) {
		return srv.(AgentServiceServer).SubmitVmTestJob(ctx, req.(*SubmitVmTestJobRequest))
	}
	return interceptor(ctx, in, info, handler)
}

func _AgentService_FetchArtifacts_Handler(
	srv any,
	ctx context.Context,
	dec func(any) error,
	interceptor grpc.UnaryServerInterceptor,
) (any, error) {
	in := new(FetchArtifactsRequest)
	if err := dec(in); err != nil {
		return nil, err
	}
	if interceptor == nil {
		return srv.(AgentServiceServer).FetchArtifacts(ctx, in)
	}
	info := &grpc.UnaryServerInfo{
		Server:     srv,
		FullMethod: "/forgeiso.agent.v1.AgentService/FetchArtifacts",
	}
	handler := func(ctx context.Context, req any) (any, error) {
		return srv.(AgentServiceServer).FetchArtifacts(ctx, req.(*FetchArtifactsRequest))
	}
	return interceptor(ctx, in, info, handler)
}

func _AgentService_StreamBuildLogs_Handler(srv any, stream grpc.ServerStream) error {
	m := new(StreamLogsRequest)
	if err := stream.RecvMsg(m); err != nil {
		return err
	}
	return srv.(AgentServiceServer).StreamBuildLogs(m, &agentServiceStreamBuildLogsServer{stream})
}

type agentServiceStreamBuildLogsServer struct {
	grpc.ServerStream
}

func (x *agentServiceStreamBuildLogsServer) Send(m *LogEntry) error {
	return x.ServerStream.SendMsg(m)
}

func _AgentService_StreamTestLogs_Handler(srv any, stream grpc.ServerStream) error {
	m := new(StreamLogsRequest)
	if err := stream.RecvMsg(m); err != nil {
		return err
	}
	return srv.(AgentServiceServer).StreamTestLogs(m, &agentServiceStreamTestLogsServer{stream})
}

type agentServiceStreamTestLogsServer struct {
	grpc.ServerStream
}

func (x *agentServiceStreamTestLogsServer) Send(m *LogEntry) error {
	return x.ServerStream.SendMsg(m)
}
