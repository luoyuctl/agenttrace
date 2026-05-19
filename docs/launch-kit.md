# agenttrace Launch Kit

agenttrace is a terminal dashboard for AI coding agent session history. It helps developers compare cost, token usage, and elapsed time across Claude Code, Codex CLI, Gemini CLI, Qwen Code, Cline, Aider, Cursor exports, Hermes Agent, OpenCode, OpenClaw, Pi, Oh My Pi, Kimi CLI, Copilot-style logs, and generic JSON/JSONL traces, then diagnose why a task ran slowly.

## Positioning

**One-liner**

Local-first TUI for AI coding agent history: compare cost, tokens, and time across agents, then diagnose slow runs.

**Problem**

AI coding agents behave like tiny build systems: they plan, call tools, retry, hang, and spend money. Most teams only see the final output, not which agent sessions burned the most cost/tokens/time or why one task got stuck.

**Why now**

Agent usage is moving from experiments to daily engineering workflows. Developers need the same kind of local visibility they expect from build tools, test runners, and production telemetry.

## Launch Post

Title ideas:

- Show HN: agenttrace, a TUI for AI coding agent cost, tokens, time, and slow runs
- agenttrace: compare AI agent session cost and diagnose slow tasks
- I built a terminal dashboard for debugging AI coding agent sessions

Body:

I built agenttrace, a single-binary TUI for inspecting AI coding agent sessions locally.

It parses logs from Claude Code, Codex CLI, Gemini CLI, Qwen Code, Cline, Aider, Cursor exports, Hermes Agent, OpenCode, OpenClaw, Pi, Oh My Pi, Kimi CLI, Copilot-style traces, and generic JSON/JSONL traces, then shows:

- historical token, cost, and elapsed-time burn
- agent/source/model breakdowns
- latency, hanging gaps, and slow tool calls
- retry loops, large params, context pressure, and anomalies
- per-session health score for triage
- detail diagnostics and session diffs
- JSON output for dashboards
- CI health gates for average health, critical sessions, and tool failure rate

Install:

```bash
curl -sL https://raw.githubusercontent.com/luoyuctl/agenttrace/master/install.sh | sh
agenttrace
```

Or:

```bash
brew install luoyuctl/tap/agenttrace
```

No local agent logs yet?

```bash
agenttrace --demo
```

The pain point: when an agent gets stuck, retries a tool loop, or silently burns context, the output alone does not tell you what happened. agenttrace gives a quick local view before you dig through raw JSONL logs.

Repo: https://github.com/luoyuctl/agenttrace
Sample HTML report: https://luoyuctl.github.io/agenttrace/demo-report.html

## Short Posts

**X / Threads**

AI coding agents now need session history too.

I built agenttrace: a fast terminal dashboard for local agent sessions.

It shows what multiple agents spent across cost, tokens, and time, then helps explain why a task was slow: hanging gaps, slow tools, retry loops, large params, context pressure, health score, details, diffs, JSON output, and CI gates.

https://github.com/luoyuctl/agenttrace

**Reddit / V2EX**

I made a TUI tool for people using AI coding agents daily. It scans local session logs and shows two things: what multiple agents spent across cost, tokens, and time; and why a specific task was slow, including hanging gaps, slow tools, retry loops, large params, context pressure, and session health.

The goal is not another chat UI. It is closer to `htop`/`lazygit` for AI agent runs: fast local inspection, filtering, diagnostics, and exportable JSON.

Would love feedback from anyone using Claude Code, Codex CLI, Gemini CLI, Qwen Code, Cline, Aider, Cursor exports, Hermes Agent, OpenCode, OpenClaw, Pi, Oh My Pi, Kimi CLI, Copilot-style logs, JSON/JSONL traces, or similar local agent histories.

Repo: https://github.com/luoyuctl/agenttrace

Feedback thread: https://github.com/luoyuctl/agenttrace/discussions/2
Sample report: https://luoyuctl.github.io/agenttrace/demo-report.html

## Target Channels

- Hacker News: Show HN
- Reddit: r/commandline, r/golang, r/LocalLLaMA, r/ClaudeAI, r/ChatGPTCoding
- V2EX: 分享创造 / 程序员
- X / Threads: AI engineering and developer tooling
- GitHub topics: `ai-agents`, `tui`, `observability`, `developer-tools`, `cost-tracking`, `aider`, `claude-code`, `codex-cli`
- Product Hunt: after a GIF demo and first external feedback

## Directory Submissions

Open PRs:

- awesome-codex-cli: https://github.com/RoggeOhta/awesome-codex-cli/pull/23
- awesome-ai-coding-tools: https://github.com/ai-for-developers/awesome-ai-coding-tools/pull/288
- ai-for-developers/awesome-claude: https://github.com/ai-for-developers/awesome-claude/pull/11
- awesome-vibe-coding: https://github.com/ai-for-developers/awesome-vibe-coding/pull/56
- awesome-ai-coding: https://github.com/wsxiaoys/awesome-ai-coding/pull/97
- filipecalegario/awesome-vibe-coding: https://github.com/filipecalegario/awesome-vibe-coding/pull/171
- jiji262/awesome-vibe-coding-tools: https://github.com/jiji262/awesome-vibe-coding-tools/pull/14
- furudo-erika/awesome-vibe-coding-tools: https://github.com/furudo-erika/awesome-vibe-coding-tools/pull/2
- awesome-coding-ai: https://github.com/ohong/awesome-coding-ai/pull/6
- awesome-claude-code-toolkit: https://github.com/rohitg00/awesome-claude-code-toolkit/pull/361
- ComposioHQ/awesome-claude-skills: https://github.com/ComposioHQ/awesome-claude-skills/pull/766
- BehiSecc/awesome-claude-skills: https://github.com/BehiSecc/awesome-claude-skills/pull/280
- jqueryscript/awesome-claude-code: https://github.com/jqueryscript/awesome-claude-code/pull/252
- awesome-go-cli: https://github.com/mantcz/awesome-go-cli/pull/4
- awesome-ai-agents-2026: https://github.com/caramaschiHG/awesome-ai-agents-2026/pull/207
- Awesome-LLMOps: https://github.com/InftyAI/Awesome-LLMOps/pull/420
- awesome-ai: https://github.com/hemanthgk10/awesome-ai/pull/7
- awesome-terminals-ai: https://github.com/BNLNPPS/awesome-terminals-ai/pull/6
- awesome-llmops: https://github.com/KennethanCeyer/awesome-llmops/pull/10
- brandonhimpfen/awesome-llmops: https://github.com/brandonhimpfen/awesome-llmops/pull/4
- onejune2018/Awesome-LLM-Eval: https://github.com/onejune2018/Awesome-LLM-Eval/pull/38
- pauldebdeep9/awesome-agentic-evaluation: https://github.com/pauldebdeep9/awesome-agentic-evaluation/pull/2
- furudo-erika/awesome-ai-testing-tools: https://github.com/furudo-erika/awesome-ai-testing-tools/pull/1
- awesome-harness-engineering: https://github.com/ai-boost/awesome-harness-engineering/pull/14
- walkinglabs/awesome-harness-engineering: https://github.com/walkinglabs/awesome-harness-engineering/pull/26
- AutoJunjie/awesome-agent-harness: https://github.com/AutoJunjie/awesome-agent-harness/pull/18
- mahonzhan/awesome-agent-harness: https://github.com/mahonzhan/awesome-agent-harness/pull/3
- awesome-agentops-landscape: https://github.com/dyronrh/awesome-agentops-landscape/pull/4
- awesome-coding-agent-eval: https://github.com/gudo7208/awesome-coding-agent-eval/pull/1
- kzhou003/awesome-coding-agent-systems: https://github.com/kzhou003/awesome-coding-agent-systems/pull/1
- zjsxply/awesome-coding-agent-tech: https://github.com/zjsxply/awesome-coding-agent-tech/pull/1
- quome-cloud/awesome-coding-agents: https://github.com/quome-cloud/awesome-coding-agents/pull/4
- KarelDO/awesome-codex: https://github.com/KarelDO/awesome-codex/pull/13
- darknorth-123/Awesome-Codex-Plugins: https://github.com/darknorth-123/Awesome-Codex-Plugins/pull/1
- noahfraiture/awesome-codex-plugins: https://github.com/noahfraiture/awesome-codex-plugins/pull/1
- launchapp-dev/awesome-ai-coding-tools: https://github.com/launchapp-dev/awesome-ai-coding-tools/pull/3
- tyler-j-dao/awesome-ai-coding-tools: https://github.com/tyler-j-dao/awesome-ai-coding-tools/pull/2
- SamurAIGPT/awesome-openclaw: https://github.com/SamurAIGPT/awesome-openclaw/pull/125
- alvinreal/awesome-openclaw: https://github.com/alvinreal/awesome-openclaw/pull/34
- hao-ji-xing/awesome-cursor: https://github.com/hao-ji-xing/awesome-cursor/pull/31
- xiaoju111a/awesome-kimi-cli: https://github.com/xiaoju111a/awesome-kimi-cli/pull/1
- lfglabs-dev/awesome-kimi-cli: https://github.com/lfglabs-dev/awesome-kimi-cli/pull/4
- agenmod/awesome-agent-cli: https://github.com/agenmod/awesome-agent-cli/pull/1
- agentablesh/awesome-agent-cli: https://github.com/agentablesh/awesome-agent-cli/pull/1
- shuyhere/awesome-agent-cli: https://github.com/shuyhere/awesome-agent-cli/pull/1
- agenmod/awesome-ai-cli: https://github.com/agenmod/awesome-ai-cli/pull/1
- shawnesquivel/awesome-agent-clis: https://github.com/shawnesquivel/awesome-agent-clis/pull/4
- wdzhwsh4067/awesome-coding-agents: https://github.com/wdzhwsh4067/awesome-coding-agents/pull/1
- tomrzv/Awesome-AI-Coding-Tools: https://github.com/tomrzv/Awesome-AI-Coding-Tools/pull/5
- furudo-erika/awesome-ai-coding-tools: https://github.com/furudo-erika/awesome-ai-coding-tools/pull/5
- dingjiu1989-hue/awesome-ai-coding-tools: https://github.com/dingjiu1989-hue/awesome-ai-coding-tools/pull/1
- yeaight7/awesome-ai-devtools: https://github.com/yeaight7/awesome-ai-devtools/pull/1
- buainoai/awesome-ai-devtools-multilingual: https://github.com/buainoai/awesome-ai-devtools-multilingual/pull/11
- Icloudeng/awesome-ai-coding-tools: https://github.com/Icloudeng/awesome-ai-coding-tools/pull/9
- dremeika/awesome-coding-assistants: https://github.com/dremeika/awesome-coding-assistants/pull/11
- tranhoangpich/awesome-agentic-coding: https://github.com/tranhoangpich/awesome-agentic-coding/pull/1
- Transcenda/awesome-agentic-coding: https://github.com/Transcenda/awesome-agentic-coding/pull/2
- yubing744/awesome-agentic-coding-cli: https://github.com/yubing744/awesome-agentic-coding-cli/pull/1
- brandonhimpfen/awesome-ai-coding-agents: https://github.com/brandonhimpfen/awesome-ai-coding-agents/pull/11
- ashishkaloge/awesome-agentic-engineering: https://github.com/ashishkaloge/awesome-agentic-engineering/pull/1
- rogerchappel/awesome-agentic-engineering: https://github.com/rogerchappel/awesome-agentic-engineering/pull/1
- AFunLS/awesome-ai-agent-tools: https://github.com/AFunLS/awesome-ai-agent-tools/pull/4
- jsnyder/awesome-llm-cli-apps: https://github.com/jsnyder/awesome-llm-cli-apps/pull/1
- o3-cloud/awesome-llm-cli: https://github.com/o3-cloud/awesome-llm-cli/pull/5
- bgl761915-debug/cool-linux-apps: https://github.com/bgl761915-debug/cool-linux-apps/pull/1
- dontriskit/awesome-ai-software-engineering: https://github.com/dontriskit/awesome-ai-software-engineering/pull/7
- yellcamap/awesome-ai-dev-tools: https://github.com/yellcamap/awesome-ai-dev-tools/pull/2
- erkcet/awesome-claude-code: https://github.com/erkcet/awesome-claude-code/pull/3
- shahshrey/awesome-claude-code-mastery: https://github.com/shahshrey/awesome-claude-code-mastery/pull/16
- chendongqi/awesome-ai-coding: https://github.com/chendongqi/awesome-ai-coding/pull/2
- shalk/awesome-ai-coding: https://github.com/shalk/awesome-ai-coding/pull/3
- AnswerZhao/ai-coding-playbook: https://github.com/AnswerZhao/ai-coding-playbook/pull/5
- sam-blackfly/awesome-llm-tools: https://github.com/sam-blackfly/awesome-llm-tools/pull/2
- jordimas/awesome-agentic-engineering: https://github.com/jordimas/awesome-agentic-engineering/pull/2
- spinov001-art/awesome-llm-tools: https://github.com/spinov001-art/awesome-llm-tools/pull/1
- dr-saad-la/awesome-llm-tools: https://github.com/dr-saad-la/awesome-llm-tools/pull/7
- Scottcjn/awesome-agents: https://github.com/Scottcjn/awesome-agents/pull/12
- awesome-cli-apps-in-a-csv follow-up: https://github.com/toolleeo/awesome-cli-apps-in-a-csv/pull/256
- awesome-agent-clis: https://github.com/ComposioHQ/awesome-agent-clis/pull/8
- awesome-code-agents follow-up: https://github.com/sorrycc/awesome-code-agents/pull/22
- tensorchord/Awesome-LLMOps: https://github.com/tensorchord/Awesome-LLMOps/pull/444
- awesome-agent-cortex: https://github.com/0xNyk/awesome-agent-cortex/pull/20
- LangGPT/awesome-claude-code: https://github.com/LangGPT/awesome-claude-code/pull/58
- command-line-tools: https://github.com/linsa-io/command-line-tools/pull/35
- awesome-cli-coding-agents: https://github.com/bradAGI/awesome-cli-coding-agents/pull/73
- awesome-opencode: https://github.com/awesome-opencode/awesome-opencode/pull/334
- awesome-llm-skills: https://github.com/Prat011/awesome-llm-skills/pull/116
- awesome-ai-plugins: https://github.com/hashgraph-online/awesome-ai-plugins/pull/22
- awesome-copilot-agents: https://github.com/Code-and-Sorts/awesome-copilot-agents/pull/53
- awesome-agent-skills: https://github.com/heilcheng/awesome-agent-skills/pull/216
- awesome-ai-eval: https://github.com/Vvkmnn/awesome-ai-eval/pull/10
- skyming/awesome-ai-agent: https://github.com/skyming/awesome-ai-agent/pull/6
- awesome-ai-agent-monitoring: https://github.com/internetbuilder/awesome-ai-agent-monitoring/pull/3
- alexanderop/awesome-ai-coding: https://github.com/alexanderop/awesome-ai-coding/pull/1
- awesome-devtools: https://github.com/devtoolsd/awesome-devtools/pull/213
- awesome-ai-sdks: https://github.com/e2b-dev/awesome-ai-sdks/pull/175
- awesome_ai_agents follow-up: https://github.com/jim-schwoebel/awesome_ai_agents/pull/254
- awesome-ai-devtools follow-up: https://github.com/jamesmurdza/awesome-ai-devtools/pull/495
- Awakehsh/awesome-agent-tools: https://github.com/Awakehsh/awesome-agent-tools/pull/2
- danielrosehill/Awesome-AI-Coding-Tools: https://github.com/danielrosehill/Awesome-AI-Coding-Tools/pull/2
- antigravity-awesome-skills: https://github.com/sickn33/antigravity-awesome-skills/pull/583
- VoltAgent/awesome-agent-skills: https://github.com/VoltAgent/awesome-agent-skills/pull/552
- ComposioHQ/awesome-codex-skills: https://github.com/ComposioHQ/awesome-codex-skills/pull/58
- JackyST0/awesome-agent-skills: https://github.com/JackyST0/awesome-agent-skills/pull/36
- skillmatic-ai/awesome-agent-skills: https://github.com/skillmatic-ai/awesome-agent-skills/pull/78
- xlabs-club/awesome-x-ops: https://github.com/xlabs-club/awesome-x-ops/pull/10
- onurkanbakirci/awesome-codex-automations: https://github.com/onurkanbakirci/awesome-codex-automations/pull/2
- alirezadir/Agentic-AI-Systems: https://github.com/alirezadir/Agentic-AI-Systems/pull/2
- CoderSJX/AI-Resources-Central: https://github.com/CoderSJX/AI-Resources-Central/pull/7
- dinakars777/awesome-tui: https://github.com/dinakars777/awesome-tui/pull/11
- doshibadev/awesome-agentic-devtools: https://github.com/doshibadev/awesome-agentic-devtools/pull/4
- XD3an/awesome-ai-coding-all-in-one: https://github.com/XD3an/awesome-ai-coding-all-in-one/pull/1
- kax168/awesome-ai-coding-2026: https://github.com/kax168/awesome-ai-coding-2026/pull/1
- bluegalaxy111/awesome-vibe-coding: https://github.com/bluegalaxy111/awesome-vibe-coding/pull/4
- hammadhaqqani/awesome-devops-ai: https://github.com/hammadhaqqani/awesome-devops-ai/pull/23
- sorrycc/awesome-code-agents follow-up: https://github.com/sorrycc/awesome-code-agents/pull/23
- eudk/awesome-ai-tools: https://github.com/eudk/awesome-ai-tools/pull/242
- scortt/awesome-ai-dev-tools: https://github.com/scortt/awesome-ai-dev-tools/pull/1
- kax168/awesome-ai-dev-tools-2026: https://github.com/kax168/awesome-ai-dev-tools-2026/pull/3
- kax168/awesome-ai-coding-tools-2026: https://github.com/kax168/awesome-ai-coding-tools-2026/pull/3
- kax168/awesome-ai-coding-agents: https://github.com/kax168/awesome-ai-coding-agents/pull/3
- claudexia-api/awesome-claude-tools: https://github.com/claudexia-api/awesome-claude-tools/pull/1
- zjh1943/awesome-claude-code: https://github.com/zjh1943/awesome-claude-code/pull/44
- gaborsoter/awesome-ai-dev-productivity: https://github.com/gaborsoter/awesome-ai-dev-productivity/pull/2
- saviorand/awesome-ai-assisted-coding: https://github.com/saviorand/awesome-ai-assisted-coding/pull/4
- karanb192/awesome-claude-skills: https://github.com/karanb192/awesome-claude-skills/pull/75
- libukai/awesome-agent-skills: https://github.com/libukai/awesome-agent-skills/pull/54
- kodustech/awesome-agent-skills: https://github.com/kodustech/awesome-agent-skills/pull/15
- Chat2AnyLLM/awesome-repo-configs: https://github.com/Chat2AnyLLM/awesome-repo-configs/pull/14
- philipbankier/awesome-agent-skills: https://github.com/philipbankier/awesome-agent-skills/pull/13
- kenryu42/awesome-claude-skills: https://github.com/kenryu42/awesome-claude-skills/pull/11
- sandipan1/awesome-claude-skills: https://github.com/sandipan1/awesome-claude-skills/pull/7
- yibie/Awesome-Claude-Skills: https://github.com/yibie/Awesome-Claude-Skills/pull/7
- coderPerseus/awesome-cli-tools-for-agents: https://github.com/coderPerseus/awesome-cli-tools-for-agents/pull/1
- danielrosehill/Awesome-AI-Evaluations-Tools: https://github.com/danielrosehill/Awesome-AI-Evaluations-Tools/pull/3
- hparreao/Awesome-AI-Evaluation-Guide: https://github.com/hparreao/Awesome-AI-Evaluation-Guide/pull/2
- jakemeany523/awesome-llm-evaluation: https://github.com/jakemeany523/awesome-llm-evaluation/pull/1
- AGBAJEMUH/Awesome-AI-Evaluation-Guide: https://github.com/AGBAJEMUH/Awesome-AI-Evaluation-Guide/pull/1
- priyathamkat/Awesome-LLM-Evaluation: https://github.com/priyathamkat/Awesome-LLM-Evaluation/pull/1
- c1505/Awesome-LLM-Evaluations: https://github.com/c1505/Awesome-LLM-Evaluations/pull/1
- itsderek23/awesome-eval-driven-development: https://github.com/itsderek23/awesome-eval-driven-development/pull/2
- chaosync-org/awesome-ai-agent-testing: https://github.com/chaosync-org/awesome-ai-agent-testing/pull/6
- ankitvirdi4/awesome-llm-cost: https://github.com/ankitvirdi4/awesome-llm-cost/pull/2
- ravsau/awesome-ai-cost-optimization: https://github.com/ravsau/awesome-ai-cost-optimization/pull/2
- u4ma-kev/awesome-ai-agent-cost-control: https://github.com/u4ma-kev/awesome-ai-agent-cost-control/pull/2
- sjakati98/awesome-tools-for-agents: https://github.com/sjakati98/awesome-tools-for-agents/pull/1
- moshehbenavraham/Ultimate-Agent-Directory: https://github.com/moshehbenavraham/Ultimate-Agent-Directory/pull/79
- KalyanKS-NLP/llm-engineer-toolkit: https://github.com/KalyanKS-NLP/llm-engineer-toolkit/pull/27
- Sumanth077/ai-engineering-toolkit: https://github.com/Sumanth077/ai-engineering-toolkit/pull/18
- a16z-infra/llm-app-stack: https://github.com/a16z-infra/llm-app-stack/pull/54
- ankurkumarz/agentic-ai-knowledge-base: https://github.com/ankurkumarz/agentic-ai-knowledge-base/pull/1
- mahseema/awesome-ai-tools: https://github.com/mahseema/awesome-ai-tools/pull/1287
- anthropics/claude-code-monitoring-guide: https://github.com/anthropics/claude-code-monitoring-guide/pull/16
- goabiaryan/awesome-observability: https://github.com/goabiaryan/awesome-observability/pull/4
- boxabirds/awesome-ai-engineering: https://github.com/boxabirds/awesome-ai-engineering/pull/2
- Guidely-org/awesome-ai-engineering: https://github.com/Guidely-org/awesome-ai-engineering/pull/1
- cola-runner/awesome-tui-design: https://github.com/cola-runner/awesome-tui-design/pull/1
- phmullins/awesome-macos-commandline: https://github.com/phmullins/awesome-macos-commandline/pull/12
- saehun/awesome-terminal: https://github.com/saehun/awesome-terminal/pull/4
- closedloop-technologies/awesome-coding-agents: https://github.com/closedloop-technologies/awesome-coding-agents/pull/3
- kax168/awesome-ai-coding-assistants-2026: https://github.com/kax168/awesome-ai-coding-assistants-2026/pull/3
- vaderyang/awesome-openai-codex: https://github.com/vaderyang/awesome-openai-codex/pull/1
- taahro/awesome-openai-codex-cli: https://github.com/taahro/awesome-openai-codex-cli/pull/2
- dtunai/awesome-gemini-cli: https://github.com/dtunai/awesome-gemini-cli/pull/4
- pantheon-org/awesome-opencode: submitted required tool suggestion https://github.com/pantheon-org/awesome-opencode/issues/12
- yiancode/AwesomeClaudeCode: submitted resource suggestion https://github.com/yiancode/AwesomeClaudeCode/issues/142
- simonpierreboucher02/awesome-claude-code: https://github.com/simonpierreboucher02/awesome-claude-code/pull/1
- itgoyo/awesome-claude-code: https://github.com/itgoyo/awesome-claude-code/pull/1
- spinov001-art/awesome-cli-tools-2026: https://github.com/spinov001-art/awesome-cli-tools-2026/pull/1
- Siilwyn/awesome-cli-tools: https://github.com/Siilwyn/awesome-cli-tools/pull/19
- CloudAI-X/claude-code-resources: https://github.com/CloudAI-X/claude-code-resources/pull/12
- abordage/awesome-ai: https://github.com/abordage/awesome-ai/pull/4
- qualisero/awesome-pi-agent: https://github.com/qualisero/awesome-pi-agent/pull/54
- FlorianBruniaux/claude-code-ultimate-guide: https://github.com/FlorianBruniaux/claude-code-ultimate-guide/pull/27
- analyticalrohit/awesome-vibe-coding-guide: https://github.com/analyticalrohit/awesome-vibe-coding-guide/pull/25
- vanna-ai/Awesome-Vibe-Coding-CLI: https://github.com/vanna-ai/Awesome-Vibe-Coding-CLI/pull/5
- no-fluff/awesome-vibe-coding: submitted required tool proposal https://github.com/no-fluff/awesome-vibe-coding/issues/107

Merged listings:

- tugkanboz/awesome-ai-testing: https://github.com/tugkanboz/awesome-ai-testing/pull/9
- awesome-ChatGPT-repositories: https://github.com/taishi-i/awesome-ChatGPT-repositories/pull/130
- GetBindu/awesome-claude-code-and-skills: https://github.com/GetBindu/awesome-claude-code-and-skills/pull/21
- awesome-gemini-cli: https://github.com/Piebald-AI/awesome-gemini-cli/pull/47
- milisp/awesome-codex-cli: https://github.com/milisp/awesome-codex-cli/pull/20
- hashgraph-online/awesome-codex-plugins follow-up: https://github.com/hashgraph-online/awesome-codex-plugins/pull/68
- awesome-mac: https://github.com/jaywcjlove/awesome-mac/pull/2026
- awesome-skills: https://github.com/gmh5225/awesome-skills/pull/14
- charm-in-the-wild: https://github.com/charm-and-friends/charm-in-the-wild/pull/88
- gobuild/awesome-go-tools: https://github.com/gobuild/awesome-go-tools/pull/6
- acvnace/awesome-vibe-coding-resources: https://github.com/acvnace/awesome-vibe-coding-resources/pull/12
- ARUNAGIRINATHAN-K/awesome-ai-agents-2026: https://github.com/ARUNAGIRINATHAN-K/awesome-ai-agents-2026/pull/27
- rohitg00/awesome-openclaw: https://github.com/rohitg00/awesome-openclaw/pull/139
- 0xWelt/Awesome-Vibe-Coding: https://github.com/0xWelt/Awesome-Vibe-Coding/pull/152
- alvinreal/awesome-opensource-ai: https://github.com/alvinreal/awesome-opensource-ai/pull/418
- Jenqyang/Awesome-AI-Agents: https://github.com/Jenqyang/Awesome-AI-Agents/pull/204
- lgaggini/awesome-cli-tui-software: https://github.com/lgaggini/awesome-cli-tui-software/pull/3
- eltociear/awesome-AI-driven-development: https://github.com/eltociear/awesome-AI-driven-development/pull/48
- NipunaRanasinghe/awesome-ai-agents: https://github.com/NipunaRanasinghe/awesome-ai-agents/pull/91
- PierrunoYT/awesome-ai-dev-tools: https://github.com/PierrunoYT/awesome-ai-dev-tools/pull/20
- adriannovegil/awesome-observability follow-up: https://github.com/adriannovegil/awesome-observability/pull/64
- pegaltier/awesome-utils-dev: https://github.com/pegaltier/awesome-utils-dev/pull/29
- llm-toolkit: https://github.com/sumanth-dhanya/llm-toolkit/pull/1
- Picrew/awesome-agent-harness: listing present on upstream main; closed stale conflicting PR https://github.com/Picrew/awesome-agent-harness/pull/5.

Manual-only submission:

- hesreallyhim/awesome-claude-code: submit via the GitHub issue form, because the repo asks contributors not to create automated issues or PRs. Suggested category: Tooling / Usage Monitors.
- e2b-dev/awesome-ai-agents: submit through the Google Form linked from the README; the repo asks for product submissions through the form instead of direct README edits.
- awesome-claude-skills: skip automated PRs unless submitted manually by a human; its contribution guide asks that PRs are not AI-assisted and generally expects social proof.
- awesome-go: defer until the project is older and has the required quality links; contribution checks expect repository maturity, pkg.go.dev, Go Report Card, and coverage evidence.
- awesome-cli-apps: PR https://github.com/agarrharr/awesome-cli-apps/pull/1032 was closed without maintainer feedback. Revisit after more external adoption or a clearer category fit.
- awesome-tuis: blocked until the repo is at least 6 months old; its PR template requires repos to be at least 6 months old, and follow-up PR https://github.com/rothgar/awesome-tuis/pull/659 was closed after reviewer feedback.
- Terminal Trove: submit through https://terminaltrove.com/post/ after confirming the author contact email. Suggested categories: `macos`, `linux`, `windows`, `monitoring`, `observability`, `tui`, `json`, `ai`, `cli`, `debugging`, `cross-platform`. Preview PNG: `https://luoyuctl.github.io/agenttrace/assets/readme-real-overview.png`; GIF: `https://luoyuctl.github.io/agenttrace/assets/agenttrace-demo.gif`.
- Terminal Apps: submitted suggestion issue https://github.com/scmmishra/terminal-apps.dev/issues/55. Name: `agenttrace`; GitHub URL: `https://github.com/luoyuctl/agenttrace`.
- awesome-ai-coding-techniques: submitted technique suggestion https://github.com/inmve/awesome-ai-coding-techniques/issues/37. Suggested technique: inspect AI agent session traces after a run. Followed up on semantic-drift feedback in https://github.com/inmve/awesome-ai-coding-techniques/issues/37#issuecomment-4414284882.
- awesome-hermes-agent: submitted resource recommendation issue https://github.com/0xNyk/awesome-hermes-agent/issues/67. Suggested category: agentskills.io Ecosystem or Tools & Utilities.
- vincentkoc/awesome-openclaw: submitted required pre-PR resource request https://github.com/vincentkoc/awesome-openclaw/issues/82. Suggested section: Developer Tooling and Observability.
- agent-gigmole/awesome-ai-agent-tools: submitted tool suggestion https://github.com/agent-gigmole/awesome-ai-agent-tools/issues/2. Suggested category: Observability & Evaluation.
- InftyAI/Awesome-LLMOps: closed duplicate PR https://github.com/InftyAI/Awesome-LLMOps/pull/418 in favor of workflow-generated PR https://github.com/InftyAI/Awesome-LLMOps/pull/420.
- kyrolabs/awesome-agents: closed duplicate PR https://github.com/kyrolabs/awesome-agents/pull/437; follow-up PR https://github.com/kyrolabs/awesome-agents/pull/440 was also closed.
- jamesmurdza/awesome-ai-devtools: closed duplicate PR https://github.com/jamesmurdza/awesome-ai-devtools/pull/492 in favor of follow-up PR https://github.com/jamesmurdza/awesome-ai-devtools/pull/495.
- jim-schwoebel/awesome_ai_agents: closed duplicate PR https://github.com/jim-schwoebel/awesome_ai_agents/pull/250 in favor of follow-up PR https://github.com/jim-schwoebel/awesome_ai_agents/pull/254.
- github/awesome-copilot: PR https://github.com/github/awesome-copilot/pull/1595 was closed as a low-quality automated submission; do not resubmit without a narrower, manual-quality angle.

Terminal Trove draft:

- Name: `agenttrace`
- URL: `github.com/luoyuctl/agenttrace`
- Tagline: `Local-first TUI for AI coding agent cost, tokens, time, and slow-run diagnosis.`
- Description: `agenttrace parses local Claude Code, Codex CLI, Gemini CLI, Qwen Code, Cline, Aider, Cursor exports, Hermes Agent, OpenCode, OpenClaw, Pi, Oh My Pi, Kimi CLI, Copilot-style logs, and JSON/JSONL traces into a fast terminal dashboard for comparing historical session cost, token usage, and elapsed time, then diagnosing slow tasks.`
- Standout features: `Overview, session list, detail, diagnostics, and diff views; incremental local cache; slow-run evidence for long gaps, hanging sessions, slow tools, large params, and context pressure; JSON, Markdown, and self-contained HTML reports.`
- Who it is for: `Developers using multiple AI coding agents who need to find expensive or slow sessions without uploading private logs to a hosted service.`
- Primary language: `go`
- License: `mit`
- Install:
  - macOS/Linux: `curl -sL https://raw.githubusercontent.com/luoyuctl/agenttrace/master/install.sh | sh`
  - Homebrew: `brew install luoyuctl/tap/agenttrace`
  - Go install: `go install github.com/luoyuctl/agenttrace/cmd/agenttrace@latest`
  - Windows PowerShell: `iwr -useb https://raw.githubusercontent.com/luoyuctl/agenttrace/master/install.ps1 | iex`

## Demo Checklist

- Render `assets/agenttrace-demo.gif` with `scripts/record-demo.sh` when VHS is available.
- First screen should show the Overview dashboard.
- Include Session List filtering and command mode.
- Show Detail with primary issue and scroll percentage.
- Show Diagnostics for hanging/tool failures/context usage.
- Show Diff between two sessions.
- End with `agenttrace --overview -f json`.
- Show CI gate output with `agenttrace --overview --fail-under-health 80 --fail-on-critical`.
- For a reproducible recording, use `agenttrace --demo`.

See [demo-playbook.md](demo-playbook.md) for the recording script and storyline.

## Verification Before Sharing

```bash
go test ./...
go build -o /tmp/agenttrace ./cmd/agenttrace
/tmp/agenttrace --version
/tmp/agenttrace --demo --overview -f json
```

For public demo, report, or release-surface checks, run the reusable gates from [CI Integration](ci-integration.md):

```bash
AGENTTRACE_BIN=/tmp/agenttrace scripts/ci/check-output-contract.sh
AGENTTRACE_BIN=/tmp/agenttrace scripts/ci/check-deterministic-output.sh
AGENTTRACE_BIN=/tmp/agenttrace scripts/ci/check-report-semantics.sh
scripts/ci/check-release-surfaces.sh
scripts/ci/check-pages-artifact.sh site
```

## Release Consistency Checklist

Before sharing a release publicly, compare these surfaces against `gh release list --repo luoyuctl/agenttrace --limit 1`:

- README release and Homebrew badges point at the latest version.
- `homebrew/Formula/agenttrace.rb` and `homebrew/README.md` match the current install story.
- `site/index.html` JSON-LD `softwareVersion`, `site/demo-report.html`, `site/llms.txt`, `site/robots.txt`, and `site/sitemap.xml` remain present and version-consistent where they mention a release.
- GitHub Discussions, release notes, and launch copy do not point readers at stale release links.
- Public CTAs use neutral product actions such as `Get agenttrace`, `Install`, or `Latest release`.

Install smoke:

```bash
tmp_home=$(mktemp -d)
AGENTTRACE_INSTALL_DIR="$tmp_home/bin" HOME="$tmp_home" sh install.sh
"$tmp_home/bin/agenttrace" --version
rm -rf "$tmp_home"
```
