package auth

import (
	"context"

	"google.golang.org/grpc"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/credentials"
	"google.golang.org/grpc/metadata"
	"google.golang.org/grpc/peer"
	"google.golang.org/grpc/status"
)

type Config struct {
	JobToken    string
	RequireMTLS bool
}

func UnaryInterceptor(cfg Config) grpc.UnaryServerInterceptor {
	return func(
		ctx context.Context,
		req any,
		info *grpc.UnaryServerInfo,
		handler grpc.UnaryHandler,
	) (any, error) {
		if err := authorize(ctx, cfg); err != nil {
			return nil, err
		}
		return handler(ctx, req)
	}
}

func StreamInterceptor(cfg Config) grpc.StreamServerInterceptor {
	return func(
		srv any,
		ss grpc.ServerStream,
		info *grpc.StreamServerInfo,
		handler grpc.StreamHandler,
	) error {
		if err := authorize(ss.Context(), cfg); err != nil {
			return err
		}
		return handler(srv, ss)
	}
}

func authorize(ctx context.Context, cfg Config) error {
	if cfg.JobToken != "" {
		md, ok := metadata.FromIncomingContext(ctx)
		if !ok {
			return status.Error(codes.Unauthenticated, "missing request metadata")
		}

		tokens := md.Get("x-job-token")
		if len(tokens) == 0 || tokens[0] != cfg.JobToken {
			return status.Error(codes.Unauthenticated, "invalid job token")
		}
	}

	if cfg.RequireMTLS {
		p, ok := peer.FromContext(ctx)
		if !ok {
			return status.Error(codes.Unauthenticated, "missing peer context")
		}

		tlsInfo, ok := p.AuthInfo.(credentials.TLSInfo)
		if !ok {
			return status.Error(codes.Unauthenticated, "TLS client auth required")
		}

		if len(tlsInfo.State.PeerCertificates) == 0 {
			return status.Error(codes.Unauthenticated, "client certificate required")
		}
	}

	return nil
}
