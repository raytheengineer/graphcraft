# graphcraft-mcp

A minimal Model Context Protocol (MCP) stdio server. An AI
assistant launches `graphcraft-mcp` and speaks line-delimited
JSON-RPC 2.0 on stdin/stdout.

## Build and launch

```
cargo build --release -p graphcraft-mcp
./target/release/graphcraft-mcp     # reads JSON-RPC on stdin
```

## Supported RPC methods

| Method | What it does |
| --- | --- |
| `initialize` | Returns server info and protocol version `2024-11-05`. |
| `tools/list` | Enumerates the tools with their input schemas. |
| `tools/call` | Dispatches one tool with its `arguments` and returns the result as a `TextContent` block. |

The `notifications/initialized` notification is accepted and
silently ignored, as the spec allows.

## Tools

Every tool takes a `csv` string argument (the full CSV content).
Column arguments accept either a 0-based index or the exact header
name.

| Tool | Extra arguments | Returns |
| --- | --- | --- |
| `describe` | — | per-column `n`, `mean`, `sd`, `variance`, `sem`, `median`, `min`, `max` |
| `anova` | — | one-way ANOVA table |
| `ttest_unpaired` | `a`, `b` | Welch's t: means, `mean_diff`, SE, `t`, `df`, `p_two_sided` |
| `ttest_paired` | `a`, `b` | paired t: `n`, `mean_diff`, `sem_diff`, `t`, `df`, `p_two_sided` |
| `correlate` | `a`, `b` | Pearson + Spearman |
| `regress` | `x`, `y` | OLS: intercept/slope with SEs, R², residual SE |
| `chi2_independence` | — | chi-squared on a count-table CSV |
| `plot_xy_svg` | `x`, `y` | SVG string of the scatter |

## Example session

```
$ graphcraft-mcp
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}
{"id":1,"jsonrpc":"2.0","result":{"capabilities":{"tools":{}},
  "protocolVersion":"2024-11-05",
  "serverInfo":{"name":"graphcraft-mcp","version":"0.0.1"}}}

{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{
  "name":"describe",
  "arguments":{"csv":"ctrl,drug\n1,5\n2,9\n3,10\n"}}}
{"id":2,"jsonrpc":"2.0","result":{"content":[{"type":"text",
  "text":"{\n  \"descriptives\": [...]\n}"}]}}
```

## Behaviour reference (public)

- Model Context Protocol specification —
  <https://modelcontextprotocol.io/> (the JSON-RPC message shapes
  used here are from the spec; the implementation is our own, no
  third-party MCP library involved).
