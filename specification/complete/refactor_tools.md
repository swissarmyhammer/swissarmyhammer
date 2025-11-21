The tools processing is getting to be way too large of a source file.



## MCP Root

Take a look in swissarmyhammer/src.

There is a ./mcp module folder and a mcp.rs -- super confusing -- get all the mcp in ./mcp

This really needs to be in ./mcp/mod.rs, subdivided into modules rather than all crammed into ./mcp.rs

All mcp and mcp tool code needs to be under ./mcp/...

Take a look in swissarmyhammer/src.

## Organization

We need to separate into a module for each related group of tools.

- ./mcp/tools/memoranda
- ./mcp/tools/issues
- ./mcp/tools/search

right now 'search' is called 'semantic' -- but we call it search in the CLI currently and that is a more sensible name to users

and get the code that implements each of these tools into those folders to improve the organization

## Individual tool modules

Each tool should be an individual folder like

./tools/issues/create

Each tool Description needs to be a markdown files brought in by a build macro much like built in prompts.

./tools/issues/create/description.md

So the pattern is NOUN/VERB. Tools are grouped up by noun they work on (memoranda, semantic, issues)

I want to be able to edit the tools descriptions and edit these as stand alone markdown files.

When I edit a description, I want the build to be dirty, so the next `cargo build` will pick up my markdown file changes.

## Tool Registry

The big match statement on strings is just too big. Use a tool service registry -- a toolbox.

`list_prompts` and `get_prompt` have the prompt library -- this is a good pattern, so I know you know how to do a good job.


## CLI

CLI commands need to invoke the exact same tools as the mcp server, turning command line arguments into parameters as needed, formatting the results for CLI output.

The CLI is just another way to call tools and pass parameters -- from a human user rather than an LLM.

Be thoughful about what is a required parameter, and what is optional.

Use named parameters `--<name>` for all parameters to pass through to the tools for consistency.

## TDD

Make sure to TDD -- ensuring tests that invoke each existing MCP before moving anything. Double check if a test exists before.

## Duplicate Code

Once each of the tools is isolated in a module, use it to then scan the entire rest of the source base to eliminate duplication.

Refactor toward the tools

## Removed tools

These tools have been removed from the system:

search_index (removed - semantic search feature deprecated)
search_query (removed - semantic search feature deprecated)

## tool_handlers

tool_handlers.rs -- seems like lots of duplication with mcp.rs and I cannot find where these tools are used.

We need to delete tool_handlers, and if any code in here is actually used, it needs ot be moved into the ./mcp