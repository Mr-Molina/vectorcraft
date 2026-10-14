# Task 6 Brief: Layer 6 MCP Server Tool Declarations & Protocol Wiring

## Objective
Register and wire the three new agent-centric tools (`semantic_summary`, `cluster_colors`, `render_overlay`) in `crates/mcp/src/tools.rs` and verify them end-to-end via JSON-RPC protocol tests in `crates/mcp/src/tests_protocol.rs`.

## Input Files
- Modify: `crates/mcp/src/tools.rs`
- Modify: `crates/mcp/src/tests_protocol.rs`

## Requirements
1. **Tool Declarations in `crates/mcp/src/tools.rs` (`tool_definitions()`)**:
   - `semantic_summary`:
     - Title: `"Get Semantic Document Summary"`
     - Description: `"Returns a token-efficient summary of the active document: artboards, canvas background card ID, total group and loose path counts, and dominant palette frequency."`
     - Parameters Schema:
       - `selection_only`: boolean (optional)
       - `max_palette_entries`: integer (optional)
     - `read_only`: `true`
   - `cluster_colors`:
     - Title: `"Cluster Nodes by Perceptual Color"`
     - Description: `"Groups or previews nodes clustered by perceptual color distance (CIEDE2000). Action 'preview' returns proposed groups. Action 'group' creates new named color groups atomically."`
     - Parameters Schema:
       - `node_ids`: array of integers (optional)
       - `max_delta_e`: number (required)
       - `action`: string enum `["preview", "group"]` (required)
     - `read_only`: `false`
   - `render_overlay`:
     - Title: `"Render Labeled Visual Overlay"`
     - Description: `"Exports an annotated PNG image of the artboard displaying semi-transparent bounding boxes and high-contrast numbered badges for nodes."`
     - Parameters Schema:
       - `path`: string (required)
       - `scale`: number (optional)
       - `node_ids`: array of integers (optional)
       - `ungrouped_only`: boolean (optional)
     - `read_only`: `true`

2. **Protocol Dispatch in `crates/mcp/src/tools.rs` (`dispatch()`)**:
   Add arms to `dispatch`:
   ```rust
   "semantic_summary" => j(exec(b, "document.semantic_summary", Value::Object(a.clone()))?),
   "cluster_colors" => j(exec(b, "color.cluster", Value::Object(a.clone()))?),
   "render_overlay" => j(exec(b, "render.overlay", Value::Object(a.clone()))?),
   ```

3. **Protocol Tests in `crates/mcp/src/tests_protocol.rs`**:
   Add tests:
   - `test_mcp_tools_include_agent_extensions`:
     Verifies `tool_definitions()` contains `"semantic_summary"`, `"cluster_colors"`, and `"render_overlay"`.
   - `test_mcp_semantic_summary_tool_call`:
     Initializes server with document, calls `tools/call` for `semantic_summary`, asserts `isError: false`, validates `group_count`, `loose_path_count`, `artboard_count` exist in output.
   - `test_mcp_cluster_colors_tool_call`:
     Draws 2 shapes, calls `tools/call` for `cluster_colors` with `{"max_delta_e": 10.0, "action": "preview"}`, asserts `isError: false`, validates `clusters` array in output.
   - `test_mcp_render_overlay_tool_call`:
     Draws a shape, calls `tools/call` for `render_overlay` with temp path, asserts `isError: false`, validates file was created, cleans up temp file.

4. **Safety & Invariants**:
   - Zero panic guarantee: NO `unwrap()`, `expect()`, `panic!`, `todo!`, `unimplemented!` in runtime code (`crates/mcp/src/tools.rs`).
   - Clean-room invariant: Pure public JSON-RPC and MCP protocol schemas.

5. **Commit Message**:
   - `feat(mcp): wire semantic_summary, cluster_colors, and render_overlay tools`
