# Print an optspec for argparse to handle cmd's options that are independent of any subcommand.
function __fish_avp_global_optspecs
	string join \n d/debug h/help V/version
end

function __fish_avp_needs_command
	# Figure out if the current invocation already has a command.
	set -l cmd (commandline -opc)
	set -e cmd[1]
	argparse -s (__fish_avp_global_optspecs) -- $cmd 2>/dev/null
	or return
	if set -q argv[1]
		# Also print the command, so this can be used to figure out what it is.
		echo $argv[1]
		return 1
	end
	return 0
end

function __fish_avp_using_subcommand
	set -l cmd (__fish_avp_needs_command)
	test -z "$cmd"
	and return 1
	contains -- $cmd[1] $argv
end

complete -c avp -n "__fish_avp_needs_command" -s d -l debug -d 'Enable debug output to stderr'
complete -c avp -n "__fish_avp_needs_command" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c avp -n "__fish_avp_needs_command" -s V -l version -d 'Print version'
complete -c avp -n "__fish_avp_needs_command" -f -a "init" -d 'Install AVP hooks into Claude Code settings'
complete -c avp -n "__fish_avp_needs_command" -f -a "deinit" -d 'Remove AVP hooks from Claude Code settings and delete .avp directory'
complete -c avp -n "__fish_avp_needs_command" -f -a "doctor" -d 'Diagnose AVP configuration and setup'
complete -c avp -n "__fish_avp_needs_command" -f -a "edit" -d 'Edit an existing RuleSet in $EDITOR'
complete -c avp -n "__fish_avp_needs_command" -f -a "new" -d 'Create a new RuleSet from template'
complete -c avp -n "__fish_avp_needs_command" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c avp -n "__fish_avp_using_subcommand init" -s d -l debug -d 'Enable debug output to stderr'
complete -c avp -n "__fish_avp_using_subcommand init" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c avp -n "__fish_avp_using_subcommand deinit" -s d -l debug -d 'Enable debug output to stderr'
complete -c avp -n "__fish_avp_using_subcommand deinit" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c avp -n "__fish_avp_using_subcommand doctor" -s v -l verbose -d 'Show detailed output including fix suggestions'
complete -c avp -n "__fish_avp_using_subcommand doctor" -s d -l debug -d 'Enable debug output to stderr'
complete -c avp -n "__fish_avp_using_subcommand doctor" -s h -l help -d 'Print help'
complete -c avp -n "__fish_avp_using_subcommand edit" -l local -l project -d 'Edit in project (.avp/validators/) [default]'
complete -c avp -n "__fish_avp_using_subcommand edit" -l global -l user -d 'Edit in user-level directory (~/.avp/validators/)'
complete -c avp -n "__fish_avp_using_subcommand edit" -s d -l debug -d 'Enable debug output to stderr'
complete -c avp -n "__fish_avp_using_subcommand edit" -s h -l help -d 'Print help'
complete -c avp -n "__fish_avp_using_subcommand new" -l local -l project -d 'Create in project (.avp/validators/) [default]'
complete -c avp -n "__fish_avp_using_subcommand new" -l global -l user -d 'Create in user-level directory (~/.avp/validators/)'
complete -c avp -n "__fish_avp_using_subcommand new" -s d -l debug -d 'Enable debug output to stderr'
complete -c avp -n "__fish_avp_using_subcommand new" -s h -l help -d 'Print help'
complete -c avp -n "__fish_avp_using_subcommand help; and not __fish_seen_subcommand_from init deinit doctor edit new help" -f -a "init" -d 'Install AVP hooks into Claude Code settings'
complete -c avp -n "__fish_avp_using_subcommand help; and not __fish_seen_subcommand_from init deinit doctor edit new help" -f -a "deinit" -d 'Remove AVP hooks from Claude Code settings and delete .avp directory'
complete -c avp -n "__fish_avp_using_subcommand help; and not __fish_seen_subcommand_from init deinit doctor edit new help" -f -a "doctor" -d 'Diagnose AVP configuration and setup'
complete -c avp -n "__fish_avp_using_subcommand help; and not __fish_seen_subcommand_from init deinit doctor edit new help" -f -a "edit" -d 'Edit an existing RuleSet in $EDITOR'
complete -c avp -n "__fish_avp_using_subcommand help; and not __fish_seen_subcommand_from init deinit doctor edit new help" -f -a "new" -d 'Create a new RuleSet from template'
complete -c avp -n "__fish_avp_using_subcommand help; and not __fish_seen_subcommand_from init deinit doctor edit new help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
