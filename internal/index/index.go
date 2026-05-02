// Package index provides a JSON-based caching layer for agenttrace sessions.
// It uses file mtime+size for change detection and only re-parses new/modified files,
// eliminating the startup latency of re-parsing all JSON session files from scratch.
package index

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"

	"github.com/luoyuctl/agenttrace/internal/engine"
)

// DefaultPath returns the default index file path (~/.cache/agenttrace/index.json).
func DefaultPath() string {
	home, err := os.UserHomeDir()
	if err != nil {
		return ".cache/agenttrace/index.json"
	}
	return filepath.Join(home, ".cache", "agenttrace", "index.json")
}

// ── Index Structures ──

// IndexEntry is JSON-serializable metadata for a single session file.
type IndexEntry struct {
	Path        string  `json:"path"`
	Name        string  `json:"name"`
	SourceTool  string  `json:"source_tool"`
	Turns       int     `json:"turns"`
	Tools       int     `json:"tools"`
	Tokens      int     `json:"tokens"`
	Cost        float64 `json:"cost"`
	Health      int     `json:"health"`
	SuccessRate float64 `json:"success_rate"`
	FailCount   int     `json:"fail_count"`
	Mtime       int64   `json:"mtime"`
	Size        int64   `json:"size"`
	DurationSec float64 `json:"duration_sec"`
	ModelUsed   string  `json:"model_used"`
}

// Index is the top-level index structure, persisted as JSON.
type Index struct {
	Path     string                 `json:"-"`
	Entries  map[string]*IndexEntry `json:"entries"`
	LastScan time.Time              `json:"last_scan"`

	// In-memory cache (not serialized) — holds full Session objects for files
	// whose mtime+size haven't changed. Populated lazily.
	mu           sync.RWMutex
	sessionCache map[string]*engine.Session // keyed by file path
}

// New creates an empty Index that will be saved to the given path.
func New(path string) *Index {
	return &Index{
		Path:         path,
		Entries:      make(map[string]*IndexEntry),
		sessionCache: make(map[string]*engine.Session),
	}
}

// ── Persistent Load/Save ──

// Load reads the index from disk. Returns nil if the file doesn't exist.
func Load(path string) (*Index, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		if os.IsNotExist(err) {
			return nil, nil
		}
		return nil, fmt.Errorf("read index: %w", err)
	}
	idx := New(path)
	if err := json.Unmarshal(data, &idx.Entries); err != nil {
		return nil, fmt.Errorf("parse index: %w", err)
	}
	// Read LastScan
	var wrapper struct {
		Entries  map[string]*IndexEntry `json:"entries"`
		LastScan time.Time              `json:"last_scan"`
	}
	if err := json.Unmarshal(data, &wrapper); err == nil {
		idx.LastScan = wrapper.LastScan
	}
	return idx, nil
}

// Save writes the index to disk as JSON.
func (idx *Index) Save() error {
	dir := filepath.Dir(idx.Path)
	if err := os.MkdirAll(dir, 0755); err != nil {
		return fmt.Errorf("create index dir: %w", err)
	}
	wrapper := struct {
		Entries  map[string]*IndexEntry `json:"entries"`
		LastScan time.Time              `json:"last_scan"`
	}{
		Entries:  idx.Entries,
		LastScan: idx.LastScan,
	}
	data, err := json.MarshalIndent(wrapper, "", "  ")
	if err != nil {
		return fmt.Errorf("marshal index: %w", err)
	}
	if err := os.WriteFile(idx.Path, data, 0644); err != nil {
		return fmt.Errorf("write index: %w", err)
	}
	return nil
}

// ── Build / Update ──

// BuildOrUpdate scans a directory for session files, checks mtime+size against
// cached entries, re-parses only changed files, and saves the index.
func BuildOrUpdate(dir string) (*Index, error) {
	idxPath := DefaultPath()
	idx, err := Load(idxPath)
	if err != nil {
		// If load fails, start fresh
		idx = New(idxPath)
	}
	if idx == nil {
		idx = New(idxPath)
	}

	// Walk directory for *.json and *.jsonl files
	entries, err := os.ReadDir(dir)
	if err != nil {
		return idx, fmt.Errorf("read dir %s: %w", dir, err)
	}

	for _, e := range entries {
		if e.IsDir() {
			continue
		}
		name := e.Name()
		if !strings.HasSuffix(name, ".jsonl") && !strings.HasSuffix(name, ".json") {
			continue
		}
		// Skip non-session files
		if strings.HasPrefix(name, "request_dump_") || name == "sessions.json" {
			continue
		}

		path := filepath.Join(dir, name)
		info, err := e.Info()
		if err != nil {
			continue
		}

		mtime := info.ModTime().Unix()
		size := info.Size()

		// Check if cached entry is still valid
		if existing, ok := idx.Entries[path]; ok {
			if existing.Mtime == mtime && existing.Size == size {
				// File unchanged — keep existing entry and cached session
				continue
			}
		}

		// File is new or changed — parse it
		sess, err := engine.LoadSession(path)
		if err != nil {
			continue
		}

		// Compute derived fields
		totalTools := sess.Metrics.ToolCallsOK + sess.Metrics.ToolCallsFail
		successRate := 0.0
		if totalTools > 0 {
			successRate = float64(sess.Metrics.ToolCallsOK) / float64(totalTools) * 100
		}
		totalTokens := sess.Metrics.TokensInput + sess.Metrics.TokensOutput

		entry := &IndexEntry{
			Path:        path,
			Name:        sess.Name,
			SourceTool:  sess.Metrics.SourceTool,
			Turns:       sess.Metrics.AssistantTurns,
			Tools:       sess.Metrics.ToolCallsTotal,
			Tokens:      totalTokens,
			Cost:        sess.Metrics.CostEstimated,
			Health:      sess.Health,
			SuccessRate: successRate,
			FailCount:   sess.Metrics.ToolCallsFail,
			Mtime:       mtime,
			Size:        size,
			DurationSec: sess.Metrics.DurationSec,
			ModelUsed:   sess.Metrics.ModelUsed,
		}

		idx.Entries[path] = entry

		// Cache the full session in memory
		idx.mu.Lock()
		idx.sessionCache[path] = sess
		idx.mu.Unlock()
	}

	// Prune entries for files that no longer exist
	for key := range idx.Entries {
		if _, err := os.Stat(key); os.IsNotExist(err) {
			delete(idx.Entries, key)
			idx.mu.Lock()
			delete(idx.sessionCache, key)
			idx.mu.Unlock()
		}
	}

	idx.LastScan = time.Now()
	if err := idx.Save(); err != nil {
		return idx, fmt.Errorf("save index: %w", err)
	}

	return idx, nil
}

// ── GetSessions — the key optimization ──

// GetSessions returns all sessions from dir, using the index to skip re-parsing
// unchanged files. This gives fast metadata for list views while still loading
// full Session objects for detail views.
func GetSessions(dir string) ([]engine.Session, error) {
	idx, err := BuildOrUpdate(dir)
	if err != nil {
		return nil, err
	}

	var sessions []engine.Session

	for path, entry := range idx.Entries {
		// Check in-memory cache first
		idx.mu.RLock()
		cached, ok := idx.sessionCache[path]
		idx.mu.RUnlock()

		if ok {
			sessions = append(sessions, *cached)
			continue
		}

		// Load from disk (shouldn't normally reach here since BuildOrUpdate populates cache)
		sess, err := engine.LoadSession(path)
		if err != nil {
			continue
		}

		idx.mu.Lock()
		idx.sessionCache[path] = sess
		idx.mu.Unlock()

		sessions = append(sessions, *sess)
		_ = entry // entry used for metadata if needed
	}

	return sessions, nil
}

// GetIndex returns the loaded/updated index without loading full sessions.
// Useful when only metadata (turns, cost, health) is needed.
func GetIndex(dir string) (*Index, error) {
	return BuildOrUpdate(dir)
}

// CachedSession returns a cached session by path if available.
func (idx *Index) CachedSession(path string) (*engine.Session, bool) {
	idx.mu.RLock()
	defer idx.mu.RUnlock()
	s, ok := idx.sessionCache[path]
	return s, ok
}

// Entry returns the index entry for a given path.
func (idx *Index) Entry(path string) *IndexEntry {
	return idx.Entries[path]
}
