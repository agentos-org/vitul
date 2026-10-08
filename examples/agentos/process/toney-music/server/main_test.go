package main

import (
	"encoding/json"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"testing"
)

func newTestServer(t *testing.T) *server {
	t.Helper()
	root := t.TempDir()
	libraryDir := filepath.Join(root, "library")
	cacheDir := filepath.Join(root, "decoded-cache")
	if err := os.MkdirAll(libraryDir, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(cacheDir, 0o755); err != nil {
		t.Fatal(err)
	}
	return &server{
		dataDir:     root,
		libraryDir:  libraryDir,
		cacheDir:    cacheDir,
		publicDir:   root,
		storePath:   filepath.Join(root, "library.json"),
		tracks:      map[string]trackRecord{},
		decodeLocks: map[string]*sync.Mutex{},
	}
}

func TestParseTrackName(t *testing.T) {
	tests := []struct {
		name   string
		file   string
		title  string
		artist string
	}{
		{name: "artist separator", file: "Tycho - Awake.flac", title: "Awake", artist: "Tycho"},
		{name: "unicode separator", file: "周杰伦 — 晴天.mp3", title: "晴天", artist: "周杰伦"},
		{name: "local file", file: "voice.wav", title: "voice", artist: "Local file"},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			title, artist := parseTrackName(test.file)
			if title != test.title || artist != test.artist {
				t.Fatalf("parseTrackName(%q) = %q, %q; want %q, %q", test.file, title, artist, test.title, test.artist)
			}
		})
	}
}

func TestParseFFMetadata(t *testing.T) {
	metadata := parseFFMetadata(`;FFMETADATA1
title=Tagged\=Title
artist=Tagged Artist
album_artist=Album Artist
album=Tagged Album
`)
	if metadata.Title != "Tagged=Title" || metadata.Artist != "Tagged Artist" || metadata.AlbumArtist != "Album Artist" || metadata.Album != "Tagged Album" {
		t.Fatalf("unexpected ffmetadata: %+v", metadata)
	}
}

func TestStoreUploadDeduplicatesAndPersists(t *testing.T) {
	app := newTestServer(t)
	first, duplicate, err := app.storeUpload(strings.NewReader("audio bytes"), "Tycho - Awake.flac")
	if err != nil {
		t.Fatal(err)
	}
	if duplicate {
		t.Fatal("first upload was reported as a duplicate")
	}
	if first.Title != "Awake" || first.Artist != "Tycho" {
		t.Fatalf("unexpected track metadata: %+v", first)
	}
	if _, err := os.Stat(filepath.Join(app.libraryDir, first.StoredName)); err != nil {
		t.Fatalf("stored upload is missing: %v", err)
	}

	second, duplicate, err := app.storeUpload(strings.NewReader("audio bytes"), "renamed.flac")
	if err != nil {
		t.Fatal(err)
	}
	if !duplicate || second.ID != first.ID {
		t.Fatalf("duplicate upload returned %+v, duplicate=%v", second, duplicate)
	}

	var persisted libraryFile
	data, err := os.ReadFile(app.storePath)
	if err != nil {
		t.Fatal(err)
	}
	if err := json.Unmarshal(data, &persisted); err != nil {
		t.Fatal(err)
	}
	if len(persisted.Tracks) != 1 || persisted.Tracks[0].ID != first.ID {
		t.Fatalf("unexpected persisted library: %+v", persisted)
	}
}

func TestListTracksReturnsPublicRecords(t *testing.T) {
	app := newTestServer(t)
	track, _, err := app.storeUpload(strings.NewReader("audio bytes"), "Local.wav")
	if err != nil {
		t.Fatal(err)
	}

	recorder := httptest.NewRecorder()
	request := httptest.NewRequest("GET", "/api/tracks", nil)
	app.ServeHTTP(recorder, request)
	if recorder.Code != 200 {
		t.Fatalf("GET /api/tracks returned %d: %s", recorder.Code, recorder.Body.String())
	}
	var response struct {
		Tracks []trackResponse `json:"tracks"`
	}
	if err := json.Unmarshal(recorder.Body.Bytes(), &response); err != nil {
		t.Fatal(err)
	}
	if len(response.Tracks) != 1 || response.Tracks[0].ID != track.ID {
		t.Fatalf("unexpected response: %+v", response)
	}
	if response.Tracks[0].Key == "" {
		t.Fatal("public track is missing its stable key")
	}
}
