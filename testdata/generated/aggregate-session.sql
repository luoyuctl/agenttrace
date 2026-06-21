CREATE TABLE sessions (id TEXT, model TEXT, started_at REAL, ended_at REAL, message_count INTEGER, tool_call_count INTEGER, input_tokens INTEGER, output_tokens INTEGER, cache_read_tokens INTEGER, cache_write_tokens INTEGER, cwd TEXT);
CREATE TABLE messages (session_id TEXT, role TEXT);
INSERT INTO sessions VALUES ('aggregate','gpt-5',1735689600,1735689660,2,1,100,20,0,0,'/tmp/project');
INSERT INTO sessions VALUES ('limited',NULL,0,0,0,0,0,0,0,0,'');
INSERT INTO messages VALUES ('aggregate','user');
INSERT INTO messages VALUES ('aggregate','assistant');
