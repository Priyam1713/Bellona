# Examples

Runnable recipes. Each one is copy-paste-able and uses free local models
where a model is needed at all.

| File | What you'll see |
|------|-----------------|
| [ollama-fix-bug.md](ollama-fix-bug.md) | Bellona reads a repo, fixes a failing test, commits — fully offline |
| [telegram-bot.md](telegram-bot.md) | Your agent answering campaigns from a Telegram chat |
| [mcp-server.md](mcp-server.md) | Exposing Bellona's tools to Claude Desktop / any MCP client |

Prerequisite for model-backed examples:

```sh
ollama pull qwen2.5:7b
cargo build --release
```
