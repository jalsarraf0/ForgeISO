package main

import (
	"crypto/rand"
	"crypto/rsa"
	"crypto/tls"
	"crypto/x509"
	"crypto/x509/pkix"
	"encoding/pem"
	"flag"
	"fmt"
	"log"
	"math/big"
	"net"
	"os"
	"path/filepath"
	"time"

	"github.com/jalsarraf0/ForgeISO/agent/internal/api"
	"github.com/jalsarraf0/ForgeISO/agent/internal/auth"
	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials"
)

func main() {
	var (
		listenAddr   string
		certFile     string
		keyFile      string
		caFile       string
		workDir      string
		jobToken     string
		selfSigned   bool
		requireMTLS  bool
	)

	flag.StringVar(&listenAddr, "listen", ":7443", "gRPC listen address")
	flag.StringVar(&certFile, "tls-cert", "", "TLS server certificate PEM path")
	flag.StringVar(&keyFile, "tls-key", "", "TLS server private key PEM path")
	flag.StringVar(&caFile, "tls-ca", "", "CA PEM path for mTLS client verification")
	flag.StringVar(&workDir, "workdir", "/tmp/forgeiso-agent", "agent workspace")
	flag.StringVar(&jobToken, "job-token", os.Getenv("FORGEISO_AGENT_TOKEN"), "job token for API authorization")
	flag.BoolVar(&selfSigned, "self-signed", true, "generate self-signed certificate if none supplied")
	flag.BoolVar(&requireMTLS, "require-mtls", true, "require client TLS certificate")
	flag.Parse()

	if err := os.MkdirAll(workDir, 0o755); err != nil {
		log.Fatalf("failed to create workdir: %v", err)
	}

	tlsConf, err := loadTLSConfig(certFile, keyFile, caFile, selfSigned, requireMTLS)
	if err != nil {
		log.Fatalf("failed to configure TLS: %v", err)
	}

	lis, err := net.Listen("tcp", listenAddr)
	if err != nil {
		log.Fatalf("failed to listen on %s: %v", listenAddr, err)
	}

	server := grpc.NewServer(
		grpc.Creds(credentials.NewTLS(tlsConf)),
		grpc.ForceServerCodec(api.JSONCodec{}),
		grpc.ChainUnaryInterceptor(auth.UnaryInterceptor(auth.Config{JobToken: jobToken, RequireMTLS: requireMTLS})),
		grpc.ChainStreamInterceptor(auth.StreamInterceptor(auth.Config{JobToken: jobToken, RequireMTLS: requireMTLS})),
	)

	api.RegisterAgentServiceServer(server, api.NewService(workDir))
	log.Printf("forgeiso-agent listening on %s", listenAddr)
	if err := server.Serve(lis); err != nil {
		log.Fatalf("serve failed: %v", err)
	}
}

func loadTLSConfig(certFile, keyFile, caFile string, selfSigned, requireMTLS bool) (*tls.Config, error) {
	if certFile == "" || keyFile == "" {
		if !selfSigned {
			return nil, fmt.Errorf("tls-cert and tls-key are required unless --self-signed is enabled")
		}

		tmpDir, err := os.MkdirTemp("", "forgeiso-agent-cert")
		if err != nil {
			return nil, err
		}
		certFile = filepath.Join(tmpDir, "server.crt")
		keyFile = filepath.Join(tmpDir, "server.key")
		if err := writeSelfSignedPair(certFile, keyFile); err != nil {
			return nil, err
		}
	}

	cert, err := tls.LoadX509KeyPair(certFile, keyFile)
	if err != nil {
		return nil, err
	}

	conf := &tls.Config{MinVersion: tls.VersionTLS13, Certificates: []tls.Certificate{cert}}

	if caFile != "" {
		caBytes, err := os.ReadFile(caFile)
		if err != nil {
			return nil, err
		}
		pool := x509.NewCertPool()
		if !pool.AppendCertsFromPEM(caBytes) {
			return nil, fmt.Errorf("failed to parse CA PEM")
		}
		conf.ClientCAs = pool
		if requireMTLS {
			conf.ClientAuth = tls.RequireAndVerifyClientCert
		}
	} else if requireMTLS {
		conf.ClientAuth = tls.RequireAnyClientCert
	}

	return conf, nil
}

func writeSelfSignedPair(certPath, keyPath string) error {
	priv, err := rsa.GenerateKey(rand.Reader, 2048)
	if err != nil {
		return err
	}

	notBefore := time.Now().Add(-1 * time.Hour)
	notAfter := time.Now().Add(365 * 24 * time.Hour)
	serial, err := rand.Int(rand.Reader, big.NewInt(1<<62))
	if err != nil {
		return err
	}

	tpl := &x509.Certificate{
		SerialNumber: serial,
		Subject: pkix.Name{
			CommonName: "forgeiso-agent",
		},
		NotBefore:             notBefore,
		NotAfter:              notAfter,
		KeyUsage:              x509.KeyUsageKeyEncipherment | x509.KeyUsageDigitalSignature,
		ExtKeyUsage:           []x509.ExtKeyUsage{x509.ExtKeyUsageServerAuth},
		BasicConstraintsValid: true,
		DNSNames:              []string{"localhost"},
	}

	der, err := x509.CreateCertificate(rand.Reader, tpl, tpl, &priv.PublicKey, priv)
	if err != nil {
		return err
	}

	certOut, err := os.Create(certPath)
	if err != nil {
		return err
	}
	defer certOut.Close()

	if err := pem.Encode(certOut, &pem.Block{Type: "CERTIFICATE", Bytes: der}); err != nil {
		return err
	}

	keyOut, err := os.Create(keyPath)
	if err != nil {
		return err
	}
	defer keyOut.Close()

	if err := pem.Encode(keyOut, &pem.Block{Type: "RSA PRIVATE KEY", Bytes: x509.MarshalPKCS1PrivateKey(priv)}); err != nil {
		return err
	}

	return nil
}
