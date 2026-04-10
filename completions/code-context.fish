# Print an optspec for argparse to handle cmd's options that are independent of any subcommand.
function __fish_code_context_global_optspecs
	string join \n d/debug j/json h/help V/version
end

function __fish_code_context_needs_command
	# Figure out if the current invocation already has a command.
	set -l cmd (commandline -opc)
	set -e cmd[1]
	argparse -s (__fish_code_context_global_optspecs) -- $cmd 2>/dev/null
	or return
	if set -q argv[1]
		# Also print the command, so this can be used to figure out what it is.
		echo $argv[1]
		return 1
	end
	return 0
end

function __fish_code_context_using_subcommand
	set -l cmd (__fish_code_context_needs_command)
	test -z "$cmd"
	and return 1
	contains -- $cmd[1] $argv
end

complete -c code-context -n "__fish_code_context_needs_command" -s d -l debug -d 'Enable debug output to stderr'
complete -c code-context -n "__fish_code_context_needs_command" -s j -l json -d 'Output results as JSON (for operation commands)'
complete -c code-context -n "__fish_code_context_needs_command" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c code-context -n "__fish_code_context_needs_command" -s V -l version -d 'Print version'
complete -c code-context -n "__fish_code_context_needs_command" -f -a "serve" -d 'Run MCP server over stdio, exposing code-context tools'
complete -c code-context -n "__fish_code_context_needs_command" -f -a "init" -d 'Install code-context MCP server into Claude Code settings'
complete -c code-context -n "__fish_code_context_needs_command" -f -a "deinit" -d 'Remove code-context from Claude Code settings'
complete -c code-context -n "__fish_code_context_needs_command" -f -a "doctor" -d 'Diagnose code-context configuration and setup'
complete -c code-context -n "__fish_code_context_needs_command" -f -a "skill" -d 'Deploy code-context skill to agent .skills/ directories'
complete -c code-context -n "__fish_code_context_needs_command" -f -a "get" -d 'Get a resource (symbol, callgraph, blast radius, status, etc.)'
complete -c code-context -n "__fish_code_context_needs_command" -f -a "search" -d 'Search for symbols, code, or workspace symbols'
complete -c code-context -n "__fish_code_context_needs_command" -f -a "list" -d 'List resources (symbols in a file)'
complete -c code-context -n "__fish_code_context_needs_command" -f -a "grep" -d 'Regex search across stored code chunks'
complete -c code-context -n "__fish_code_context_needs_command" -f -a "query" -d 'Execute tree-sitter queries against parsed ASTs'
complete -c code-context -n "__fish_code_context_needs_command" -f -a "find" -d 'Find duplicated code'
complete -c code-context -n "__fish_code_context_needs_command" -f -a "build" -d 'Trigger re-indexing'
complete -c code-context -n "__fish_code_context_needs_command" -f -a "clear" -d 'Wipe index data'
complete -c code-context -n "__fish_code_context_needs_command" -f -a "lsp" -d 'LSP server management'
complete -c code-context -n "__fish_code_context_needs_command" -f -a "detect" -d 'Detect project types and languages'
complete -c code-context -n "__fish_code_context_needs_command" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c code-context -n "__fish_code_context_using_subcommand serve" -s d -l debug -d 'Enable debug output to stderr'
complete -c code-context -n "__fish_code_context_using_subcommand serve" -s j -l json -d 'Output results as JSON (for operation commands)'
complete -c code-context -n "__fish_code_context_using_subcommand serve" -s h -l help -d 'Print help'
complete -c code-context -n "__fish_code_context_using_subcommand init" -s d -l debug -d 'Enable debug output to stderr'
complete -c code-context -n "__fish_code_context_using_subcommand init" -s j -l json -d 'Output results as JSON (for operation commands)'
complete -c code-context -n "__fish_code_context_using_subcommand init" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c code-context -n "__fish_code_context_using_subcommand deinit" -s d -l debug -d 'Enable debug output to stderr'
complete -c code-context -n "__fish_code_context_using_subcommand deinit" -s j -l json -d 'Output results as JSON (for operation commands)'
complete -c code-context -n "__fish_code_context_using_subcommand deinit" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c code-context -n "__fish_code_context_using_subcommand doctor" -s v -l verbose -d 'Show detailed output including fix suggestions'
complete -c code-context -n "__fish_code_context_using_subcommand doctor" -s d -l debug -d 'Enable debug output to stderr'
complete -c code-context -n "__fish_code_context_using_subcommand doctor" -s j -l json -d 'Output results as JSON (for operation commands)'
complete -c code-context -n "__fish_code_context_using_subcommand doctor" -s h -l help -d 'Print help'
complete -c code-context -n "__fish_code_context_using_subcommand skill" -s d -l debug -d 'Enable debug output to stderr'
complete -c code-context -n "__fish_code_context_using_subcommand skill" -s j -l json -d 'Output results as JSON (for operation commands)'
complete -c code-context -n "__fish_code_context_using_subcommand skill" -s h -l help -d 'Print help'
complete -c code-context -n "__fish_code_context_using_subcommand get; and not __fish_seen_subcommand_from symbol callgraph blastradius status definition type-definition hover references implementations code-actions inbound-calls rename-edits diagnostics help" -s d -l debug -d 'Enable debug output to stderr'
complete -c code-context -n "__fish_code_context_using_subcommand get; and not __fish_seen_subcommand_from symbol callgraph blastradius status definition type-definition hover references implementations code-actions inbound-calls rename-edits diagnostics help" -s j -l json -d 'Output results as JSON (for operation commands)'
complete -c code-context -n "__fish_code_context_using_subcommand get; and not __fish_seen_subcommand_from symbol callgraph blastradius status definition type-definition hover references implementations code-actions inbound-calls rename-edits diagnostics help" -s h -l help -d 'Print help'
complete -c code-context -n "__fish_code_context_using_subcommand get; and not __fish_seen_subcommand_from symbol callgraph blastradius status definition type-definition hover references implementations code-actions inbound-calls rename-edits diagnostics help" -f -a "symbol" -d 'Look up symbol locations and source text with fuzzy matching'
complete -c code-context -n "__fish_code_context_using_subcommand get; and not __fish_seen_subcommand_from symbol callgraph blastradius status definition type-definition hover references implementations code-actions inbound-calls rename-edits diagnostics help" -f -a "callgraph" -d 'Traverse call graph from a starting symbol'
complete -c code-context -n "__fish_code_context_using_subcommand get; and not __fish_seen_subcommand_from symbol callgraph blastradius status definition type-definition hover references implementations code-actions inbound-calls rename-edits diagnostics help" -f -a "blastradius" -d 'Analyze blast radius of changes to a file or symbol'
complete -c code-context -n "__fish_code_context_using_subcommand get; and not __fish_seen_subcommand_from symbol callgraph blastradius status definition type-definition hover references implementations code-actions inbound-calls rename-edits diagnostics help" -f -a "status" -d 'Health report with file counts, indexing progress, chunk/edge counts'
complete -c code-context -n "__fish_code_context_using_subcommand get; and not __fish_seen_subcommand_from symbol callgraph blastradius status definition type-definition hover references implementations code-actions inbound-calls rename-edits diagnostics help" -f -a "definition" -d 'Go to definition with layered resolution (live LSP, LSP index, tree-sitter)'
complete -c code-context -n "__fish_code_context_using_subcommand get; and not __fish_seen_subcommand_from symbol callgraph blastradius status definition type-definition hover references implementations code-actions inbound-calls rename-edits diagnostics help" -f -a "type-definition" -d 'Go to type definition (live LSP only)'
complete -c code-context -n "__fish_code_context_using_subcommand get; and not __fish_seen_subcommand_from symbol callgraph blastradius status definition type-definition hover references implementations code-actions inbound-calls rename-edits diagnostics help" -f -a "hover" -d 'Get hover information (type signature, docs)'
complete -c code-context -n "__fish_code_context_using_subcommand get; and not __fish_seen_subcommand_from symbol callgraph blastradius status definition type-definition hover references implementations code-actions inbound-calls rename-edits diagnostics help" -f -a "references" -d 'Find all references to a symbol'
complete -c code-context -n "__fish_code_context_using_subcommand get; and not __fish_seen_subcommand_from symbol callgraph blastradius status definition type-definition hover references implementations code-actions inbound-calls rename-edits diagnostics help" -f -a "implementations" -d 'Find implementations of a trait/interface'
complete -c code-context -n "__fish_code_context_using_subcommand get; and not __fish_seen_subcommand_from symbol callgraph blastradius status definition type-definition hover references implementations code-actions inbound-calls rename-edits diagnostics help" -f -a "code-actions" -d 'Get code actions (quickfixes, refactors) for a range (live LSP only)'
complete -c code-context -n "__fish_code_context_using_subcommand get; and not __fish_seen_subcommand_from symbol callgraph blastradius status definition type-definition hover references implementations code-actions inbound-calls rename-edits diagnostics help" -f -a "inbound-calls" -d 'Find all callers of a function at a given position'
complete -c code-context -n "__fish_code_context_using_subcommand get; and not __fish_seen_subcommand_from symbol callgraph blastradius status definition type-definition hover references implementations code-actions inbound-calls rename-edits diagnostics help" -f -a "rename-edits" -d 'Preview rename edits without applying them (live LSP only)'
complete -c code-context -n "__fish_code_context_using_subcommand get; and not __fish_seen_subcommand_from symbol callgraph blastradius status definition type-definition hover references implementations code-actions inbound-calls rename-edits diagnostics help" -f -a "diagnostics" -d 'Get errors and warnings for a file (live LSP only)'
complete -c code-context -n "__fish_code_context_using_subcommand get; and not __fish_seen_subcommand_from symbol callgraph blastradius status definition type-definition hover references implementations code-actions inbound-calls rename-edits diagnostics help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from symbol" -l query -d 'Symbol name or qualified path to search for' -r
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from symbol" -l max-results -d 'Maximum number of results to return' -r
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from symbol" -s d -l debug -d 'Enable debug output to stderr'
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from symbol" -s j -l json -d 'Output results as JSON (for operation commands)'
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from symbol" -s h -l help -d 'Print help'
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from callgraph" -l symbol -d 'Symbol identifier (name or file:line:char locator)' -r
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from callgraph" -l direction -d 'Traversal direction: inbound, outbound, or both' -r
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from callgraph" -l max-depth -d 'Maximum traversal depth (1-5)' -r
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from callgraph" -s d -l debug -d 'Enable debug output to stderr'
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from callgraph" -s j -l json -d 'Output results as JSON (for operation commands)'
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from callgraph" -s h -l help -d 'Print help'
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from blastradius" -l file-path -d 'File path to analyze' -r
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from blastradius" -l symbol -d 'Optional symbol name to narrow the starting set' -r
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from blastradius" -l max-hops -d 'Maximum number of hops to follow (1-10)' -r
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from blastradius" -s d -l debug -d 'Enable debug output to stderr'
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from blastradius" -s j -l json -d 'Output results as JSON (for operation commands)'
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from blastradius" -s h -l help -d 'Print help'
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from status" -s d -l debug -d 'Enable debug output to stderr'
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from status" -s j -l json -d 'Output results as JSON (for operation commands)'
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from status" -s h -l help -d 'Print help'
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from definition" -l file-path -d 'Path to the file containing the symbol' -r
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from definition" -l line -d 'Zero-based line number of the symbol' -r
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from definition" -l character -d 'Zero-based character offset within the line' -r
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from definition" -s d -l debug -d 'Enable debug output to stderr'
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from definition" -s j -l json -d 'Output results as JSON (for operation commands)'
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from definition" -s h -l help -d 'Print help'
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from type-definition" -l file-path -d 'Path to the file containing the symbol' -r
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from type-definition" -l line -d 'Zero-based line number of the symbol' -r
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from type-definition" -l character -d 'Zero-based character offset within the line' -r
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from type-definition" -s d -l debug -d 'Enable debug output to stderr'
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from type-definition" -s j -l json -d 'Output results as JSON (for operation commands)'
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from type-definition" -s h -l help -d 'Print help'
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from hover" -l file-path -d 'Path to the file containing the symbol' -r
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from hover" -l line -d 'Zero-based line number of the symbol' -r
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from hover" -l character -d 'Zero-based character offset within the line' -r
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from hover" -s d -l debug -d 'Enable debug output to stderr'
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from hover" -s j -l json -d 'Output results as JSON (for operation commands)'
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from hover" -s h -l help -d 'Print help'
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from references" -l file-path -d 'Path to the file containing the symbol' -r
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from references" -l line -d 'Zero-based line number of the symbol' -r
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from references" -l character -d 'Zero-based character offset within the line' -r
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from references" -l include-declaration -d 'Whether to include the declaration itself in results' -r -f -a "true\t''
false\t''"
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from references" -l max-results -d 'Maximum number of references to return' -r
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from references" -s d -l debug -d 'Enable debug output to stderr'
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from references" -s j -l json -d 'Output results as JSON (for operation commands)'
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from references" -s h -l help -d 'Print help'
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from implementations" -l file-path -d 'Path to the file containing the trait/interface symbol' -r
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from implementations" -l line -d 'Zero-based line number of the symbol' -r
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from implementations" -l character -d 'Zero-based character offset within the line' -r
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from implementations" -l max-results -d 'Maximum number of implementation locations to return' -r
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from implementations" -s d -l debug -d 'Enable debug output to stderr'
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from implementations" -s j -l json -d 'Output results as JSON (for operation commands)'
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from implementations" -s h -l help -d 'Print help'
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from code-actions" -l file-path -d 'Path to the file to get code actions for' -r
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from code-actions" -l start-line -d 'Zero-based start line of the range' -r
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from code-actions" -l start-character -d 'Zero-based start character offset' -r
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from code-actions" -l end-line -d 'Zero-based end line of the range' -r
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from code-actions" -l end-character -d 'Zero-based end character offset' -r
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from code-actions" -l filter-kind -d 'Filter for code action kinds (e.g. quickfix, refactor, source)' -r
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from code-actions" -s d -l debug -d 'Enable debug output to stderr'
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from code-actions" -s j -l json -d 'Output results as JSON (for operation commands)'
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from code-actions" -s h -l help -d 'Print help'
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from inbound-calls" -l file-path -d 'Path to the file containing the target symbol' -r
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from inbound-calls" -l line -d 'Zero-based line number of the target symbol' -r
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from inbound-calls" -l character -d 'Zero-based character offset within the line' -r
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from inbound-calls" -l depth -d 'Recursive depth for caller traversal (1-5)' -r
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from inbound-calls" -s d -l debug -d 'Enable debug output to stderr'
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from inbound-calls" -s j -l json -d 'Output results as JSON (for operation commands)'
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from inbound-calls" -s h -l help -d 'Print help'
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from rename-edits" -l file-path -d 'Path to the file containing the symbol to rename' -r
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from rename-edits" -l line -d 'Zero-based line number of the symbol' -r
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from rename-edits" -l character -d 'Zero-based character offset within the line' -r
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from rename-edits" -l new-name -d 'The new name for the symbol' -r
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from rename-edits" -s d -l debug -d 'Enable debug output to stderr'
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from rename-edits" -s j -l json -d 'Output results as JSON (for operation commands)'
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from rename-edits" -s h -l help -d 'Print help'
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from diagnostics" -l file-path -d 'Path to the file to get diagnostics for' -r
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from diagnostics" -l severity-filter -d 'Only return diagnostics at or above this severity (error, warning, info, hint)' -r
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from diagnostics" -s d -l debug -d 'Enable debug output to stderr'
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from diagnostics" -s j -l json -d 'Output results as JSON (for operation commands)'
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from diagnostics" -s h -l help -d 'Print help'
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from help" -f -a "symbol" -d 'Look up symbol locations and source text with fuzzy matching'
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from help" -f -a "callgraph" -d 'Traverse call graph from a starting symbol'
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from help" -f -a "blastradius" -d 'Analyze blast radius of changes to a file or symbol'
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from help" -f -a "status" -d 'Health report with file counts, indexing progress, chunk/edge counts'
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from help" -f -a "definition" -d 'Go to definition with layered resolution (live LSP, LSP index, tree-sitter)'
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from help" -f -a "type-definition" -d 'Go to type definition (live LSP only)'
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from help" -f -a "hover" -d 'Get hover information (type signature, docs)'
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from help" -f -a "references" -d 'Find all references to a symbol'
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from help" -f -a "implementations" -d 'Find implementations of a trait/interface'
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from help" -f -a "code-actions" -d 'Get code actions (quickfixes, refactors) for a range (live LSP only)'
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from help" -f -a "inbound-calls" -d 'Find all callers of a function at a given position'
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from help" -f -a "rename-edits" -d 'Preview rename edits without applying them (live LSP only)'
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from help" -f -a "diagnostics" -d 'Get errors and warnings for a file (live LSP only)'
complete -c code-context -n "__fish_code_context_using_subcommand get; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c code-context -n "__fish_code_context_using_subcommand search; and not __fish_seen_subcommand_from symbol code workspace-symbol help" -s d -l debug -d 'Enable debug output to stderr'
complete -c code-context -n "__fish_code_context_using_subcommand search; and not __fish_seen_subcommand_from symbol code workspace-symbol help" -s j -l json -d 'Output results as JSON (for operation commands)'
complete -c code-context -n "__fish_code_context_using_subcommand search; and not __fish_seen_subcommand_from symbol code workspace-symbol help" -s h -l help -d 'Print help'
complete -c code-context -n "__fish_code_context_using_subcommand search; and not __fish_seen_subcommand_from symbol code workspace-symbol help" -f -a "symbol" -d 'Fuzzy search across all indexed symbols'
complete -c code-context -n "__fish_code_context_using_subcommand search; and not __fish_seen_subcommand_from symbol code workspace-symbol help" -f -a "code" -d 'Semantic similarity search across code chunks using embeddings'
complete -c code-context -n "__fish_code_context_using_subcommand search; and not __fish_seen_subcommand_from symbol code workspace-symbol help" -f -a "workspace-symbol" -d 'Live workspace symbol search with layered resolution'
complete -c code-context -n "__fish_code_context_using_subcommand search; and not __fish_seen_subcommand_from symbol code workspace-symbol help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c code-context -n "__fish_code_context_using_subcommand search; and __fish_seen_subcommand_from symbol" -l query -d 'Text to fuzzy-match against symbol names' -r
complete -c code-context -n "__fish_code_context_using_subcommand search; and __fish_seen_subcommand_from symbol" -l kind -d 'Filter by symbol kind (function, method, struct, class, etc.)' -r
complete -c code-context -n "__fish_code_context_using_subcommand search; and __fish_seen_subcommand_from symbol" -l max-results -d 'Maximum number of results to return' -r
complete -c code-context -n "__fish_code_context_using_subcommand search; and __fish_seen_subcommand_from symbol" -s d -l debug -d 'Enable debug output to stderr'
complete -c code-context -n "__fish_code_context_using_subcommand search; and __fish_seen_subcommand_from symbol" -s j -l json -d 'Output results as JSON (for operation commands)'
complete -c code-context -n "__fish_code_context_using_subcommand search; and __fish_seen_subcommand_from symbol" -s h -l help -d 'Print help'
complete -c code-context -n "__fish_code_context_using_subcommand search; and __fish_seen_subcommand_from code" -l query -d 'Natural language query for semantically similar code' -r
complete -c code-context -n "__fish_code_context_using_subcommand search; and __fish_seen_subcommand_from code" -l top-k -d 'Maximum number of results to return' -r
complete -c code-context -n "__fish_code_context_using_subcommand search; and __fish_seen_subcommand_from code" -l min-similarity -d 'Minimum cosine similarity threshold (0.0-1.0)' -r
complete -c code-context -n "__fish_code_context_using_subcommand search; and __fish_seen_subcommand_from code" -l file-pattern -d 'Only search chunks from files matching this path pattern' -r
complete -c code-context -n "__fish_code_context_using_subcommand search; and __fish_seen_subcommand_from code" -l language -d 'Only search chunks from files with these extensions' -r
complete -c code-context -n "__fish_code_context_using_subcommand search; and __fish_seen_subcommand_from code" -s d -l debug -d 'Enable debug output to stderr'
complete -c code-context -n "__fish_code_context_using_subcommand search; and __fish_seen_subcommand_from code" -s j -l json -d 'Output results as JSON (for operation commands)'
complete -c code-context -n "__fish_code_context_using_subcommand search; and __fish_seen_subcommand_from code" -s h -l help -d 'Print help'
complete -c code-context -n "__fish_code_context_using_subcommand search; and __fish_seen_subcommand_from workspace-symbol" -l query -d 'Symbol name or text to search for across the workspace' -r
complete -c code-context -n "__fish_code_context_using_subcommand search; and __fish_seen_subcommand_from workspace-symbol" -l max-results -d 'Maximum number of results to return' -r
complete -c code-context -n "__fish_code_context_using_subcommand search; and __fish_seen_subcommand_from workspace-symbol" -s d -l debug -d 'Enable debug output to stderr'
complete -c code-context -n "__fish_code_context_using_subcommand search; and __fish_seen_subcommand_from workspace-symbol" -s j -l json -d 'Output results as JSON (for operation commands)'
complete -c code-context -n "__fish_code_context_using_subcommand search; and __fish_seen_subcommand_from workspace-symbol" -s h -l help -d 'Print help'
complete -c code-context -n "__fish_code_context_using_subcommand search; and __fish_seen_subcommand_from help" -f -a "symbol" -d 'Fuzzy search across all indexed symbols'
complete -c code-context -n "__fish_code_context_using_subcommand search; and __fish_seen_subcommand_from help" -f -a "code" -d 'Semantic similarity search across code chunks using embeddings'
complete -c code-context -n "__fish_code_context_using_subcommand search; and __fish_seen_subcommand_from help" -f -a "workspace-symbol" -d 'Live workspace symbol search with layered resolution'
complete -c code-context -n "__fish_code_context_using_subcommand search; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c code-context -n "__fish_code_context_using_subcommand list; and not __fish_seen_subcommand_from symbols help" -s d -l debug -d 'Enable debug output to stderr'
complete -c code-context -n "__fish_code_context_using_subcommand list; and not __fish_seen_subcommand_from symbols help" -s j -l json -d 'Output results as JSON (for operation commands)'
complete -c code-context -n "__fish_code_context_using_subcommand list; and not __fish_seen_subcommand_from symbols help" -s h -l help -d 'Print help'
complete -c code-context -n "__fish_code_context_using_subcommand list; and not __fish_seen_subcommand_from symbols help" -f -a "symbols" -d 'List all symbols in a specific file, sorted by start line'
complete -c code-context -n "__fish_code_context_using_subcommand list; and not __fish_seen_subcommand_from symbols help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c code-context -n "__fish_code_context_using_subcommand list; and __fish_seen_subcommand_from symbols" -l file-path -d 'Path to the file to list symbols from' -r
complete -c code-context -n "__fish_code_context_using_subcommand list; and __fish_seen_subcommand_from symbols" -s d -l debug -d 'Enable debug output to stderr'
complete -c code-context -n "__fish_code_context_using_subcommand list; and __fish_seen_subcommand_from symbols" -s j -l json -d 'Output results as JSON (for operation commands)'
complete -c code-context -n "__fish_code_context_using_subcommand list; and __fish_seen_subcommand_from symbols" -s h -l help -d 'Print help'
complete -c code-context -n "__fish_code_context_using_subcommand list; and __fish_seen_subcommand_from help" -f -a "symbols" -d 'List all symbols in a specific file, sorted by start line'
complete -c code-context -n "__fish_code_context_using_subcommand list; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c code-context -n "__fish_code_context_using_subcommand grep; and not __fish_seen_subcommand_from code help" -s d -l debug -d 'Enable debug output to stderr'
complete -c code-context -n "__fish_code_context_using_subcommand grep; and not __fish_seen_subcommand_from code help" -s j -l json -d 'Output results as JSON (for operation commands)'
complete -c code-context -n "__fish_code_context_using_subcommand grep; and not __fish_seen_subcommand_from code help" -s h -l help -d 'Print help'
complete -c code-context -n "__fish_code_context_using_subcommand grep; and not __fish_seen_subcommand_from code help" -f -a "code" -d 'Regex search across stored code chunks'
complete -c code-context -n "__fish_code_context_using_subcommand grep; and not __fish_seen_subcommand_from code help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c code-context -n "__fish_code_context_using_subcommand grep; and __fish_seen_subcommand_from code" -l pattern -d 'Regex pattern to search for' -r
complete -c code-context -n "__fish_code_context_using_subcommand grep; and __fish_seen_subcommand_from code" -l language -d 'Only search chunks from files with these extensions (e.g. rs, py)' -r
complete -c code-context -n "__fish_code_context_using_subcommand grep; and __fish_seen_subcommand_from code" -l files -d 'Only search chunks from these specific file paths' -r
complete -c code-context -n "__fish_code_context_using_subcommand grep; and __fish_seen_subcommand_from code" -l max-results -d 'Maximum number of matching chunks to return' -r
complete -c code-context -n "__fish_code_context_using_subcommand grep; and __fish_seen_subcommand_from code" -s d -l debug -d 'Enable debug output to stderr'
complete -c code-context -n "__fish_code_context_using_subcommand grep; and __fish_seen_subcommand_from code" -s j -l json -d 'Output results as JSON (for operation commands)'
complete -c code-context -n "__fish_code_context_using_subcommand grep; and __fish_seen_subcommand_from code" -s h -l help -d 'Print help'
complete -c code-context -n "__fish_code_context_using_subcommand grep; and __fish_seen_subcommand_from help" -f -a "code" -d 'Regex search across stored code chunks'
complete -c code-context -n "__fish_code_context_using_subcommand grep; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c code-context -n "__fish_code_context_using_subcommand query; and not __fish_seen_subcommand_from ast help" -s d -l debug -d 'Enable debug output to stderr'
complete -c code-context -n "__fish_code_context_using_subcommand query; and not __fish_seen_subcommand_from ast help" -s j -l json -d 'Output results as JSON (for operation commands)'
complete -c code-context -n "__fish_code_context_using_subcommand query; and not __fish_seen_subcommand_from ast help" -s h -l help -d 'Print help'
complete -c code-context -n "__fish_code_context_using_subcommand query; and not __fish_seen_subcommand_from ast help" -f -a "ast" -d 'Execute tree-sitter S-expression queries against parsed ASTs'
complete -c code-context -n "__fish_code_context_using_subcommand query; and not __fish_seen_subcommand_from ast help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c code-context -n "__fish_code_context_using_subcommand query; and __fish_seen_subcommand_from ast" -l query -d 'Tree-sitter S-expression query pattern' -r
complete -c code-context -n "__fish_code_context_using_subcommand query; and __fish_seen_subcommand_from ast" -l language -d 'Language to parse files as (e.g. rust, python, typescript)' -r
complete -c code-context -n "__fish_code_context_using_subcommand query; and __fish_seen_subcommand_from ast" -l files -d 'File paths to query against' -r
complete -c code-context -n "__fish_code_context_using_subcommand query; and __fish_seen_subcommand_from ast" -l max-results -d 'Maximum number of matches to return' -r
complete -c code-context -n "__fish_code_context_using_subcommand query; and __fish_seen_subcommand_from ast" -s d -l debug -d 'Enable debug output to stderr'
complete -c code-context -n "__fish_code_context_using_subcommand query; and __fish_seen_subcommand_from ast" -s j -l json -d 'Output results as JSON (for operation commands)'
complete -c code-context -n "__fish_code_context_using_subcommand query; and __fish_seen_subcommand_from ast" -s h -l help -d 'Print help'
complete -c code-context -n "__fish_code_context_using_subcommand query; and __fish_seen_subcommand_from help" -f -a "ast" -d 'Execute tree-sitter S-expression queries against parsed ASTs'
complete -c code-context -n "__fish_code_context_using_subcommand query; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c code-context -n "__fish_code_context_using_subcommand find; and not __fish_seen_subcommand_from duplicates help" -s d -l debug -d 'Enable debug output to stderr'
complete -c code-context -n "__fish_code_context_using_subcommand find; and not __fish_seen_subcommand_from duplicates help" -s j -l json -d 'Output results as JSON (for operation commands)'
complete -c code-context -n "__fish_code_context_using_subcommand find; and not __fish_seen_subcommand_from duplicates help" -s h -l help -d 'Print help'
complete -c code-context -n "__fish_code_context_using_subcommand find; and not __fish_seen_subcommand_from duplicates help" -f -a "duplicates" -d 'Find code in a file that is duplicated elsewhere in the codebase'
complete -c code-context -n "__fish_code_context_using_subcommand find; and not __fish_seen_subcommand_from duplicates help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c code-context -n "__fish_code_context_using_subcommand find; and __fish_seen_subcommand_from duplicates" -l file-path -d 'File to check for duplicated code' -r
complete -c code-context -n "__fish_code_context_using_subcommand find; and __fish_seen_subcommand_from duplicates" -l min-similarity -d 'Minimum cosine similarity to report as duplicate (0.0-1.0)' -r
complete -c code-context -n "__fish_code_context_using_subcommand find; and __fish_seen_subcommand_from duplicates" -l max-per-chunk -d 'Maximum duplicates to show per source chunk' -r
complete -c code-context -n "__fish_code_context_using_subcommand find; and __fish_seen_subcommand_from duplicates" -l min-chunk-bytes -d 'Minimum chunk size in bytes to consider' -r
complete -c code-context -n "__fish_code_context_using_subcommand find; and __fish_seen_subcommand_from duplicates" -s d -l debug -d 'Enable debug output to stderr'
complete -c code-context -n "__fish_code_context_using_subcommand find; and __fish_seen_subcommand_from duplicates" -s j -l json -d 'Output results as JSON (for operation commands)'
complete -c code-context -n "__fish_code_context_using_subcommand find; and __fish_seen_subcommand_from duplicates" -s h -l help -d 'Print help'
complete -c code-context -n "__fish_code_context_using_subcommand find; and __fish_seen_subcommand_from help" -f -a "duplicates" -d 'Find code in a file that is duplicated elsewhere in the codebase'
complete -c code-context -n "__fish_code_context_using_subcommand find; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c code-context -n "__fish_code_context_using_subcommand build; and not __fish_seen_subcommand_from status help" -s d -l debug -d 'Enable debug output to stderr'
complete -c code-context -n "__fish_code_context_using_subcommand build; and not __fish_seen_subcommand_from status help" -s j -l json -d 'Output results as JSON (for operation commands)'
complete -c code-context -n "__fish_code_context_using_subcommand build; and not __fish_seen_subcommand_from status help" -s h -l help -d 'Print help'
complete -c code-context -n "__fish_code_context_using_subcommand build; and not __fish_seen_subcommand_from status help" -f -a "status" -d 'Mark files for re-indexing by resetting indexed flags'
complete -c code-context -n "__fish_code_context_using_subcommand build; and not __fish_seen_subcommand_from status help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c code-context -n "__fish_code_context_using_subcommand build; and __fish_seen_subcommand_from status" -l layer -d 'Which indexing layer to reset: treesitter, lsp, or both' -r
complete -c code-context -n "__fish_code_context_using_subcommand build; and __fish_seen_subcommand_from status" -s d -l debug -d 'Enable debug output to stderr'
complete -c code-context -n "__fish_code_context_using_subcommand build; and __fish_seen_subcommand_from status" -s j -l json -d 'Output results as JSON (for operation commands)'
complete -c code-context -n "__fish_code_context_using_subcommand build; and __fish_seen_subcommand_from status" -s h -l help -d 'Print help'
complete -c code-context -n "__fish_code_context_using_subcommand build; and __fish_seen_subcommand_from help" -f -a "status" -d 'Mark files for re-indexing by resetting indexed flags'
complete -c code-context -n "__fish_code_context_using_subcommand build; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c code-context -n "__fish_code_context_using_subcommand clear; and not __fish_seen_subcommand_from status help" -s d -l debug -d 'Enable debug output to stderr'
complete -c code-context -n "__fish_code_context_using_subcommand clear; and not __fish_seen_subcommand_from status help" -s j -l json -d 'Output results as JSON (for operation commands)'
complete -c code-context -n "__fish_code_context_using_subcommand clear; and not __fish_seen_subcommand_from status help" -s h -l help -d 'Print help'
complete -c code-context -n "__fish_code_context_using_subcommand clear; and not __fish_seen_subcommand_from status help" -f -a "status" -d 'Wipe all index data and return stats about what was cleared'
complete -c code-context -n "__fish_code_context_using_subcommand clear; and not __fish_seen_subcommand_from status help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c code-context -n "__fish_code_context_using_subcommand clear; and __fish_seen_subcommand_from status" -s d -l debug -d 'Enable debug output to stderr'
complete -c code-context -n "__fish_code_context_using_subcommand clear; and __fish_seen_subcommand_from status" -s j -l json -d 'Output results as JSON (for operation commands)'
complete -c code-context -n "__fish_code_context_using_subcommand clear; and __fish_seen_subcommand_from status" -s h -l help -d 'Print help'
complete -c code-context -n "__fish_code_context_using_subcommand clear; and __fish_seen_subcommand_from help" -f -a "status" -d 'Wipe all index data and return stats about what was cleared'
complete -c code-context -n "__fish_code_context_using_subcommand clear; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c code-context -n "__fish_code_context_using_subcommand lsp; and not __fish_seen_subcommand_from status help" -s d -l debug -d 'Enable debug output to stderr'
complete -c code-context -n "__fish_code_context_using_subcommand lsp; and not __fish_seen_subcommand_from status help" -s j -l json -d 'Output results as JSON (for operation commands)'
complete -c code-context -n "__fish_code_context_using_subcommand lsp; and not __fish_seen_subcommand_from status help" -s h -l help -d 'Print help'
complete -c code-context -n "__fish_code_context_using_subcommand lsp; and not __fish_seen_subcommand_from status help" -f -a "status" -d 'Show detected languages, their LSP servers, and install status'
complete -c code-context -n "__fish_code_context_using_subcommand lsp; and not __fish_seen_subcommand_from status help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c code-context -n "__fish_code_context_using_subcommand lsp; and __fish_seen_subcommand_from status" -s d -l debug -d 'Enable debug output to stderr'
complete -c code-context -n "__fish_code_context_using_subcommand lsp; and __fish_seen_subcommand_from status" -s j -l json -d 'Output results as JSON (for operation commands)'
complete -c code-context -n "__fish_code_context_using_subcommand lsp; and __fish_seen_subcommand_from status" -s h -l help -d 'Print help'
complete -c code-context -n "__fish_code_context_using_subcommand lsp; and __fish_seen_subcommand_from help" -f -a "status" -d 'Show detected languages, their LSP servers, and install status'
complete -c code-context -n "__fish_code_context_using_subcommand lsp; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c code-context -n "__fish_code_context_using_subcommand detect; and not __fish_seen_subcommand_from projects help" -s d -l debug -d 'Enable debug output to stderr'
complete -c code-context -n "__fish_code_context_using_subcommand detect; and not __fish_seen_subcommand_from projects help" -s j -l json -d 'Output results as JSON (for operation commands)'
complete -c code-context -n "__fish_code_context_using_subcommand detect; and not __fish_seen_subcommand_from projects help" -s h -l help -d 'Print help'
complete -c code-context -n "__fish_code_context_using_subcommand detect; and not __fish_seen_subcommand_from projects help" -f -a "projects" -d 'Detect project types in the workspace and return language-specific guidelines'
complete -c code-context -n "__fish_code_context_using_subcommand detect; and not __fish_seen_subcommand_from projects help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c code-context -n "__fish_code_context_using_subcommand detect; and __fish_seen_subcommand_from projects" -l path -d 'Root path to search for projects' -r
complete -c code-context -n "__fish_code_context_using_subcommand detect; and __fish_seen_subcommand_from projects" -l max-depth -d 'Maximum directory depth to search' -r
complete -c code-context -n "__fish_code_context_using_subcommand detect; and __fish_seen_subcommand_from projects" -l include-guidelines -d 'Include language-specific guidelines in output' -r -f -a "true\t''
false\t''"
complete -c code-context -n "__fish_code_context_using_subcommand detect; and __fish_seen_subcommand_from projects" -s d -l debug -d 'Enable debug output to stderr'
complete -c code-context -n "__fish_code_context_using_subcommand detect; and __fish_seen_subcommand_from projects" -s j -l json -d 'Output results as JSON (for operation commands)'
complete -c code-context -n "__fish_code_context_using_subcommand detect; and __fish_seen_subcommand_from projects" -s h -l help -d 'Print help'
complete -c code-context -n "__fish_code_context_using_subcommand detect; and __fish_seen_subcommand_from help" -f -a "projects" -d 'Detect project types in the workspace and return language-specific guidelines'
complete -c code-context -n "__fish_code_context_using_subcommand detect; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c code-context -n "__fish_code_context_using_subcommand help; and not __fish_seen_subcommand_from serve init deinit doctor skill get search list grep query find build clear lsp detect help" -f -a "serve" -d 'Run MCP server over stdio, exposing code-context tools'
complete -c code-context -n "__fish_code_context_using_subcommand help; and not __fish_seen_subcommand_from serve init deinit doctor skill get search list grep query find build clear lsp detect help" -f -a "init" -d 'Install code-context MCP server into Claude Code settings'
complete -c code-context -n "__fish_code_context_using_subcommand help; and not __fish_seen_subcommand_from serve init deinit doctor skill get search list grep query find build clear lsp detect help" -f -a "deinit" -d 'Remove code-context from Claude Code settings'
complete -c code-context -n "__fish_code_context_using_subcommand help; and not __fish_seen_subcommand_from serve init deinit doctor skill get search list grep query find build clear lsp detect help" -f -a "doctor" -d 'Diagnose code-context configuration and setup'
complete -c code-context -n "__fish_code_context_using_subcommand help; and not __fish_seen_subcommand_from serve init deinit doctor skill get search list grep query find build clear lsp detect help" -f -a "skill" -d 'Deploy code-context skill to agent .skills/ directories'
complete -c code-context -n "__fish_code_context_using_subcommand help; and not __fish_seen_subcommand_from serve init deinit doctor skill get search list grep query find build clear lsp detect help" -f -a "get" -d 'Get a resource (symbol, callgraph, blast radius, status, etc.)'
complete -c code-context -n "__fish_code_context_using_subcommand help; and not __fish_seen_subcommand_from serve init deinit doctor skill get search list grep query find build clear lsp detect help" -f -a "search" -d 'Search for symbols, code, or workspace symbols'
complete -c code-context -n "__fish_code_context_using_subcommand help; and not __fish_seen_subcommand_from serve init deinit doctor skill get search list grep query find build clear lsp detect help" -f -a "list" -d 'List resources (symbols in a file)'
complete -c code-context -n "__fish_code_context_using_subcommand help; and not __fish_seen_subcommand_from serve init deinit doctor skill get search list grep query find build clear lsp detect help" -f -a "grep" -d 'Regex search across stored code chunks'
complete -c code-context -n "__fish_code_context_using_subcommand help; and not __fish_seen_subcommand_from serve init deinit doctor skill get search list grep query find build clear lsp detect help" -f -a "query" -d 'Execute tree-sitter queries against parsed ASTs'
complete -c code-context -n "__fish_code_context_using_subcommand help; and not __fish_seen_subcommand_from serve init deinit doctor skill get search list grep query find build clear lsp detect help" -f -a "find" -d 'Find duplicated code'
complete -c code-context -n "__fish_code_context_using_subcommand help; and not __fish_seen_subcommand_from serve init deinit doctor skill get search list grep query find build clear lsp detect help" -f -a "build" -d 'Trigger re-indexing'
complete -c code-context -n "__fish_code_context_using_subcommand help; and not __fish_seen_subcommand_from serve init deinit doctor skill get search list grep query find build clear lsp detect help" -f -a "clear" -d 'Wipe index data'
complete -c code-context -n "__fish_code_context_using_subcommand help; and not __fish_seen_subcommand_from serve init deinit doctor skill get search list grep query find build clear lsp detect help" -f -a "lsp" -d 'LSP server management'
complete -c code-context -n "__fish_code_context_using_subcommand help; and not __fish_seen_subcommand_from serve init deinit doctor skill get search list grep query find build clear lsp detect help" -f -a "detect" -d 'Detect project types and languages'
complete -c code-context -n "__fish_code_context_using_subcommand help; and not __fish_seen_subcommand_from serve init deinit doctor skill get search list grep query find build clear lsp detect help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c code-context -n "__fish_code_context_using_subcommand help; and __fish_seen_subcommand_from get" -f -a "symbol" -d 'Look up symbol locations and source text with fuzzy matching'
complete -c code-context -n "__fish_code_context_using_subcommand help; and __fish_seen_subcommand_from get" -f -a "callgraph" -d 'Traverse call graph from a starting symbol'
complete -c code-context -n "__fish_code_context_using_subcommand help; and __fish_seen_subcommand_from get" -f -a "blastradius" -d 'Analyze blast radius of changes to a file or symbol'
complete -c code-context -n "__fish_code_context_using_subcommand help; and __fish_seen_subcommand_from get" -f -a "status" -d 'Health report with file counts, indexing progress, chunk/edge counts'
complete -c code-context -n "__fish_code_context_using_subcommand help; and __fish_seen_subcommand_from get" -f -a "definition" -d 'Go to definition with layered resolution (live LSP, LSP index, tree-sitter)'
complete -c code-context -n "__fish_code_context_using_subcommand help; and __fish_seen_subcommand_from get" -f -a "type-definition" -d 'Go to type definition (live LSP only)'
complete -c code-context -n "__fish_code_context_using_subcommand help; and __fish_seen_subcommand_from get" -f -a "hover" -d 'Get hover information (type signature, docs)'
complete -c code-context -n "__fish_code_context_using_subcommand help; and __fish_seen_subcommand_from get" -f -a "references" -d 'Find all references to a symbol'
complete -c code-context -n "__fish_code_context_using_subcommand help; and __fish_seen_subcommand_from get" -f -a "implementations" -d 'Find implementations of a trait/interface'
complete -c code-context -n "__fish_code_context_using_subcommand help; and __fish_seen_subcommand_from get" -f -a "code-actions" -d 'Get code actions (quickfixes, refactors) for a range (live LSP only)'
complete -c code-context -n "__fish_code_context_using_subcommand help; and __fish_seen_subcommand_from get" -f -a "inbound-calls" -d 'Find all callers of a function at a given position'
complete -c code-context -n "__fish_code_context_using_subcommand help; and __fish_seen_subcommand_from get" -f -a "rename-edits" -d 'Preview rename edits without applying them (live LSP only)'
complete -c code-context -n "__fish_code_context_using_subcommand help; and __fish_seen_subcommand_from get" -f -a "diagnostics" -d 'Get errors and warnings for a file (live LSP only)'
complete -c code-context -n "__fish_code_context_using_subcommand help; and __fish_seen_subcommand_from search" -f -a "symbol" -d 'Fuzzy search across all indexed symbols'
complete -c code-context -n "__fish_code_context_using_subcommand help; and __fish_seen_subcommand_from search" -f -a "code" -d 'Semantic similarity search across code chunks using embeddings'
complete -c code-context -n "__fish_code_context_using_subcommand help; and __fish_seen_subcommand_from search" -f -a "workspace-symbol" -d 'Live workspace symbol search with layered resolution'
complete -c code-context -n "__fish_code_context_using_subcommand help; and __fish_seen_subcommand_from list" -f -a "symbols" -d 'List all symbols in a specific file, sorted by start line'
complete -c code-context -n "__fish_code_context_using_subcommand help; and __fish_seen_subcommand_from grep" -f -a "code" -d 'Regex search across stored code chunks'
complete -c code-context -n "__fish_code_context_using_subcommand help; and __fish_seen_subcommand_from query" -f -a "ast" -d 'Execute tree-sitter S-expression queries against parsed ASTs'
complete -c code-context -n "__fish_code_context_using_subcommand help; and __fish_seen_subcommand_from find" -f -a "duplicates" -d 'Find code in a file that is duplicated elsewhere in the codebase'
complete -c code-context -n "__fish_code_context_using_subcommand help; and __fish_seen_subcommand_from build" -f -a "status" -d 'Mark files for re-indexing by resetting indexed flags'
complete -c code-context -n "__fish_code_context_using_subcommand help; and __fish_seen_subcommand_from clear" -f -a "status" -d 'Wipe all index data and return stats about what was cleared'
complete -c code-context -n "__fish_code_context_using_subcommand help; and __fish_seen_subcommand_from lsp" -f -a "status" -d 'Show detected languages, their LSP servers, and install status'
complete -c code-context -n "__fish_code_context_using_subcommand help; and __fish_seen_subcommand_from detect" -f -a "projects" -d 'Detect project types in the workspace and return language-specific guidelines'
