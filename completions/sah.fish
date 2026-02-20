# Print an optspec for argparse to handle cmd's options that are independent of any subcommand.
function __fish_sah_global_optspecs
	string join \n v/verbose d/debug q/quiet format= model= h/help V/version
end

function __fish_sah_needs_command
	# Figure out if the current invocation already has a command.
	set -l cmd (commandline -opc)
	set -e cmd[1]
	argparse -s (__fish_sah_global_optspecs) -- $cmd 2>/dev/null
	or return
	if set -q argv[1]
		# Also print the command, so this can be used to figure out what it is.
		echo $argv[1]
		return 1
	end
	return 0
end

function __fish_sah_using_subcommand
	set -l cmd (__fish_sah_needs_command)
	test -z "$cmd"
	and return 1
	contains -- $cmd[1] $argv
end

complete -c sah -n "__fish_sah_needs_command" -l format -d 'Global output format' -r -f -a "table\t''
json\t''
yaml\t''"
complete -c sah -n "__fish_sah_needs_command" -l model -d 'Override model for all use cases (runtime only, doesn\'t modify config)' -r
complete -c sah -n "__fish_sah_needs_command" -s v -l verbose -d 'Enable verbose logging'
complete -c sah -n "__fish_sah_needs_command" -s d -l debug -d 'Enable debug logging'
complete -c sah -n "__fish_sah_needs_command" -s q -l quiet -d 'Suppress all output except errors'
complete -c sah -n "__fish_sah_needs_command" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c sah -n "__fish_sah_needs_command" -s V -l version -d 'Print version'
complete -c sah -n "__fish_sah_needs_command" -f -a "serve" -d 'Run as MCP server (default when invoked via stdio)'
complete -c sah -n "__fish_sah_needs_command" -f -a "init" -d 'Initialize sah MCP server in Claude Code settings'
complete -c sah -n "__fish_sah_needs_command" -f -a "deinit" -d 'Remove sah MCP server from Claude Code settings'
complete -c sah -n "__fish_sah_needs_command" -f -a "doctor" -d 'Diagnose configuration and setup issues'
complete -c sah -n "__fish_sah_needs_command" -f -a "prompt" -d 'Manage and test prompts'
complete -c sah -n "__fish_sah_needs_command" -f -a "flow" -d 'Execute and manage workflows'
complete -c sah -n "__fish_sah_needs_command" -f -a "completion" -d 'Generate shell completion scripts'
complete -c sah -n "__fish_sah_needs_command" -f -a "validate" -d 'Validate prompt files and workflows for syntax and best practices'
complete -c sah -n "__fish_sah_needs_command" -f -a "model" -d 'Manage and interact with models'
complete -c sah -n "__fish_sah_needs_command" -f -a "agent" -d 'Manage and interact with Agent Client Protocol server'
complete -c sah -n "__fish_sah_needs_command" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c sah -n "__fish_sah_using_subcommand serve; and not __fish_seen_subcommand_from http help" -l model -d 'Override model for all use cases (runtime only, doesn\'t modify config)' -r
complete -c sah -n "__fish_sah_using_subcommand serve; and not __fish_seen_subcommand_from http help" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c sah -n "__fish_sah_using_subcommand serve; and not __fish_seen_subcommand_from http help" -f -a "http" -d 'Start HTTP MCP server'
complete -c sah -n "__fish_sah_using_subcommand serve; and not __fish_seen_subcommand_from http help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c sah -n "__fish_sah_using_subcommand serve; and __fish_seen_subcommand_from http" -s p -l port -d 'Port to bind to (use 0 for random port)' -r
complete -c sah -n "__fish_sah_using_subcommand serve; and __fish_seen_subcommand_from http" -s H -l host -d 'Host to bind to' -r
complete -c sah -n "__fish_sah_using_subcommand serve; and __fish_seen_subcommand_from http" -l model -d 'Override model for all use cases (runtime only, doesn\'t modify config)' -r
complete -c sah -n "__fish_sah_using_subcommand serve; and __fish_seen_subcommand_from http" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c sah -n "__fish_sah_using_subcommand serve; and __fish_seen_subcommand_from help" -f -a "http" -d 'Start HTTP MCP server'
complete -c sah -n "__fish_sah_using_subcommand serve; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c sah -n "__fish_sah_using_subcommand init" -l model -d 'Override model for all use cases (runtime only, doesn\'t modify config)' -r
complete -c sah -n "__fish_sah_using_subcommand init" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c sah -n "__fish_sah_using_subcommand deinit" -l model -d 'Override model for all use cases (runtime only, doesn\'t modify config)' -r
complete -c sah -n "__fish_sah_using_subcommand deinit" -l remove-directory -d 'Also remove .swissarmyhammer/ project directory'
complete -c sah -n "__fish_sah_using_subcommand deinit" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c sah -n "__fish_sah_using_subcommand doctor" -l model -d 'Override model for all use cases (runtime only, doesn\'t modify config)' -r
complete -c sah -n "__fish_sah_using_subcommand doctor" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c sah -n "__fish_sah_using_subcommand prompt" -l model -d 'Override model for all use cases (runtime only, doesn\'t modify config)' -r
complete -c sah -n "__fish_sah_using_subcommand prompt" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c sah -n "__fish_sah_using_subcommand flow" -l model -d 'Override model for all use cases (runtime only, doesn\'t modify config)' -r
complete -c sah -n "__fish_sah_using_subcommand flow" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c sah -n "__fish_sah_using_subcommand completion" -l model -d 'Override model for all use cases (runtime only, doesn\'t modify config)' -r
complete -c sah -n "__fish_sah_using_subcommand completion" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c sah -n "__fish_sah_using_subcommand validate" -l format -d 'Output format' -r -f -a "table\t''
json\t''
yaml\t''"
complete -c sah -n "__fish_sah_using_subcommand validate" -l workflow-dir -d '\\[DEPRECATED\\] This parameter is ignored. Workflows are now only loaded from standard locations' -r
complete -c sah -n "__fish_sah_using_subcommand validate" -l model -d 'Override model for all use cases (runtime only, doesn\'t modify config)' -r
complete -c sah -n "__fish_sah_using_subcommand validate" -s q -l quiet -d 'Suppress all output except errors. In quiet mode, warnings are hidden from both output and summary'
complete -c sah -n "__fish_sah_using_subcommand validate" -l validate-tools -d 'Validate MCP tool schemas for CLI compatibility'
complete -c sah -n "__fish_sah_using_subcommand validate" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c sah -n "__fish_sah_using_subcommand model; and not __fish_seen_subcommand_from list show use help" -l model -d 'Override model for all use cases (runtime only, doesn\'t modify config)' -r
complete -c sah -n "__fish_sah_using_subcommand model; and not __fish_seen_subcommand_from list show use help" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c sah -n "__fish_sah_using_subcommand model; and not __fish_seen_subcommand_from list show use help" -f -a "list" -d 'List available models'
complete -c sah -n "__fish_sah_using_subcommand model; and not __fish_seen_subcommand_from list show use help" -f -a "show" -d 'Show current model use case assignments'
complete -c sah -n "__fish_sah_using_subcommand model; and not __fish_seen_subcommand_from list show use help" -f -a "use" -d 'Use a specific model'
complete -c sah -n "__fish_sah_using_subcommand model; and not __fish_seen_subcommand_from list show use help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c sah -n "__fish_sah_using_subcommand model; and __fish_seen_subcommand_from list" -l format -d 'Output format' -r -f -a "table\t''
json\t''
yaml\t''"
complete -c sah -n "__fish_sah_using_subcommand model; and __fish_seen_subcommand_from list" -l model -d 'Override model for all use cases (runtime only, doesn\'t modify config)' -r
complete -c sah -n "__fish_sah_using_subcommand model; and __fish_seen_subcommand_from list" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c sah -n "__fish_sah_using_subcommand model; and __fish_seen_subcommand_from show" -l format -d 'Output format' -r -f -a "table\t''
json\t''
yaml\t''"
complete -c sah -n "__fish_sah_using_subcommand model; and __fish_seen_subcommand_from show" -l model -d 'Override model for all use cases (runtime only, doesn\'t modify config)' -r
complete -c sah -n "__fish_sah_using_subcommand model; and __fish_seen_subcommand_from show" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c sah -n "__fish_sah_using_subcommand model; and __fish_seen_subcommand_from use" -l model -d 'Override model for all use cases (runtime only, doesn\'t modify config)' -r
complete -c sah -n "__fish_sah_using_subcommand model; and __fish_seen_subcommand_from use" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c sah -n "__fish_sah_using_subcommand model; and __fish_seen_subcommand_from help" -f -a "list" -d 'List available models'
complete -c sah -n "__fish_sah_using_subcommand model; and __fish_seen_subcommand_from help" -f -a "show" -d 'Show current model use case assignments'
complete -c sah -n "__fish_sah_using_subcommand model; and __fish_seen_subcommand_from help" -f -a "use" -d 'Use a specific model'
complete -c sah -n "__fish_sah_using_subcommand model; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c sah -n "__fish_sah_using_subcommand agent; and not __fish_seen_subcommand_from acp help" -l model -d 'Override model for all use cases (runtime only, doesn\'t modify config)' -r
complete -c sah -n "__fish_sah_using_subcommand agent; and not __fish_seen_subcommand_from acp help" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c sah -n "__fish_sah_using_subcommand agent; and not __fish_seen_subcommand_from acp help" -f -a "acp" -d 'Start ACP server over stdio'
complete -c sah -n "__fish_sah_using_subcommand agent; and not __fish_seen_subcommand_from acp help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c sah -n "__fish_sah_using_subcommand agent; and __fish_seen_subcommand_from acp" -s c -l config -d 'Path to ACP configuration file (optional)' -r -F
complete -c sah -n "__fish_sah_using_subcommand agent; and __fish_seen_subcommand_from acp" -l permission-policy -d 'Permission policy: always-ask, auto-approve-reads' -r
complete -c sah -n "__fish_sah_using_subcommand agent; and __fish_seen_subcommand_from acp" -l allow-path -d 'Allowed filesystem paths (can be specified multiple times)' -r -F
complete -c sah -n "__fish_sah_using_subcommand agent; and __fish_seen_subcommand_from acp" -l block-path -d 'Blocked filesystem paths (can be specified multiple times)' -r -F
complete -c sah -n "__fish_sah_using_subcommand agent; and __fish_seen_subcommand_from acp" -l max-file-size -d 'Maximum file size for read operations in bytes' -r
complete -c sah -n "__fish_sah_using_subcommand agent; and __fish_seen_subcommand_from acp" -l terminal-buffer-size -d 'Terminal output buffer size in bytes' -r
complete -c sah -n "__fish_sah_using_subcommand agent; and __fish_seen_subcommand_from acp" -l graceful-shutdown-timeout -d 'Graceful shutdown timeout in seconds' -r
complete -c sah -n "__fish_sah_using_subcommand agent; and __fish_seen_subcommand_from acp" -l model -d 'Override model for all use cases (runtime only, doesn\'t modify config)' -r
complete -c sah -n "__fish_sah_using_subcommand agent; and __fish_seen_subcommand_from acp" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c sah -n "__fish_sah_using_subcommand agent; and __fish_seen_subcommand_from help" -f -a "acp" -d 'Start ACP server over stdio'
complete -c sah -n "__fish_sah_using_subcommand agent; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c sah -n "__fish_sah_using_subcommand help; and not __fish_seen_subcommand_from serve init deinit doctor prompt flow completion validate model agent help" -f -a "serve" -d 'Run as MCP server (default when invoked via stdio)'
complete -c sah -n "__fish_sah_using_subcommand help; and not __fish_seen_subcommand_from serve init deinit doctor prompt flow completion validate model agent help" -f -a "init" -d 'Initialize sah MCP server in Claude Code settings'
complete -c sah -n "__fish_sah_using_subcommand help; and not __fish_seen_subcommand_from serve init deinit doctor prompt flow completion validate model agent help" -f -a "deinit" -d 'Remove sah MCP server from Claude Code settings'
complete -c sah -n "__fish_sah_using_subcommand help; and not __fish_seen_subcommand_from serve init deinit doctor prompt flow completion validate model agent help" -f -a "doctor" -d 'Diagnose configuration and setup issues'
complete -c sah -n "__fish_sah_using_subcommand help; and not __fish_seen_subcommand_from serve init deinit doctor prompt flow completion validate model agent help" -f -a "prompt" -d 'Manage and test prompts'
complete -c sah -n "__fish_sah_using_subcommand help; and not __fish_seen_subcommand_from serve init deinit doctor prompt flow completion validate model agent help" -f -a "flow" -d 'Execute and manage workflows'
complete -c sah -n "__fish_sah_using_subcommand help; and not __fish_seen_subcommand_from serve init deinit doctor prompt flow completion validate model agent help" -f -a "completion" -d 'Generate shell completion scripts'
complete -c sah -n "__fish_sah_using_subcommand help; and not __fish_seen_subcommand_from serve init deinit doctor prompt flow completion validate model agent help" -f -a "validate" -d 'Validate prompt files and workflows for syntax and best practices'
complete -c sah -n "__fish_sah_using_subcommand help; and not __fish_seen_subcommand_from serve init deinit doctor prompt flow completion validate model agent help" -f -a "model" -d 'Manage and interact with models'
complete -c sah -n "__fish_sah_using_subcommand help; and not __fish_seen_subcommand_from serve init deinit doctor prompt flow completion validate model agent help" -f -a "agent" -d 'Manage and interact with Agent Client Protocol server'
complete -c sah -n "__fish_sah_using_subcommand help; and not __fish_seen_subcommand_from serve init deinit doctor prompt flow completion validate model agent help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c sah -n "__fish_sah_using_subcommand help; and __fish_seen_subcommand_from serve" -f -a "http" -d 'Start HTTP MCP server'
complete -c sah -n "__fish_sah_using_subcommand help; and __fish_seen_subcommand_from model" -f -a "list" -d 'List available models'
complete -c sah -n "__fish_sah_using_subcommand help; and __fish_seen_subcommand_from model" -f -a "show" -d 'Show current model use case assignments'
complete -c sah -n "__fish_sah_using_subcommand help; and __fish_seen_subcommand_from model" -f -a "use" -d 'Use a specific model'
complete -c sah -n "__fish_sah_using_subcommand help; and __fish_seen_subcommand_from agent" -f -a "acp" -d 'Start ACP server over stdio'
