package engine

import (
	"path/filepath"
	"strings"
)

func isGeminiWrapper(doc map[string]interface{}) bool {
	if doc == nil {
		return false
	}
	if _, ok := doc["contents"]; ok {
		return true
	}
	if _, ok := doc["candidates"]; ok {
		return true
	}
	for _, key := range []string{"history", "messages", "conversation", "clientHistory", "chatHistory"} {
		if arr, ok := doc[key].([]interface{}); ok && isGeminiContentArray(arr) {
			return true
		}
	}
	for _, key := range []string{"checkpoint", "session", "chat"} {
		if nested, ok := doc[key].(map[string]interface{}); ok && isGeminiWrapper(nested) {
			return true
		}
	}
	return false
}

func isGeminiContentArray(arr []interface{}) bool {
	for _, item := range arr {
		if m, ok := item.(map[string]interface{}); ok && isGeminiContentObject(m) {
			return true
		}
	}
	return false
}

func isGeminiContentObject(obj map[string]interface{}) bool {
	if _, ok := obj["parts"].([]interface{}); !ok {
		return false
	}
	role, _ := obj["role"].(string)
	return role == "user" || role == "model" || role == "assistant" || role == "tool"
}

func isGeminiTempSessionFile(path string) bool {
	clean := filepath.Clean(path)
	if !isGeminiTempPath(clean) {
		return false
	}
	parent := filepath.Base(filepath.Dir(clean))
	return parent == "chats" || parent == "checkpoints"
}

func isGeminiTempPath(path string) bool {
	return strings.Contains(filepath.ToSlash(path), "/.gemini/tmp/")
}

func isSkippedSessionDir(path string) bool {
	name := filepath.Base(path)
	if name == ".git" || name == "node_modules" {
		return true
	}
	if isGeminiTempPath(path) && strings.HasPrefix(name, ".") {
		return true
	}
	if isOpenCodeStorageSkippedDir(path) {
		return true
	}
	return false
}

func geminiRole(role string) string {
	switch role {
	case "model", "assistant":
		return "assistant"
	case "tool":
		return "tool"
	default:
		return role
	}
}
