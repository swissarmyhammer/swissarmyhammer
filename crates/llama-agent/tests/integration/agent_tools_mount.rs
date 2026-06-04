//! Real-path test for the always-on in-memory Agent-tools mount.
//!
//! The load-bearing invariant of the mount is that a llama-agent has its
//! intrinsic Agent tools (files, web, skill, subagent, shell) even when handed
//! ZERO external/ACP MCP servers — they are mounted in-process, not "servers it
//! connects to". This test proves that with the real production pieces, no
//! mocks:
//!
//! 1. Build the real `swissarmyhammer_tools::McpServer` and derive the
//!    agent-tools-only server via `create_agent_tools_server()` (the same call
//!    the `swissarmyhammer-agent` wiring tier makes).
//! 2. Wrap it in `llama_agent::InProcessMount` — the generic duplex serve/
//!    connect mount llama-agent exposes.
//! 3. `connect()` once (exactly what `AcpServer::new_session` does per session)
//!    with NO ACP servers anywhere in the picture.
//! 4. Drive `list_tools_with_schemas()` over the in-process rmcp `tools/list`
//!    handshake and assert the intrinsic Agent tools are present.
//!
//! This exercises the real value handoff across the crate boundary: the tools
//! crate produces an rmcp `ServerHandler`, llama-agent serves it over a
//! `tokio::io::duplex` pair and connects a client, and the tools surface through
//! the standard MCP `tools/list` path.

use llama_agent::{AgentToolsMount, InProcessMount};
use swissarmyhammer_prompts::PromptLibrary;
use swissarmyhammer_tools::McpServer;

/// With an empty external/ACP server list, the agent-tools mount alone still
/// surfaces files, web, skill, subagent, and shell through `tools/list`.
#[tokio::test]
async fn agent_tools_mount_lists_intrinsic_tools_with_no_external_servers() {
    // Real SAH server → agent-tools-only filtered server (compose_per_client =
    // false, so it serves its registry verbatim). This is the exact handler the
    // production wiring mounts.
    let server = McpServer::new(PromptLibrary::default())
        .await
        .expect("build SAH McpServer");
    let agent_tools_server = server.create_agent_tools_server();

    // Mount it in-process. No ACP/external MCP servers are constructed anywhere
    // — the mount is the only source of tools.
    let mount = InProcessMount::new(agent_tools_server);

    // One per-session connection, just like `AcpServer::new_session`.
    let client = mount
        .connect()
        .await
        .expect("connect to in-process agent-tools mount");

    let tools = client
        .list_tools_with_schemas()
        .await
        .expect("list tools over in-process mount");

    let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();

    // The intrinsic Agent tool set must be present from the mount alone.
    for expected in ["files", "web", "skill", "agent", "shell"] {
        assert!(
            names.contains(&expected),
            "expected intrinsic agent tool `{expected}` to be served by the \
             in-memory mount with no external MCP servers; got {names:?}"
        );
    }

    // The split file tools (the names Hermes-trained models emit) ride along
    // with the unified `files` tool.
    for expected in ["read_file", "glob_files", "grep_files"] {
        assert!(
            names.contains(&expected),
            "expected split file tool `{expected}` from the mount; got {names:?}"
        );
    }

    // The schemas must be real, not placeholders: every tool carries a
    // non-empty parameters object so the chat-template renderer sees the true
    // contract.
    let shell = tools
        .iter()
        .find(|t| t.name == "shell")
        .expect("shell tool present");
    assert!(
        shell.parameters.is_object(),
        "shell tool schema should be a JSON object, got {:?}",
        shell.parameters
    );
}
