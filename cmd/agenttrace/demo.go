package main

import (
	"os"
	"path/filepath"
)

func writeDemoSessions() (string, func(), error) {
	dir, err := os.MkdirTemp("", "agenttrace-demo-*")
	if err != nil {
		return "", nil, err
	}
	cleanup := func() { _ = os.RemoveAll(dir) }
	files := map[string]string{
		"01-healthy-ship.jsonl": demoHealthySession,
		"02-hanging-loop.jsonl": demoHangingSession,
		"03-shallow-chat.jsonl": demoShallowSession,
	}
	for name, content := range files {
		if err := os.WriteFile(filepath.Join(dir, name), []byte(content), 0644); err != nil {
			cleanup()
			return "", nil, err
		}
	}
	return dir, cleanup, nil
}

const demoHealthySession = `{"role":"session_meta","timestamp":"2026-05-02T10:00:00Z","ModelUsed":"claude-sonnet-4"}
{"role":"meta","ModelUsed":"claude-sonnet-4","Usage":{"input_tokens":4200,"output_tokens":1800,"cache_read_input_tokens":9000}}
{"role":"user","content":"Implement the billing export endpoint and keep the patch small.","timestamp":"2026-05-02T10:00:00Z","ModelUsed":"claude-sonnet-4"}
{"role":"assistant","content":"I will inspect the API shape, add the endpoint, and run the focused tests.","timestamp":"2026-05-02T10:00:04Z","reasoning":"Plan: locate the billing router, reuse the existing CSV serializer, add one route and a focused regression test. Risk is around permissions, so verify auth middleware before editing. Keep the patch narrow: route registration, handler wiring, and one test fixture. After implementation, run only the billing package tests, then a final package-wide smoke to catch integration fallout. If the serializer already handles date ranges, avoid inventing another abstraction and pass through the existing query parser.","tool_calls":[{"id":"t1","name":"rg","args":"billing export"}],"ModelUsed":"claude-sonnet-4"}
{"role":"tool","content":"{\"success\":true,\"matches\":4}","tool_call_id":"t1","timestamp":"2026-05-02T10:00:05Z"}
{"role":"assistant","content":"Endpoint implemented and tests pass.","timestamp":"2026-05-02T10:00:18Z","reasoning":"The route can share the existing report serializer. Permission middleware is already mounted at the group level, so the change stays narrow. The test should assert status, content type, and a representative CSV row instead of snapshotting the full response. That keeps the contract stable without making unrelated fixture changes noisy. I also checked the empty result path because export endpoints often fail there; the existing serializer already emits the header row, so no special case is needed. The final patch is intentionally boring: one route, one handler branch, one regression test, and no new dependency.","tool_calls":[{"id":"t2","name":"go_test","args":"./internal/billing"}],"ModelUsed":"claude-sonnet-4"}
{"role":"tool","content":"{\"success\":true,\"passed\":32}","tool_call_id":"t2","timestamp":"2026-05-02T10:00:24Z"}
`

const demoHangingSession = `{"role":"session_meta","timestamp":"2026-05-02T10:10:00Z","ModelUsed":"gpt-5.1"}
{"role":"meta","ModelUsed":"gpt-5.1","Usage":{"input_tokens":185000,"output_tokens":48000}}
{"role":"user","content":"Refactor the flaky websocket reconnect flow.","timestamp":"2026-05-02T10:10:00Z","ModelUsed":"gpt-5.1"}
{"role":"assistant","content":"I am going to reproduce the reconnect failure first.","timestamp":"2026-05-02T10:10:02Z","reasoning":"Need a focused repro before changing reconnect backoff. Start with existing websocket tests, then inspect the retry path.","tool_calls":[{"id":"t1","name":"terminal","args":"go test ./internal/ws -run Reconnect"}],"ModelUsed":"gpt-5.1"}
{"role":"tool","content":"{\"error\":\"test timed out after 10m\"}","tool_call_id":"t1","is_error":true,"timestamp":"2026-05-02T10:18:24Z"}
{"role":"assistant","content":"The test is hanging. I will inspect goroutine cleanup and retry cancellation.","timestamp":"2026-05-02T10:18:30Z","reasoning":"A long gap and timeout suggest the retry loop ignores context cancellation. Check for missing select on ctx.Done.","tool_calls":[{"id":"t2","name":"rg","args":"Reconnect|ctx.Done"}],"ModelUsed":"gpt-5.1"}
{"role":"tool","content":"{\"success\":false,\"error\":\"rg returned no cancellation branch in retry loop\"}","tool_call_id":"t2","is_error":true,"timestamp":"2026-05-02T10:19:05Z"}
{"role":"assistant","content":"Added cancellation, but the suite still flakes on shutdown.","timestamp":"2026-05-02T10:26:40Z","reasoning":"The first fix was incomplete. The reconnect worker also needs to close the pending timer before returning.","tool_calls":[{"id":"t3","name":"go_test","args":"./internal/ws"}],"ModelUsed":"gpt-5.1"}
{"role":"tool","content":"{\"success\":false,\"error\":\"1 failed, 2 timed out\"}","tool_call_id":"t3","is_error":true,"timestamp":"2026-05-02T10:28:10Z"}
`

const demoShallowSession = `{"role":"session_meta","timestamp":"2026-05-02T10:35:00Z","ModelUsed":"gemini-2.5-pro"}
{"role":"meta","ModelUsed":"gemini-2.5-pro","Usage":{"input_tokens":28000,"output_tokens":2200}}
{"role":"user","content":"Can you review this migration plan?","timestamp":"2026-05-02T10:35:00Z","ModelUsed":"gemini-2.5-pro"}
{"role":"assistant","content":"Looks good overall.","timestamp":"2026-05-02T10:35:02Z","reasoning":"Seems fine.","ModelUsed":"gemini-2.5-pro"}
{"role":"assistant","content":"I would ship it as-is.","timestamp":"2026-05-02T10:35:18Z","reasoning":"No blockers.","ModelUsed":"gemini-2.5-pro"}
{"role":"assistant","content":"Maybe add a rollback note.","timestamp":"2026-05-02T10:35:40Z","reasoning":"Small note.","ModelUsed":"gemini-2.5-pro"}
`
