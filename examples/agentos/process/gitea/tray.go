package main

import (
	"bytes"
	"context"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"io"
	"net"
	"net/http"
	"os"
	"path/filepath"
	"time"
)

const (
	trayID       = "main"
	trayEndpoint = "/v1/icons/" + trayID
	eventsURL    = "/v1/events?wait_ms=25000"
)

func main() {
	socketPath := os.Getenv("AGENTOS_TRAY_SOCKET")
	if socketPath == "" {
		return
	}

	transport := &http.Transport{
		DialContext: func(ctx context.Context, _, _ string) (net.Conn, error) {
			return (&net.Dialer{}).DialContext(ctx, "unix", socketPath)
		},
	}
	defer transport.CloseIdleConnections()
	client := &http.Client{Transport: transport, Timeout: 30 * time.Second}

	payload, err := trayPayload()
	if err != nil {
		fmt.Fprintf(os.Stderr, "Could not prepare Gitea tray icon: %v\n", err)
	}

	for {
		if err := trayRequest(client, http.MethodPut, trayEndpoint, payload); err != nil {
			fmt.Fprintf(os.Stderr, "Could not register Gitea tray icon: %v\n", err)
			time.Sleep(2 * time.Second)
			continue
		}

		for {
			// The Shell handles its built-in Open and Exit menu actions. Consume
			// broker events so tray clicks do not accumulate while Gitea runs.
			if err := trayRequest(client, http.MethodGet, eventsURL, nil); err != nil {
				fmt.Fprintf(os.Stderr, "AgentOS tray IPC temporarily unavailable: %v\n", err)
				break
			}
		}
		time.Sleep(2 * time.Second)
	}
}

func trayPayload() ([]byte, error) {
	iconPath := filepath.Join(filepath.Dir(os.Args[0]), "icon.png")
	icon, err := os.ReadFile(iconPath)
	if err != nil {
		return nil, err
	}
	return json.Marshal(map[string]interface{}{
		"tooltip":       "Gitea",
		"iconPngBase64": base64.StdEncoding.EncodeToString(icon),
		"menu":          []interface{}{},
	})
}

func trayRequest(client *http.Client, method, path string, body []byte) error {
	var requestBody io.Reader
	if body != nil {
		requestBody = bytes.NewReader(body)
	}
	request, err := http.NewRequest(method, "http://agentos"+path, requestBody)
	if err != nil {
		return err
	}
	if body != nil {
		request.Header.Set("Content-Type", "application/json")
	}

	response, err := client.Do(request)
	if err != nil {
		return err
	}
	defer response.Body.Close()
	_, _ = io.Copy(io.Discard, response.Body)
	if response.StatusCode < http.StatusOK || response.StatusCode >= http.StatusMultipleChoices {
		return fmt.Errorf("AgentOS tray request returned HTTP %d", response.StatusCode)
	}
	return nil
}
