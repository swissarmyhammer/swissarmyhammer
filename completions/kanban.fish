# Print an optspec for argparse to handle cmd's options that are independent of any subcommand.
function __fish_kanban_global_optspecs
	string join \n d/debug h/help V/version
end

function __fish_kanban_needs_command
	# Figure out if the current invocation already has a command.
	set -l cmd (commandline -opc)
	set -e cmd[1]
	argparse -s (__fish_kanban_global_optspecs) -- $cmd 2>/dev/null
	or return
	if set -q argv[1]
		# Also print the command, so this can be used to figure out what it is.
		echo $argv[1]
		return 1
	end
	return 0
end

function __fish_kanban_using_subcommand
	set -l cmd (__fish_kanban_needs_command)
	test -z "$cmd"
	and return 1
	contains -- $cmd[1] $argv
end

complete -c kanban -n "__fish_kanban_needs_command" -s d -l debug -d 'Enable debug output to stderr'
complete -c kanban -n "__fish_kanban_needs_command" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c kanban -n "__fish_kanban_needs_command" -s V -l version -d 'Print version'
complete -c kanban -n "__fish_kanban_needs_command" -f -a "serve" -d 'Run MCP server over stdio, exposing kanban tools'
complete -c kanban -n "__fish_kanban_needs_command" -f -a "init" -d 'Install kanban MCP server into Claude Code settings'
complete -c kanban -n "__fish_kanban_needs_command" -f -a "deinit" -d 'Remove kanban from Claude Code settings'
complete -c kanban -n "__fish_kanban_needs_command" -f -a "doctor" -d 'Diagnose kanban configuration and setup'
complete -c kanban -n "__fish_kanban_needs_command" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c kanban -n "__fish_kanban_using_subcommand serve" -s d -l debug -d 'Enable debug output to stderr'
complete -c kanban -n "__fish_kanban_using_subcommand serve" -s h -l help -d 'Print help'
complete -c kanban -n "__fish_kanban_using_subcommand init" -s d -l debug -d 'Enable debug output to stderr'
complete -c kanban -n "__fish_kanban_using_subcommand init" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c kanban -n "__fish_kanban_using_subcommand deinit" -s d -l debug -d 'Enable debug output to stderr'
complete -c kanban -n "__fish_kanban_using_subcommand deinit" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c kanban -n "__fish_kanban_using_subcommand doctor" -s v -l verbose -d 'Show detailed output including fix suggestions'
complete -c kanban -n "__fish_kanban_using_subcommand doctor" -s d -l debug -d 'Enable debug output to stderr'
complete -c kanban -n "__fish_kanban_using_subcommand doctor" -s h -l help -d 'Print help'
complete -c kanban -n "__fish_kanban_using_subcommand help; and not __fish_seen_subcommand_from serve init deinit doctor help" -f -a "serve" -d 'Run MCP server over stdio, exposing kanban tools'
complete -c kanban -n "__fish_kanban_using_subcommand help; and not __fish_seen_subcommand_from serve init deinit doctor help" -f -a "init" -d 'Install kanban MCP server into Claude Code settings'
complete -c kanban -n "__fish_kanban_using_subcommand help; and not __fish_seen_subcommand_from serve init deinit doctor help" -f -a "deinit" -d 'Remove kanban from Claude Code settings'
complete -c kanban -n "__fish_kanban_using_subcommand help; and not __fish_seen_subcommand_from serve init deinit doctor help" -f -a "doctor" -d 'Diagnose kanban configuration and setup'
complete -c kanban -n "__fish_kanban_using_subcommand help; and not __fish_seen_subcommand_from serve init deinit doctor help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
