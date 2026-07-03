# Tool Use Compatibility

ModelPort treats Tool Use as a first-class protocol concern for Claude Code and VS Code Claude. The gateway keeps the client-facing contract Anthropic-compatible, then adapts or validates provider-specific behavior at the routing boundary.

## Current Contract

Client requests may use:

- `tools`
- `tool_choice`
- assistant `tool_use` content blocks
- user `tool_result` content blocks
- streaming tool-call arguments

DeepSeek's official Anthropic-compatible API is the reference path and should preserve these fields natively. OpenAI-compatible providers go through ModelPort's adapter.

## OpenAI-Compatible Mapping

Request mapping:

- Anthropic `tools[]` become OpenAI `tools[]` with `type=function`.
- Anthropic `tool_choice.type=auto` becomes OpenAI `"auto"`.
- Anthropic `tool_choice.type=none` becomes OpenAI `"none"`.
- Anthropic `tool_choice.type=any` becomes OpenAI `"required"`.
- Anthropic `tool_choice.type=tool` becomes an OpenAI named function choice.
- Anthropic `tool_choice.disable_parallel_tool_use` becomes OpenAI `parallel_tool_calls=false`.
- Assistant `tool_use` blocks become assistant `tool_calls`.
- User `tool_result` blocks become OpenAI `role=tool` messages.

Response mapping:

- OpenAI `tool_calls` become Anthropic `tool_use` blocks.
- Legacy OpenAI `function_call` responses also become Anthropic `tool_use` blocks.
- Non-object function arguments are wrapped under `_raw_arguments` so Claude Code still receives valid Anthropic `input` objects.
- OpenAI `finish_reason=tool_calls` and `finish_reason=function_call` become Anthropic `stop_reason=tool_use`.

## Streaming Behavior

ModelPort emits Anthropic-style SSE:

- `content_block_start` for each text or tool block.
- `content_block_delta` with `text_delta` for text.
- `content_block_delta` with `input_json_delta` for tool arguments.
- `content_block_stop` when the block ends.
- `message_delta` with the mapped stop reason.
- `message_stop` when complete.

For providers that replay cumulative argument strings, ModelPort can deduplicate fragments and recover the best complete JSON object it can parse. For providers that send arguments before the function name, ModelPort buffers the arguments and starts the tool block once the name arrives. If a provider never sends the name, ModelPort still emits a synthetic `tool` block instead of silently dropping arguments.

## Validation Before Routing

ModelPort rejects malformed Tool Use requests before contacting an upstream provider:

- `tools` must be an array.
- Tool names must be unique.
- Tool names must be 1-64 characters and use letters, numbers, `_`, or `-`.
- Tool `input_schema` must be an object schema when present.
- `tool_choice` must be an object with `type=auto`, `any`, `none`, or `tool`.
- Named `tool_choice` must refer to a declared tool when tools are declared.
- `tool_choice.type=any` and `tool_choice.type=tool` require at least one tool.
- Assistant `tool_use` blocks must have non-empty `id`, valid `name`, and object `input`.
- User `tool_result` blocks must reference a previous assistant `tool_use` id.
- Duplicate `tool_use.id` values are rejected.
- Duplicate `tool_result` answers for the same `tool_use.id` are rejected.
- Tool count and tools JSON size are bounded by `MODELPORT_MAX_TOOLS` and `MODELPORT_MAX_TOOLS_JSON_CHARS`.

## Provider Capability Matrix

Each provider has a lightweight `tool_use` capability matrix:

```toml
[providers.deepseek.tool_use]
supported = true
tool_choice = true
parallel_tool_calls = true
streaming_arguments = "native"
```

Fields:

- `supported`: whether the provider should receive Tool Use requests.
- `tool_choice`: whether the provider supports tool choice control.
- `parallel_tool_calls`: whether the provider supports parallel tool calls.
- `streaming_arguments`: one of `native`, `delta`, `cumulative`, or `best_effort`.

Dashboard provider cards expose this capability, and the provider form can edit it.

The dashboard model page also includes a capability matrix tab that lists protocol, Tool Use support, `tool_choice`, parallel tool calls, streaming argument mode, fidelity mode, and runtime status per provider.

## Acceptance

Run the mock-backed Tool Use acceptance before changing protocol conversion, provider routing, streaming, or Tool Use validation:

```bash
scripts/tool-use-acceptance.sh
```

The default mode:

- starts a temporary local OpenAI-compatible mock upstream;
- creates a temporary `local_` provider through the dashboard API;
- validates non-streaming Tool Use response mapping;
- validates streaming `input_json_delta` mapping;
- validates Anthropic `tool_result` to OpenAI `role=tool` continuation;
- validates malformed Tool Use rejection before upstream routing;
- checks that `disable_parallel_tool_use` maps to `parallel_tool_calls=false`;
- cleans up the temporary provider.

To certify a real configured provider:

```bash
scripts/tool-use-acceptance.sh --upstream
```

## What Is Intentionally Deferred

ModelPort does not yet introduce a heavyweight internal Tool IR. The current adapter remains small and testable. A Tool IR becomes worthwhile when multiple providers need deep normalization beyond the current matrix, such as provider-specific argument repair, schema transformation, or tool-call replay diagnostics.

It also does not run real upstream Tool Use in default smoke tests, because that can consume provider quota. Add provider-specific acceptance tests when you want to certify a channel.
