package main

import (
	"crypto/rand"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"log"
	"net"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
	"sync"
	"time"
)

const (
	serviceName       = "toney-musicd"
	serviceVersion    = "0.2.0"
	maxUploadBytes    = int64(8 << 30)
	maxRequestBytes   = int64(8<<30 + 32<<20)
	pluginRoute       = "/apps/toney-music"
	defaultListenHost = "0.0.0.0"
	defaultListenPort = 3000
)

var supportedExtensions = []string{
	"flac", "wav", "wave", "aiff", "aif", "mp3", "aac", "m4a", "ogg", "oga", "opus",
	"dsf", "dff", "ape", "wv", "alac",
}

type trackRecord struct {
	ID              string  `json:"id"`
	Key             string  `json:"key"`
	FileName        string  `json:"fileName"`
	StoredName      string  `json:"storedName"`
	Title           string  `json:"title"`
	Artist          string  `json:"artist"`
	Album           string  `json:"album,omitempty"`
	Extension       string  `json:"extension"`
	Size            int64   `json:"size"`
	Duration        float64 `json:"duration"`
	Tone            int     `json:"tone"`
	AddedAt         int64   `json:"addedAt"`
	MetadataVersion int     `json:"metadataVersion,omitempty"`
}

type libraryFile struct {
	Version int           `json:"version"`
	Tracks  []trackRecord `json:"tracks"`
}

type trackResponse struct {
	ID        string  `json:"id"`
	Key       string  `json:"key"`
	FileName  string  `json:"fileName"`
	Title     string  `json:"title"`
	Artist    string  `json:"artist"`
	Album     string  `json:"album,omitempty"`
	Extension string  `json:"extension"`
	Size      int64   `json:"size"`
	Duration  float64 `json:"duration"`
	Tone      int     `json:"tone"`
	AddedAt   int64   `json:"addedAt"`
}

type audioMetadata struct {
	Title       string
	Artist      string
	Album       string
	AlbumArtist string
}

type server struct {
	mu          sync.RWMutex
	dataDir     string
	libraryDir  string
	cacheDir    string
	publicDir   string
	storePath   string
	ffmpeg      string
	tracks      map[string]trackRecord
	decodeLocks map[string]*sync.Mutex
}

func main() {
	dataDir := getenv("DATA_DIR", filepath.Join(".", "data"))
	publicDir := getenv("PUBLIC_DIR", filepath.Join(".", "public"))
	host := getenv("HOST", defaultListenHost)
	port := parsePort(getenv("PORT", strconv.Itoa(defaultListenPort)))
	ffmpeg := strings.TrimSpace(os.Getenv("FFMPEG_BIN"))

	if err := os.MkdirAll(dataDir, 0o755); err != nil {
		log.Fatalf("create data directory: %v", err)
	}
	libraryDir := filepath.Join(dataDir, "library")
	cacheDir := filepath.Join(dataDir, "decoded-cache")
	if err := os.MkdirAll(libraryDir, 0o755); err != nil {
		log.Fatalf("create library directory: %v", err)
	}
	if err := os.MkdirAll(cacheDir, 0o755); err != nil {
		log.Fatalf("create decoded cache directory: %v", err)
	}

	app := &server{
		dataDir:     dataDir,
		libraryDir:  libraryDir,
		cacheDir:    cacheDir,
		publicDir:   publicDir,
		storePath:   filepath.Join(dataDir, "library.json"),
		ffmpeg:      ffmpeg,
		tracks:      map[string]trackRecord{},
		decodeLocks: map[string]*sync.Mutex{},
	}
	if err := app.loadLibrary(); err != nil {
		log.Fatalf("load library: %v", err)
	}

	log.Printf("%s %s listening on %s:%d", serviceName, serviceVersion, host, port)
	if app.ffmpeg != "" {
		log.Printf("decoder: %s", app.ffmpeg)
	} else {
		log.Printf("decoder: unavailable; set FFMPEG_BIN or install ffmpeg")
	}

	httpServer := &http.Server{
		Addr:              net.JoinHostPort(host, strconv.Itoa(port)),
		Handler:           app,
		ReadHeaderTimeout: 10 * time.Second,
		ReadTimeout:       30 * time.Minute,
		WriteTimeout:      30 * time.Minute,
		IdleTimeout:       2 * time.Minute,
	}
	if err := httpServer.ListenAndServe(); err != nil && !errors.Is(err, http.ErrServerClosed) {
		log.Fatal(err)
	}
}

func (s *server) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	setCommonHeaders(w)
	if r.Method == http.MethodOptions {
		w.WriteHeader(http.StatusNoContent)
		return
	}

	requestPath := normalizePluginPath(r.URL.Path)
	switch {
	case requestPath == "/health":
		s.handleHealth(w, r)
	case requestPath == "/api/status":
		s.handleStatus(w, r)
	case requestPath == "/api/tracks" && r.Method == http.MethodGet:
		s.handleListTracks(w, r)
	case requestPath == "/api/tracks" && r.Method == http.MethodPost:
		s.handleUpload(w, r)
	case strings.HasPrefix(requestPath, "/api/tracks/"):
		s.handleTrackRoute(w, r, strings.TrimPrefix(requestPath, "/api/tracks/"))
	default:
		s.handlePublic(w, r, requestPath)
	}
}

func (s *server) handleHealth(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet && r.Method != http.MethodHead {
		methodNotAllowed(w)
		return
	}
	if !s.decoderAvailable() {
		writeJSON(w, http.StatusServiceUnavailable, map[string]any{
			"status": "degraded", "service": serviceName,
			"error": "ffmpeg decoder is unavailable",
		})
		return
	}
	writeJSON(w, http.StatusOK, map[string]any{
		"status": "ok", "service": serviceName, "version": serviceVersion,
	})
}

func (s *server) handleStatus(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		methodNotAllowed(w)
		return
	}
	s.mu.RLock()
	trackCount := len(s.tracks)
	ffmpeg := s.ffmpeg
	s.mu.RUnlock()
	writeJSON(w, http.StatusOK, map[string]any{
		"service": serviceName, "version": serviceVersion, "tracks": trackCount,
		"decoder": map[string]any{
			"engine": "ffmpeg-bundled", "available": executableExists(ffmpeg),
			"path": ffmpeg, "formats": supportedExtensions,
		},
	})
}

func (s *server) handleListTracks(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		methodNotAllowed(w)
		return
	}
	s.mu.RLock()
	tracks := make([]trackRecord, 0, len(s.tracks))
	for _, track := range s.tracks {
		tracks = append(tracks, track)
	}
	s.mu.RUnlock()
	sort.Slice(tracks, func(i, j int) bool { return tracks[i].AddedAt < tracks[j].AddedAt })
	responses := make([]trackResponse, 0, len(tracks))
	for _, track := range tracks {
		responses = append(responses, publicTrack(track))
	}
	writeJSON(w, http.StatusOK, map[string]any{"tracks": responses})
}

func (s *server) handleUpload(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		methodNotAllowed(w)
		return
	}
	r.Body = http.MaxBytesReader(w, r.Body, maxRequestBytes)
	reader, err := r.MultipartReader()
	if err != nil {
		writeError(w, http.StatusBadRequest, "expected multipart form data")
		return
	}

	for {
		part, nextErr := reader.NextPart()
		if errors.Is(nextErr, io.EOF) {
			break
		}
		if nextErr != nil {
			writeError(w, http.StatusBadRequest, "invalid multipart payload")
			return
		}
		if part.FileName() == "" || part.FormName() != "file" {
			_ = part.Close()
			continue
		}
		track, duplicate, uploadErr := s.storeUpload(part, filepath.Base(part.FileName()))
		_ = part.Close()
		if uploadErr != nil {
			writeError(w, http.StatusBadRequest, uploadErr.Error())
			return
		}
		writeJSON(w, http.StatusCreated, map[string]any{
			"track": publicTrack(track), "duplicate": duplicate,
		})
		return
	}
	writeError(w, http.StatusBadRequest, "missing file field")
}

func (s *server) handleTrackRoute(w http.ResponseWriter, r *http.Request, suffix string) {
	parts := strings.Split(strings.Trim(suffix, "/"), "/")
	if len(parts) == 2 && parts[0] != "" && parts[1] == "stream" {
		s.handleStream(w, r, parts[0])
		return
	}
	if len(parts) == 1 && parts[0] != "" && r.Method == http.MethodDelete {
		s.handleDeleteTrack(w, r, parts[0])
		return
	}
	writeError(w, http.StatusNotFound, "track route not found")
}

func (s *server) handleDeleteTrack(w http.ResponseWriter, r *http.Request, id string) {
	track, ok := s.findTrack(id)
	if !ok {
		writeError(w, http.StatusNotFound, "track not found")
		return
	}
	if err := os.Remove(filepath.Join(s.libraryDir, track.StoredName)); err != nil && !errors.Is(err, os.ErrNotExist) {
		writeError(w, http.StatusInternalServerError, "failed to remove source file")
		return
	}
	_ = os.Remove(filepath.Join(s.cacheDir, id+".wav"))

	s.mu.Lock()
	delete(s.tracks, id)
	err := s.saveLibraryLocked()
	s.mu.Unlock()
	if err != nil {
		writeError(w, http.StatusInternalServerError, "failed to save library")
		return
	}
	w.WriteHeader(http.StatusNoContent)
}

func (s *server) handleStream(w http.ResponseWriter, r *http.Request, id string) {
	if r.Method != http.MethodGet && r.Method != http.MethodHead {
		methodNotAllowed(w)
		return
	}
	track, ok := s.findTrack(id)
	if !ok {
		writeError(w, http.StatusNotFound, "track not found")
		return
	}
	wavPath, err := s.ensureDecoded(track)
	if err != nil {
		log.Printf("decode %s failed: %v", id, err)
		writeError(w, http.StatusUnprocessableEntity, "unable to decode audio file")
		return
	}
	file, err := os.Open(wavPath)
	if err != nil {
		writeError(w, http.StatusInternalServerError, "decoded audio is unavailable")
		return
	}
	defer file.Close()
	info, err := file.Stat()
	if err != nil {
		writeError(w, http.StatusInternalServerError, "decoded audio metadata is unavailable")
		return
	}
	w.Header().Set("Content-Type", "audio/wav")
	w.Header().Set("Content-Disposition", "inline")
	w.Header().Set("Accept-Ranges", "bytes")
	http.ServeContent(w, r, track.FileName, info.ModTime(), file)
}

func (s *server) handlePublic(w http.ResponseWriter, r *http.Request, requestPath string) {
	if r.Method != http.MethodGet && r.Method != http.MethodHead {
		methodNotAllowed(w)
		return
	}
	if requestPath == "/" || requestPath == "/index.html" {
		requestPath = "/index.html"
	}
	if strings.Contains(requestPath, "..") {
		writeError(w, http.StatusNotFound, "not found")
		return
	}
	relative := strings.TrimPrefix(requestPath, "/")
	filePath := filepath.Join(s.publicDir, filepath.FromSlash(relative))
	if !withinDirectory(s.publicDir, filePath) {
		writeError(w, http.StatusNotFound, "not found")
		return
	}
	if _, err := os.Stat(filePath); err != nil {
		writeError(w, http.StatusNotFound, "not found")
		return
	}
	http.ServeFile(w, r, filePath)
}

func (s *server) storeUpload(part io.Reader, fileName string) (trackRecord, bool, error) {
	ext := strings.TrimPrefix(strings.ToLower(filepath.Ext(fileName)), ".")
	if !isSupportedExtension(ext) {
		return trackRecord{}, false, fmt.Errorf("unsupported audio format: %s", ext)
	}
	tmp, err := os.CreateTemp(s.libraryDir, ".upload-*")
	if err != nil {
		return trackRecord{}, false, fmt.Errorf("create upload file: %w", err)
	}
	tmpName := tmp.Name()
	defer func() {
		_ = tmp.Close()
		_ = os.Remove(tmpName)
	}()
	hasher := sha256.New()
	size, err := io.Copy(io.MultiWriter(tmp, hasher), io.LimitReader(part, maxUploadBytes+1))
	if err != nil {
		return trackRecord{}, false, fmt.Errorf("save upload: %w", err)
	}
	if size == 0 {
		return trackRecord{}, false, errors.New("audio file is empty")
	}
	if size > maxUploadBytes {
		return trackRecord{}, false, errors.New("audio file is larger than 8 GiB")
	}
	if err := tmp.Close(); err != nil {
		return trackRecord{}, false, fmt.Errorf("close upload: %w", err)
	}
	key := hex.EncodeToString(hasher.Sum(nil)) + ":" + strconv.FormatInt(size, 10)

	s.mu.RLock()
	for _, existing := range s.tracks {
		if existing.Key == key {
			s.mu.RUnlock()
			return existing, true, nil
		}
	}
	s.mu.RUnlock()
	id := newID()
	storedName := id + "." + ext
	finalPath := filepath.Join(s.libraryDir, storedName)
	if err := os.Rename(tmpName, finalPath); err != nil {
		return trackRecord{}, false, fmt.Errorf("finalize upload: %w", err)
	}
	title, artist := parseTrackName(fileName)
	album := ""
	if metadata, ok := s.readAudioMetadata(finalPath); ok {
		title = firstNonEmpty(metadata.Title, title)
		artist = firstNonEmpty(metadata.Artist, metadata.AlbumArtist, artist)
		album = metadata.Album
	}
	track := trackRecord{
		ID: id, Key: key, FileName: fileName, StoredName: storedName,
		Title: title, Artist: artist, Album: album, Extension: ext, Size: size,
		Tone: toneFor(key), AddedAt: time.Now().UnixMilli(), MetadataVersion: 1,
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	for _, existing := range s.tracks {
		if existing.Key == key {
			_ = os.Remove(finalPath)
			return existing, true, nil
		}
	}
	s.tracks[id] = track
	if err := s.saveLibraryLocked(); err != nil {
		delete(s.tracks, id)
		_ = os.Remove(finalPath)
		return trackRecord{}, false, fmt.Errorf("save library: %w", err)
	}
	return track, false, nil
}

func (s *server) readAudioMetadata(sourcePath string) (audioMetadata, bool) {
	if !s.decoderAvailable() {
		return audioMetadata{}, false
	}
	cmd := exec.Command(s.ffmpeg,
		"-nostdin", "-hide_banner", "-loglevel", "error",
		"-i", sourcePath, "-map_metadata", "0", "-f", "ffmetadata", "-",
	)
	output, err := cmd.Output()
	if err != nil {
		return audioMetadata{}, false
	}
	metadata := parseFFMetadata(string(output))
	return metadata, true
}

func parseFFMetadata(value string) audioMetadata {
	var metadata audioMetadata
	for _, line := range strings.Split(value, "\n") {
		line = strings.TrimSpace(line)
		if line == "" || strings.HasPrefix(line, ";") || strings.HasPrefix(line, "[") {
			continue
		}
		separator := strings.IndexByte(line, '=')
		if separator <= 0 {
			continue
		}
		key := strings.ToLower(strings.TrimSpace(line[:separator]))
		item := strings.TrimSpace(unescapeFFMetadata(line[separator+1:]))
		if item == "" {
			continue
		}
		switch strings.ReplaceAll(key, "_", "") {
		case "title", "tracktitle":
			metadata.Title = item
		case "artist", "trackartist":
			metadata.Artist = item
		case "album":
			metadata.Album = item
		case "albumartist", "albumartists":
			metadata.AlbumArtist = item
		}
	}
	return metadata
}

func unescapeFFMetadata(value string) string {
	var result strings.Builder
	result.Grow(len(value))
	escaped := false
	for _, character := range value {
		if escaped {
			switch character {
			case 'n':
				result.WriteByte('\n')
			case 'r':
				result.WriteByte('\r')
			default:
				result.WriteRune(character)
			}
			escaped = false
			continue
		}
		if character == '\\' {
			escaped = true
			continue
		}
		result.WriteRune(character)
	}
	if escaped {
		result.WriteByte('\\')
	}
	return strings.TrimSpace(result.String())
}

func firstNonEmpty(values ...string) string {
	for _, value := range values {
		if trimmed := strings.TrimSpace(value); trimmed != "" {
			return trimmed
		}
	}
	return ""
}

func (s *server) ensureDecoded(track trackRecord) (string, error) {
	if !s.decoderAvailable() {
		return "", errors.New("ffmpeg decoder is unavailable")
	}
	cachePath := filepath.Join(s.cacheDir, track.ID+".wav")
	if validDecodedFile(cachePath) {
		return cachePath, nil
	}
	lock := s.decodeLock(track.ID)
	lock.Lock()
	defer lock.Unlock()
	if validDecodedFile(cachePath) {
		return cachePath, nil
	}
	sourcePath := filepath.Join(s.libraryDir, track.StoredName)
	if _, err := os.Stat(sourcePath); err != nil {
		return "", err
	}
	tmp, err := os.CreateTemp(s.cacheDir, track.ID+"-*.wav")
	if err != nil {
		return "", err
	}
	tmpPath := tmp.Name()
	if err := tmp.Close(); err != nil {
		_ = os.Remove(tmpPath)
		return "", err
	}
	defer os.Remove(tmpPath)
	cmd := exec.Command(s.ffmpeg,
		"-nostdin", "-hide_banner", "-loglevel", "error", "-y",
		"-i", sourcePath, "-vn", "-map_metadata", "-1",
		"-c:a", "pcm_s16le", "-f", "wav", tmpPath,
	)
	output, err := cmd.CombinedOutput()
	if err != nil {
		message := strings.TrimSpace(string(output))
		if message == "" {
			message = err.Error()
		}
		return "", errors.New(message)
	}
	if !validDecodedFile(tmpPath) {
		return "", errors.New("ffmpeg returned an empty WAV")
	}
	if err := os.Rename(tmpPath, cachePath); err != nil {
		return "", err
	}
	return cachePath, nil
}

func (s *server) decodeLock(id string) *sync.Mutex {
	s.mu.Lock()
	defer s.mu.Unlock()
	lock := s.decodeLocks[id]
	if lock == nil {
		lock = &sync.Mutex{}
		s.decodeLocks[id] = lock
	}
	return lock
}

func (s *server) loadLibrary() error {
	data, err := os.ReadFile(s.storePath)
	if errors.Is(err, os.ErrNotExist) {
		return nil
	}
	if err != nil {
		return err
	}
	var stored libraryFile
	if err := json.Unmarshal(data, &stored); err != nil {
		return err
	}
	s.mu.Lock()
	for _, track := range stored.Tracks {
		if track.ID != "" && track.StoredName != "" {
			s.tracks[track.ID] = track
		}
	}
	s.mu.Unlock()
	return s.refreshLegacyMetadata()
}

func (s *server) refreshLegacyMetadata() error {
	s.mu.RLock()
	legacy := make([]trackRecord, 0)
	for _, track := range s.tracks {
		if track.MetadataVersion < 1 {
			legacy = append(legacy, track)
		}
	}
	s.mu.RUnlock()
	if len(legacy) == 0 {
		return nil
	}

	updates := make([]trackRecord, 0, len(legacy))
	for _, track := range legacy {
		metadata, ok := s.readAudioMetadata(filepath.Join(s.libraryDir, track.StoredName))
		if !ok {
			continue
		}
		fallbackTitle, fallbackArtist := parseTrackName(track.FileName)
		track.Title = firstNonEmpty(metadata.Title, track.Title, fallbackTitle)
		track.Artist = firstNonEmpty(metadata.Artist, metadata.AlbumArtist, track.Artist, fallbackArtist)
		track.Album = metadata.Album
		track.MetadataVersion = 1
		updates = append(updates, track)
	}
	if len(updates) == 0 {
		return nil
	}

	s.mu.Lock()
	for _, track := range updates {
		if _, exists := s.tracks[track.ID]; exists {
			s.tracks[track.ID] = track
		}
	}
	err := s.saveLibraryLocked()
	s.mu.Unlock()
	return err
}

func (s *server) saveLibraryLocked() error {
	tracks := make([]trackRecord, 0, len(s.tracks))
	for _, track := range s.tracks {
		tracks = append(tracks, track)
	}
	sort.Slice(tracks, func(i, j int) bool { return tracks[i].AddedAt < tracks[j].AddedAt })
	payload, err := json.MarshalIndent(libraryFile{Version: 1, Tracks: tracks}, "", "  ")
	if err != nil {
		return err
	}
	tmp, err := os.CreateTemp(s.dataDir, "library-*.json")
	if err != nil {
		return err
	}
	tmpName := tmp.Name()
	defer func() {
		_ = tmp.Close()
		_ = os.Remove(tmpName)
	}()
	if _, err := tmp.Write(payload); err != nil {
		return err
	}
	if err := tmp.Close(); err != nil {
		return err
	}
	return os.Rename(tmpName, s.storePath)
}

func (s *server) findTrack(id string) (trackRecord, bool) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	track, ok := s.tracks[id]
	return track, ok
}

func (s *server) decoderAvailable() bool {
	s.mu.RLock()
	ffmpeg := s.ffmpeg
	s.mu.RUnlock()
	return executableExists(ffmpeg)
}

func publicTrack(track trackRecord) trackResponse {
	return trackResponse{
		ID: track.ID, Key: track.Key, FileName: track.FileName, Title: track.Title,
		Artist: track.Artist, Album: track.Album, Extension: track.Extension, Size: track.Size,
		Duration: track.Duration, Tone: track.Tone, AddedAt: track.AddedAt,
	}
}

func normalizePluginPath(path string) string {
	if path == pluginRoute {
		return "/"
	}
	if strings.HasPrefix(path, pluginRoute+"/") {
		return strings.TrimPrefix(path, pluginRoute)
	}
	if path == "" {
		return "/"
	}
	return path
}

func withinDirectory(root, candidate string) bool {
	rootAbs, err := filepath.Abs(root)
	if err != nil {
		return false
	}
	candidateAbs, err := filepath.Abs(candidate)
	if err != nil {
		return false
	}
	relative, err := filepath.Rel(rootAbs, candidateAbs)
	return err == nil && relative != ".." && !strings.HasPrefix(relative, ".."+string(os.PathSeparator))
}

func validDecodedFile(path string) bool {
	info, err := os.Stat(path)
	return err == nil && info.Size() > 44
}

func executableExists(path string) bool {
	if path == "" {
		return false
	}
	info, err := os.Stat(path)
	return err == nil && !info.IsDir()
}

func isSupportedExtension(ext string) bool {
	for _, supported := range supportedExtensions {
		if ext == supported {
			return true
		}
	}
	return false
}

func parseTrackName(fileName string) (string, string) {
	base := strings.TrimSuffix(fileName, filepath.Ext(fileName))
	for _, separator := range []string{" – ", " — ", " - "} {
		if parts := strings.SplitN(base, separator, 2); len(parts) == 2 {
			artist := strings.TrimSpace(parts[0])
			title := strings.TrimSpace(parts[1])
			if title != "" {
				if artist == "" {
					artist = "Local file"
				}
				return title, artist
			}
		}
	}
	if base == "" {
		base = fileName
	}
	return base, "Local file"
}

func toneFor(value string) int {
	hash := sha256.Sum256([]byte(value))
	return int(hash[0] % 5)
}

func newID() string {
	var bytes [12]byte
	if _, err := rand.Read(bytes[:]); err == nil {
		return hex.EncodeToString(bytes[:])
	}
	return strconv.FormatInt(time.Now().UnixNano(), 36)
}

func setCommonHeaders(w http.ResponseWriter) {
	w.Header().Set("Access-Control-Allow-Origin", "*")
	w.Header().Set("Access-Control-Allow-Headers", "Content-Type, Range")
	w.Header().Set("Access-Control-Allow-Methods", "GET, HEAD, POST, DELETE, OPTIONS")
}

func writeJSON(w http.ResponseWriter, status int, value any) {
	w.Header().Set("Content-Type", "application/json; charset=utf-8")
	w.WriteHeader(status)
	if status != http.StatusNoContent {
		_ = json.NewEncoder(w).Encode(value)
	}
}

func writeError(w http.ResponseWriter, status int, message string) {
	writeJSON(w, status, map[string]string{"error": message})
}

func methodNotAllowed(w http.ResponseWriter) {
	writeError(w, http.StatusMethodNotAllowed, "method not allowed")
}

func getenv(name, fallback string) string {
	value := strings.TrimSpace(os.Getenv(name))
	if value == "" {
		return fallback
	}
	return value
}

func parsePort(value string) int {
	port, err := strconv.Atoi(value)
	if err != nil || port < 1 || port > 65535 {
		return defaultListenPort
	}
	return port
}
