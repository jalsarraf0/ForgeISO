package auth

import (
	"context"
	"crypto/tls"
	"crypto/x509"
	"testing"

	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/credentials"
	"google.golang.org/grpc/metadata"
	"google.golang.org/grpc/peer"
	"google.golang.org/grpc/status"
)

func TestAuthorizeTokenSuccess(t *testing.T) {
	ctx := metadata.NewIncomingContext(context.Background(), metadata.Pairs("x-job-token", "abc"))
	if err := authorize(ctx, Config{JobToken: "abc", RequireMTLS: false}); err != nil {
		t.Fatalf("expected success: %v", err)
	}
}

func TestAuthorizeTokenFailure(t *testing.T) {
	ctx := metadata.NewIncomingContext(context.Background(), metadata.Pairs("x-job-token", "bad"))
	err := authorize(ctx, Config{JobToken: "abc", RequireMTLS: false})
	if status.Code(err) != codes.Unauthenticated {
		t.Fatalf("expected unauthenticated, got %v", err)
	}
}

func TestAuthorizeMTLSSuccess(t *testing.T) {
	ctx := metadata.NewIncomingContext(context.Background(), metadata.Pairs("x-job-token", "abc"))
	tlsInfo := credentials.TLSInfo{State: tls.ConnectionState{PeerCertificates: []*x509.Certificate{{}}}}
	ctx = peer.NewContext(ctx, &peer.Peer{AuthInfo: tlsInfo})

	if err := authorize(ctx, Config{JobToken: "abc", RequireMTLS: true}); err != nil {
		t.Fatalf("expected success: %v", err)
	}
}
