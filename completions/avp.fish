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
complete -c avp -n "__fish_avp_needs_command" -f -a "list" -d 'List all available validators'
complete -c avp -n "__fish_avp_needs_command" -f -a "login" -d 'Authenticate with the AVP registry'
complete -c avp -n "__fish_avp_needs_command" -f -a "logout" -d 'Log out from the AVP registry'
complete -c avp -n "__fish_avp_needs_command" -f -a "whoami" -d 'Show current authenticated user'
complete -c avp -n "__fish_avp_needs_command" -f -a "search" -d 'Search the AVP registry for packages'
complete -c avp -n "__fish_avp_needs_command" -f -a "info" -d 'Show detailed information about a package'
complete -c avp -n "__fish_avp_needs_command" -f -a "install" -d 'Install a package from the registry'
complete -c avp -n "__fish_avp_needs_command" -f -a "uninstall" -d 'Remove an installed package'
complete -c avp -n "__fish_avp_needs_command" -f -a "new" -d 'Create a new RuleSet from template'
complete -c avp -n "__fish_avp_needs_command" -f -a "publish" -d 'Publish a package to the registry'
complete -c avp -n "__fish_avp_needs_command" -f -a "unpublish" -d 'Remove a published package version from the registry'
complete -c avp -n "__fish_avp_needs_command" -f -a "outdated" -d 'Check for available package updates'
complete -c avp -n "__fish_avp_needs_command" -f -a "update" -d 'Update installed packages to latest versions'
complete -c avp -n "__fish_avp_needs_command" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c avp -n "__fish_avp_using_subcommand init" -s d -l debug -d 'Enable debug output to stderr'
complete -c avp -n "__fish_avp_using_subcommand init" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c avp -n "__fish_avp_using_subcommand deinit" -s d -l debug -d 'Enable debug output to stderr'
complete -c avp -n "__fish_avp_using_subcommand deinit" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c avp -n "__fish_avp_using_subcommand doctor" -s v -l verbose -d 'Show detailed output including fix suggestions'
complete -c avp -n "__fish_avp_using_subcommand doctor" -s d -l debug -d 'Enable debug output to stderr'
complete -c avp -n "__fish_avp_using_subcommand doctor" -s h -l help -d 'Print help'
complete -c avp -n "__fish_avp_using_subcommand list" -s v -l verbose -d 'Show detailed output including descriptions'
complete -c avp -n "__fish_avp_using_subcommand list" -l global -d 'Show only global (user-level) validators'
complete -c avp -n "__fish_avp_using_subcommand list" -l local -d 'Show only local (project-level) validators'
complete -c avp -n "__fish_avp_using_subcommand list" -l json -d 'Output as JSON'
complete -c avp -n "__fish_avp_using_subcommand list" -s d -l debug -d 'Enable debug output to stderr'
complete -c avp -n "__fish_avp_using_subcommand list" -s h -l help -d 'Print help'
complete -c avp -n "__fish_avp_using_subcommand login" -s d -l debug -d 'Enable debug output to stderr'
complete -c avp -n "__fish_avp_using_subcommand login" -s h -l help -d 'Print help'
complete -c avp -n "__fish_avp_using_subcommand logout" -s d -l debug -d 'Enable debug output to stderr'
complete -c avp -n "__fish_avp_using_subcommand logout" -s h -l help -d 'Print help'
complete -c avp -n "__fish_avp_using_subcommand whoami" -s d -l debug -d 'Enable debug output to stderr'
complete -c avp -n "__fish_avp_using_subcommand whoami" -s h -l help -d 'Print help'
complete -c avp -n "__fish_avp_using_subcommand search" -l tag -d 'Filter by tag' -r
complete -c avp -n "__fish_avp_using_subcommand search" -l json -d 'Output as JSON'
complete -c avp -n "__fish_avp_using_subcommand search" -s d -l debug -d 'Enable debug output to stderr'
complete -c avp -n "__fish_avp_using_subcommand search" -s h -l help -d 'Print help'
complete -c avp -n "__fish_avp_using_subcommand info" -s d -l debug -d 'Enable debug output to stderr'
complete -c avp -n "__fish_avp_using_subcommand info" -s h -l help -d 'Print help'
complete -c avp -n "__fish_avp_using_subcommand install" -l local -l project -d 'Install to project (.avp/validators/) [default]'
complete -c avp -n "__fish_avp_using_subcommand install" -l global -l user -d 'Install globally (~/.avp/validators/)'
complete -c avp -n "__fish_avp_using_subcommand install" -s d -l debug -d 'Enable debug output to stderr'
complete -c avp -n "__fish_avp_using_subcommand install" -s h -l help -d 'Print help'
complete -c avp -n "__fish_avp_using_subcommand uninstall" -l local -l project -d 'Remove from project (.avp/validators/) [default]'
complete -c avp -n "__fish_avp_using_subcommand uninstall" -l global -l user -d 'Remove from global (~/.avp/validators/)'
complete -c avp -n "__fish_avp_using_subcommand uninstall" -s d -l debug -d 'Enable debug output to stderr'
complete -c avp -n "__fish_avp_using_subcommand uninstall" -s h -l help -d 'Print help'
complete -c avp -n "__fish_avp_using_subcommand new" -l local -l project -d 'Create in project (.avp/validators/) [default]'
complete -c avp -n "__fish_avp_using_subcommand new" -l global -l user -d 'Create in user-level directory (~/.avp/validators/)'
complete -c avp -n "__fish_avp_using_subcommand new" -s d -l debug -d 'Enable debug output to stderr'
complete -c avp -n "__fish_avp_using_subcommand new" -s h -l help -d 'Print help'
complete -c avp -n "__fish_avp_using_subcommand publish" -l dry-run -d 'Validate and show what would be published without uploading'
complete -c avp -n "__fish_avp_using_subcommand publish" -s d -l debug -d 'Enable debug output to stderr'
complete -c avp -n "__fish_avp_using_subcommand publish" -s h -l help -d 'Print help'
complete -c avp -n "__fish_avp_using_subcommand unpublish" -s d -l debug -d 'Enable debug output to stderr'
complete -c avp -n "__fish_avp_using_subcommand unpublish" -s h -l help -d 'Print help'
complete -c avp -n "__fish_avp_using_subcommand outdated" -s d -l debug -d 'Enable debug output to stderr'
complete -c avp -n "__fish_avp_using_subcommand outdated" -s h -l help -d 'Print help'
complete -c avp -n "__fish_avp_using_subcommand update" -l local -l project -d 'Update project packages [default]'
complete -c avp -n "__fish_avp_using_subcommand update" -l global -l user -d 'Update global (~/.avp/validators/) packages'
complete -c avp -n "__fish_avp_using_subcommand update" -s d -l debug -d 'Enable debug output to stderr'
complete -c avp -n "__fish_avp_using_subcommand update" -s h -l help -d 'Print help'
complete -c avp -n "__fish_avp_using_subcommand help; and not __fish_seen_subcommand_from init deinit doctor list login logout whoami search info install uninstall new publish unpublish outdated update help" -f -a "init" -d 'Install AVP hooks into Claude Code settings'
complete -c avp -n "__fish_avp_using_subcommand help; and not __fish_seen_subcommand_from init deinit doctor list login logout whoami search info install uninstall new publish unpublish outdated update help" -f -a "deinit" -d 'Remove AVP hooks from Claude Code settings and delete .avp directory'
complete -c avp -n "__fish_avp_using_subcommand help; and not __fish_seen_subcommand_from init deinit doctor list login logout whoami search info install uninstall new publish unpublish outdated update help" -f -a "doctor" -d 'Diagnose AVP configuration and setup'
complete -c avp -n "__fish_avp_using_subcommand help; and not __fish_seen_subcommand_from init deinit doctor list login logout whoami search info install uninstall new publish unpublish outdated update help" -f -a "list" -d 'List all available validators'
complete -c avp -n "__fish_avp_using_subcommand help; and not __fish_seen_subcommand_from init deinit doctor list login logout whoami search info install uninstall new publish unpublish outdated update help" -f -a "login" -d 'Authenticate with the AVP registry'
complete -c avp -n "__fish_avp_using_subcommand help; and not __fish_seen_subcommand_from init deinit doctor list login logout whoami search info install uninstall new publish unpublish outdated update help" -f -a "logout" -d 'Log out from the AVP registry'
complete -c avp -n "__fish_avp_using_subcommand help; and not __fish_seen_subcommand_from init deinit doctor list login logout whoami search info install uninstall new publish unpublish outdated update help" -f -a "whoami" -d 'Show current authenticated user'
complete -c avp -n "__fish_avp_using_subcommand help; and not __fish_seen_subcommand_from init deinit doctor list login logout whoami search info install uninstall new publish unpublish outdated update help" -f -a "search" -d 'Search the AVP registry for packages'
complete -c avp -n "__fish_avp_using_subcommand help; and not __fish_seen_subcommand_from init deinit doctor list login logout whoami search info install uninstall new publish unpublish outdated update help" -f -a "info" -d 'Show detailed information about a package'
complete -c avp -n "__fish_avp_using_subcommand help; and not __fish_seen_subcommand_from init deinit doctor list login logout whoami search info install uninstall new publish unpublish outdated update help" -f -a "install" -d 'Install a package from the registry'
complete -c avp -n "__fish_avp_using_subcommand help; and not __fish_seen_subcommand_from init deinit doctor list login logout whoami search info install uninstall new publish unpublish outdated update help" -f -a "uninstall" -d 'Remove an installed package'
complete -c avp -n "__fish_avp_using_subcommand help; and not __fish_seen_subcommand_from init deinit doctor list login logout whoami search info install uninstall new publish unpublish outdated update help" -f -a "new" -d 'Create a new RuleSet from template'
complete -c avp -n "__fish_avp_using_subcommand help; and not __fish_seen_subcommand_from init deinit doctor list login logout whoami search info install uninstall new publish unpublish outdated update help" -f -a "publish" -d 'Publish a package to the registry'
complete -c avp -n "__fish_avp_using_subcommand help; and not __fish_seen_subcommand_from init deinit doctor list login logout whoami search info install uninstall new publish unpublish outdated update help" -f -a "unpublish" -d 'Remove a published package version from the registry'
complete -c avp -n "__fish_avp_using_subcommand help; and not __fish_seen_subcommand_from init deinit doctor list login logout whoami search info install uninstall new publish unpublish outdated update help" -f -a "outdated" -d 'Check for available package updates'
complete -c avp -n "__fish_avp_using_subcommand help; and not __fish_seen_subcommand_from init deinit doctor list login logout whoami search info install uninstall new publish unpublish outdated update help" -f -a "update" -d 'Update installed packages to latest versions'
complete -c avp -n "__fish_avp_using_subcommand help; and not __fish_seen_subcommand_from init deinit doctor list login logout whoami search info install uninstall new publish unpublish outdated update help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
