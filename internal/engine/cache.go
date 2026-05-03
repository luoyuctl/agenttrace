package engine

import (
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"sort"
	"time"
)

// ── Session file cache ──────────────────────────────────────────
//
// Caches full Session structs keyed by absolute file path.
// mtime + size are used as cache keys — session files are write-once
// so this is sufficient. Stored as a single JSON file under
// the OS cache directory (~/Library/Caches/agenttrace/sessions.json).

// SessionCache holds all cached session entries.
type SessionCache struct {
	Entries map[string]CacheEntry    `json:"entries"`
	Dirs    map[string]DirCacheEntry `json:"dirs,omitempty"`

	rawEntries map[string]json.RawMessage
}

// CacheEntry pairs a session file's mtime+size with its parsed Session.
type CacheEntry struct {
	ModTime int64   `json:"mod_time"`
	Size    int64   `json:"size"`
	Session Session `json:"session"`
}

// DirCacheEntry stores one directory listing so startup can stat directories
// and only re-read directories whose mtime changed.
type DirCacheEntry struct {
	ModTime int64    `json:"mod_time"`
	Files   []string `json:"files"`
	Dirs    []string `json:"dirs"`
}

func sessionCachePath() string {
	dir := os.Getenv("AGENTTRACE_SESSION_CACHE_DIR")
	if dir == "" {
		dir, _ = os.UserCacheDir()
		if dir == "" {
			dir = os.TempDir()
		}
		dir = filepath.Join(dir, "agenttrace")
	}
	os.MkdirAll(dir, 0755)
	return filepath.Join(dir, "sessions.json")
}

// SessionCachePath returns the on-disk session cache path.
func SessionCachePath() string {
	return sessionCachePath()
}

func emptySessionCache() SessionCache {
	return SessionCache{
		Entries:    make(map[string]CacheEntry),
		Dirs:       make(map[string]DirCacheEntry),
		rawEntries: make(map[string]json.RawMessage),
	}
}

// EntryCount 返回缓存文件中的唯一会话条目数。
func (sc SessionCache) EntryCount() int {
	if len(sc.rawEntries) == 0 {
		return len(sc.Entries)
	}
	seen := make(map[string]struct{}, len(sc.rawEntries)+len(sc.Entries))
	for path := range sc.rawEntries {
		seen[path] = struct{}{}
	}
	for path := range sc.Entries {
		seen[path] = struct{}{}
	}
	return len(seen)
}

// LoadSessionCache 从磁盘读取会话缓存，完整 Session 会按需解码。
func LoadSessionCache() SessionCache {
	data, err := os.ReadFile(sessionCachePath())
	if err != nil {
		return emptySessionCache()
	}

	var raw struct {
		Entries map[string]json.RawMessage `json:"entries"`
		Dirs    map[string]json.RawMessage `json:"dirs"`
	}
	if err := json.Unmarshal(data, &raw); err != nil {
		return emptySessionCache()
	}

	sc := emptySessionCache()
	for path, msg := range raw.Entries {
		if _, ok := decodeCacheEntryHeader(msg); !ok {
			continue
		}
		sc.rawEntries[path] = append(json.RawMessage(nil), msg...)
	}
	for path, msg := range raw.Dirs {
		var entry DirCacheEntry
		if err := json.Unmarshal(msg, &entry); err != nil {
			continue
		}
		sc.Dirs[path] = entry
	}
	return sc
}

// SaveSessionCache atomically writes the session cache to disk.
func SaveSessionCache(sc SessionCache) error {
	raw := struct {
		Entries map[string]json.RawMessage `json:"entries"`
		Dirs    map[string]DirCacheEntry   `json:"dirs,omitempty"`
	}{
		Entries: make(map[string]json.RawMessage, len(sc.rawEntries)+len(sc.Entries)),
		Dirs:    sc.Dirs,
	}
	for path, msg := range sc.rawEntries {
		raw.Entries[path] = append(json.RawMessage(nil), msg...)
	}
	for path, entry := range sc.Entries {
		msg, err := json.Marshal(entry)
		if err != nil {
			return err
		}
		raw.Entries[path] = msg
	}

	data, err := json.Marshal(raw)
	if err != nil {
		return err
	}
	tmp := sessionCachePath() + ".tmp"
	if err := os.WriteFile(tmp, data, 0644); err != nil {
		return err
	}
	return os.Rename(tmp, sessionCachePath())
}

// ClearSessionCache removes the session cache file.
func ClearSessionCache() error {
	err := os.Remove(sessionCachePath())
	if errors.Is(err, os.ErrNotExist) {
		return nil
	}
	return err
}

// CachedSession returns a parsed session only when file metadata still matches.
func CachedSession(path string, cache SessionCache) (Session, bool) {
	entry, ok := cachedEntry(path, cache)
	if !ok {
		return Session{}, false
	}
	info, err := os.Stat(path)
	if err != nil {
		deleteCachedSession(cache, path)
		return Session{}, false
	}
	if entry.ModTime != info.ModTime().UnixNano() || entry.Size != info.Size() {
		deleteCachedSession(cache, path)
		return Session{}, false
	}
	s := entry.Session
	s.Path = path
	return LocalizeSession(s), true
}

// FindSessionFilesCached discovers session files using cached directory
// listings. It still checks directory mtimes so newly written sessions are
// picked up incrementally, but unchanged directories do not need ReadDir.
func FindSessionFilesCached(dir string, cache SessionCache) []string {
	if cache.Entries == nil {
		cache.Entries = make(map[string]CacheEntry)
	}
	if cache.Dirs == nil {
		cache.Dirs = make(map[string]DirCacheEntry)
	}
	if cache.rawEntries == nil {
		cache.rawEntries = make(map[string]json.RawMessage)
	}
	if isClineTaskDir(dir) {
		return []string{dir}
	}

	var roots []string
	if dir == "" {
		roots = DiscoverSessionDirs()
	} else {
		roots = []string{dir}
	}

	seen := make(map[string]bool)
	var files []string
	for _, root := range roots {
		maxDepth := maxSessionDirDepth(root)
		for _, f := range collectSessionFilesCached(root, 0, maxDepth, cache) {
			if !seen[f] {
				seen[f] = true
				files = append(files, f)
			}
		}
	}
	return sortFilesByCache(files, cache)
}

func collectSessionFilesCached(dir string, depth, maxDepth int, cache SessionCache) []string {
	if depth > maxDepth {
		return nil
	}
	if isClineTaskDir(dir) {
		return []string{dir}
	}
	info, err := os.Stat(dir)
	if err != nil || !info.IsDir() {
		return nil
	}
	mtime := info.ModTime().UnixNano()

	entry, ok := cache.Dirs[dir]
	if ok && entry.ModTime == mtime {
		var out []string
		out = append(out, entry.Files...)
		for _, child := range entry.Dirs {
			out = append(out, collectSessionFilesCached(child, depth+1, maxDepth, cache)...)
		}
		return out
	}

	entries, err := os.ReadDir(dir)
	if err != nil {
		return nil
	}
	var files []string
	var dirs []string
	for _, e := range entries {
		path := filepath.Join(dir, e.Name())
		if e.IsDir() {
			if isSkippedSessionDir(path) {
				continue
			}
			if isClineTaskDir(path) {
				files = append(files, path)
				continue
			}
			dirs = append(dirs, path)
			continue
		}
		name := e.Name()
		if !isSessionFileName(name) {
			continue
		}
		if isGeminiTempPath(path) && !isGeminiTempSessionFile(path) {
			continue
		}
		if isOpenCodeStoragePath(path) && !isOpenCodeStorageSessionFile(path) {
			continue
		}
		files = append(files, path)
	}
	sort.Strings(files)
	sort.Strings(dirs)
	cache.Dirs[dir] = DirCacheEntry{ModTime: mtime, Files: files, Dirs: dirs}

	out := append([]string{}, files...)
	for _, child := range dirs {
		out = append(out, collectSessionFilesCached(child, depth+1, maxDepth, cache)...)
	}
	return out
}

func sortFilesByCache(paths []string, cache SessionCache) []string {
	type item struct {
		path string
		t    time.Time
	}
	items := make([]item, 0, len(paths))
	for _, p := range paths {
		info, err := os.Stat(p)
		if err != nil {
			deleteCachedSession(cache, p)
			continue
		}
		if entry, ok := cachedEntryHeader(p, cache); ok {
			if entry.ModTime == info.ModTime().UnixNano() && entry.Size == info.Size() {
				items = append(items, item{path: p, t: time.Unix(0, entry.ModTime)})
				continue
			}
			items = append(items, item{path: p, t: info.ModTime()})
			continue
		}
		items = append(items, item{path: p, t: info.ModTime()})
	}
	sort.Slice(items, func(i, j int) bool {
		return items[i].t.After(items[j].t)
	})
	out := make([]string, len(items))
	for i, it := range items {
		out[i] = it.path
	}
	return out
}

func cachedEntry(path string, cache SessionCache) (CacheEntry, bool) {
	if cache.Entries == nil {
		cache.Entries = make(map[string]CacheEntry)
	}
	if entry, ok := cache.Entries[path]; ok {
		return entry, true
	}
	raw, ok := cache.rawEntries[path]
	if !ok {
		return CacheEntry{}, false
	}
	var entry CacheEntry
	if err := json.Unmarshal(raw, &entry); err != nil {
		deleteCachedSession(cache, path)
		return CacheEntry{}, false
	}
	cache.Entries[path] = entry
	return entry, true
}

func cachedEntryHeader(path string, cache SessionCache) (CacheEntry, bool) {
	if entry, ok := cache.Entries[path]; ok {
		return entry, true
	}
	raw, ok := cache.rawEntries[path]
	if !ok {
		return CacheEntry{}, false
	}
	entry, ok := decodeCacheEntryHeader(raw)
	if !ok {
		deleteCachedSession(cache, path)
		return CacheEntry{}, false
	}
	return entry, true
}

func decodeCacheEntryHeader(raw json.RawMessage) (CacheEntry, bool) {
	var header struct {
		ModTime int64 `json:"mod_time"`
		Size    int64 `json:"size"`
	}
	if err := json.Unmarshal(raw, &header); err != nil {
		return CacheEntry{}, false
	}
	return CacheEntry{ModTime: header.ModTime, Size: header.Size}, true
}

func deleteCachedSession(cache SessionCache, path string) {
	delete(cache.Entries, path)
	delete(cache.rawEntries, path)
}
