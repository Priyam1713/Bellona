# MCP: Bellona's tools inside Claude Desktop (or any MCP client)

## stdio transport (Claude Desktop config)

```json
{
  "mcpServers": {
    "bellona": {
      "command": "/abs/path/to/target/release/bellona",
      "args": ["mcp", "--workspace", "/abs/path/to/project"]
    }
  }
}
```

## streamable HTTP (from the War Room)

```sh
bellona serve --yolo &
curl -s localhost:3001/mcp -H 'content-type: application/json' -d \
  '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | jq .
```

## What a client sees

- `initialize` → protocol `2025-03-26`, serverInfo `bellona`
- `tools/list` → read_file, write_file, list_files, search_files,
  read_document, web_fetch, git_status/log/diff/commit/branch, run_shell …
- `tools/call` → **every call passes the Praetorian Gate**: unallowed tools
  return `isError: true` with the refusing rule named; writes park as
  approval tickets exactly like native runs.

Your editor gets capability; your ledger gets receipts.
