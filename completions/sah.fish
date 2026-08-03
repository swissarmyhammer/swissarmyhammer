# Print an optspec for argparse to handle cmd's options that are independent of any subcommand.
function __fish_sah_global_optspecs
	string join \n v/verbose d/debug q/quiet format= h/help V/version
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
complete -c sah -n "__fish_sah_needs_command" -s v -l verbose -d 'Enable verbose logging'
complete -c sah -n "__fish_sah_needs_command" -s d -l debug -d 'Enable debug logging'
complete -c sah -n "__fish_sah_needs_command" -s q -l quiet -d 'Suppress all output except errors'
complete -c sah -n "__fish_sah_needs_command" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c sah -n "__fish_sah_needs_command" -s V -l version -d 'Print version'
complete -c sah -n "__fish_sah_needs_command" -f -a "serve" -d 'Run as MCP server (default when invoked via stdio)'
complete -c sah -n "__fish_sah_needs_command" -f -a "init" -d 'Set up sah for all detected AI coding agents (skills + MCP)'
complete -c sah -n "__fish_sah_needs_command" -f -a "deinit" -d 'Remove sah from all detected AI coding agents (skills + MCP)'
complete -c sah -n "__fish_sah_needs_command" -f -a "doctor" -d 'Diagnose configuration and setup issues'
complete -c sah -n "__fish_sah_needs_command" -f -a "completion" -d 'Generate shell completion scripts'
complete -c sah -n "__fish_sah_needs_command" -f -a "validate" -d 'Validate skills and workflows for syntax and best practices'
complete -c sah -n "__fish_sah_needs_command" -f -a "tools" -d 'Manage tool enable/disable state'
complete -c sah -n "__fish_sah_needs_command" -f -a "statusline" -d 'Render statusline from Claude Code JSON (stdin) or dump config'
complete -c sah -n "__fish_sah_needs_command" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c sah -n "__fish_sah_using_subcommand serve; and not __fish_seen_subcommand_from http help" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c sah -n "__fish_sah_using_subcommand serve; and not __fish_seen_subcommand_from http help" -f -a "http" -d 'Start HTTP MCP server'
complete -c sah -n "__fish_sah_using_subcommand serve; and not __fish_seen_subcommand_from http help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c sah -n "__fish_sah_using_subcommand serve; and __fish_seen_subcommand_from http" -s p -l port -d 'Port to bind to (use 0 for random port)' -r
complete -c sah -n "__fish_sah_using_subcommand serve; and __fish_seen_subcommand_from http" -s H -l host -d 'Host to bind to' -r
complete -c sah -n "__fish_sah_using_subcommand serve; and __fish_seen_subcommand_from http" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c sah -n "__fish_sah_using_subcommand serve; and __fish_seen_subcommand_from help" -f -a "http" -d 'Start HTTP MCP server'
complete -c sah -n "__fish_sah_using_subcommand serve; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c sah -n "__fish_sah_using_subcommand init" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c sah -n "__fish_sah_using_subcommand deinit" -l remove-directory -d 'Also remove .sah/ project directory'
complete -c sah -n "__fish_sah_using_subcommand deinit" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c sah -n "__fish_sah_using_subcommand doctor" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c sah -n "__fish_sah_using_subcommand completion" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c sah -n "__fish_sah_using_subcommand validate" -l format -d 'Output format' -r -f -a "table\t''
json\t''
yaml\t''"
complete -c sah -n "__fish_sah_using_subcommand validate" -s q -l quiet -d 'Suppress all output except errors. In quiet mode, warnings are hidden from both output and summary'
complete -c sah -n "__fish_sah_using_subcommand validate" -l validate-tools -d 'Validate MCP tool schemas for CLI compatibility'
complete -c sah -n "__fish_sah_using_subcommand validate" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c sah -n "__fish_sah_using_subcommand tools; and not __fish_seen_subcommand_from enable disable help" -l global -d 'Write to global config (~/.sah/tools.yaml) instead of project'
complete -c sah -n "__fish_sah_using_subcommand tools; and not __fish_seen_subcommand_from enable disable help" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c sah -n "__fish_sah_using_subcommand tools; and not __fish_seen_subcommand_from enable disable help" -f -a "enable" -d 'Enable tools (all if no names given)'
complete -c sah -n "__fish_sah_using_subcommand tools; and not __fish_seen_subcommand_from enable disable help" -f -a "disable" -d 'Disable tools (all if no names given)'
complete -c sah -n "__fish_sah_using_subcommand tools; and not __fish_seen_subcommand_from enable disable help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c sah -n "__fish_sah_using_subcommand tools; and __fish_seen_subcommand_from enable" -s h -l help -d 'Print help'
complete -c sah -n "__fish_sah_using_subcommand tools; and __fish_seen_subcommand_from disable" -s h -l help -d 'Print help'
complete -c sah -n "__fish_sah_using_subcommand tools; and __fish_seen_subcommand_from help" -f -a "enable" -d 'Enable tools (all if no names given)'
complete -c sah -n "__fish_sah_using_subcommand tools; and __fish_seen_subcommand_from help" -f -a "disable" -d 'Disable tools (all if no names given)'
complete -c sah -n "__fish_sah_using_subcommand tools; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c sah -n "__fish_sah_using_subcommand statusline; and not __fish_seen_subcommand_from config help" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c sah -n "__fish_sah_using_subcommand statusline; and not __fish_seen_subcommand_from config help" -f -a "config" -d 'Dump the full annotated builtin config to stdout'
complete -c sah -n "__fish_sah_using_subcommand statusline; and not __fish_seen_subcommand_from config help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c sah -n "__fish_sah_using_subcommand statusline; and __fish_seen_subcommand_from config" -s h -l help -d 'Print help'
complete -c sah -n "__fish_sah_using_subcommand statusline; and __fish_seen_subcommand_from help" -f -a "config" -d 'Dump the full annotated builtin config to stdout'
complete -c sah -n "__fish_sah_using_subcommand statusline; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c sah -n "__fish_sah_using_subcommand help; and not __fish_seen_subcommand_from serve init deinit doctor completion validate tools statusline help" -f -a "serve" -d 'Run as MCP server (default when invoked via stdio)'
complete -c sah -n "__fish_sah_using_subcommand help; and not __fish_seen_subcommand_from serve init deinit doctor completion validate tools statusline help" -f -a "init" -d 'Set up sah for all detected AI coding agents (skills + MCP)'
complete -c sah -n "__fish_sah_using_subcommand help; and not __fish_seen_subcommand_from serve init deinit doctor completion validate tools statusline help" -f -a "deinit" -d 'Remove sah from all detected AI coding agents (skills + MCP)'
complete -c sah -n "__fish_sah_using_subcommand help; and not __fish_seen_subcommand_from serve init deinit doctor completion validate tools statusline help" -f -a "doctor" -d 'Diagnose configuration and setup issues'
complete -c sah -n "__fish_sah_using_subcommand help; and not __fish_seen_subcommand_from serve init deinit doctor completion validate tools statusline help" -f -a "completion" -d 'Generate shell completion scripts'
complete -c sah -n "__fish_sah_using_subcommand help; and not __fish_seen_subcommand_from serve init deinit doctor completion validate tools statusline help" -f -a "validate" -d 'Validate skills and workflows for syntax and best practices'
complete -c sah -n "__fish_sah_using_subcommand help; and not __fish_seen_subcommand_from serve init deinit doctor completion validate tools statusline help" -f -a "tools" -d 'Manage tool enable/disable state'
complete -c sah -n "__fish_sah_using_subcommand help; and not __fish_seen_subcommand_from serve init deinit doctor completion validate tools statusline help" -f -a "statusline" -d 'Render statusline from Claude Code JSON (stdin) or dump config'
complete -c sah -n "__fish_sah_using_subcommand help; and not __fish_seen_subcommand_from serve init deinit doctor completion validate tools statusline help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c sah -n "__fish_sah_using_subcommand help; and __fish_seen_subcommand_from serve" -f -a "http" -d 'Start HTTP MCP server'
complete -c sah -n "__fish_sah_using_subcommand help; and __fish_seen_subcommand_from tools" -f -a "enable" -d 'Enable tools (all if no names given)'
complete -c sah -n "__fish_sah_using_subcommand help; and __fish_seen_subcommand_from tools" -f -a "disable" -d 'Disable tools (all if no names given)'
complete -c sah -n "__fish_sah_using_subcommand help; and __fish_seen_subcommand_from statusline" -f -a "config" -d 'Dump the full annotated builtin config to stdout'
