# Print an optspec for argparse to handle cmd's options that are independent of any subcommand.
function __fish_mirdan_global_optspecs
	string join \n d/debug y/yes agent= h/help V/version
end

function __fish_mirdan_needs_command
	# Figure out if the current invocation already has a command.
	set -l cmd (commandline -opc)
	set -e cmd[1]
	argparse -s (__fish_mirdan_global_optspecs) -- $cmd 2>/dev/null
	or return
	if set -q argv[1]
		# Also print the command, so this can be used to figure out what it is.
		echo $argv[1]
		return 1
	end
	return 0
end

function __fish_mirdan_using_subcommand
	set -l cmd (__fish_mirdan_needs_command)
	test -z "$cmd"
	and return 1
	contains -- $cmd[1] $argv
end

complete -c mirdan -n "__fish_mirdan_needs_command" -l agent -d 'Limit operations to a single agent (e.g. claude-code, cursor)' -r
complete -c mirdan -n "__fish_mirdan_needs_command" -s d -l debug -d 'Enable debug output to stderr'
complete -c mirdan -n "__fish_mirdan_needs_command" -s y -l yes -d 'Skip confirmation prompts (useful for CI/CD)'
complete -c mirdan -n "__fish_mirdan_needs_command" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c mirdan -n "__fish_mirdan_needs_command" -s V -l version -d 'Print version'
complete -c mirdan -n "__fish_mirdan_needs_command" -f -a "agents" -d 'Detect and list installed AI coding agents'
complete -c mirdan -n "__fish_mirdan_needs_command" -f -a "new" -d 'Create a new skill or validator from template'
complete -c mirdan -n "__fish_mirdan_needs_command" -f -a "install" -d 'Install a skill or validator package (type auto-detected from contents)'
complete -c mirdan -n "__fish_mirdan_needs_command" -f -a "uninstall" -d 'Remove an installed skill or validator package'
complete -c mirdan -n "__fish_mirdan_needs_command" -f -a "list" -d 'List installed skills and validators'
complete -c mirdan -n "__fish_mirdan_needs_command" -f -a "search" -d 'Search the registry for skills and validators'
complete -c mirdan -n "__fish_mirdan_needs_command" -f -a "info" -d 'Show detailed information about a package'
complete -c mirdan -n "__fish_mirdan_needs_command" -f -a "login" -d 'Authenticate with the registry'
complete -c mirdan -n "__fish_mirdan_needs_command" -f -a "logout" -d 'Log out from the registry and revoke token'
complete -c mirdan -n "__fish_mirdan_needs_command" -f -a "whoami" -d 'Show current authenticated user'
complete -c mirdan -n "__fish_mirdan_needs_command" -f -a "publish" -d 'Publish a skill or validator to the registry (type auto-detected)'
complete -c mirdan -n "__fish_mirdan_needs_command" -f -a "unpublish" -d 'Remove a published package version from the registry'
complete -c mirdan -n "__fish_mirdan_needs_command" -f -a "outdated" -d 'Check for available package updates'
complete -c mirdan -n "__fish_mirdan_needs_command" -f -a "update" -d 'Update installed packages to latest versions'
complete -c mirdan -n "__fish_mirdan_needs_command" -f -a "doctor" -d 'Diagnose Mirdan setup and configuration'
complete -c mirdan -n "__fish_mirdan_needs_command" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c mirdan -n "__fish_mirdan_using_subcommand agents" -l agent -d 'Limit operations to a single agent (e.g. claude-code, cursor)' -r
complete -c mirdan -n "__fish_mirdan_using_subcommand agents" -l all -d 'Show all known agents, not just detected ones'
complete -c mirdan -n "__fish_mirdan_using_subcommand agents" -l json -d 'Output as JSON'
complete -c mirdan -n "__fish_mirdan_using_subcommand agents" -s d -l debug -d 'Enable debug output to stderr'
complete -c mirdan -n "__fish_mirdan_using_subcommand agents" -s y -l yes -d 'Skip confirmation prompts (useful for CI/CD)'
complete -c mirdan -n "__fish_mirdan_using_subcommand agents" -s h -l help -d 'Print help'
complete -c mirdan -n "__fish_mirdan_using_subcommand new; and not __fish_seen_subcommand_from skill validator help" -l agent -d 'Limit operations to a single agent (e.g. claude-code, cursor)' -r
complete -c mirdan -n "__fish_mirdan_using_subcommand new; and not __fish_seen_subcommand_from skill validator help" -s d -l debug -d 'Enable debug output to stderr'
complete -c mirdan -n "__fish_mirdan_using_subcommand new; and not __fish_seen_subcommand_from skill validator help" -s y -l yes -d 'Skip confirmation prompts (useful for CI/CD)'
complete -c mirdan -n "__fish_mirdan_using_subcommand new; and not __fish_seen_subcommand_from skill validator help" -s h -l help -d 'Print help'
complete -c mirdan -n "__fish_mirdan_using_subcommand new; and not __fish_seen_subcommand_from skill validator help" -f -a "skill" -d 'Scaffold a new skill (agentskills.io spec)'
complete -c mirdan -n "__fish_mirdan_using_subcommand new; and not __fish_seen_subcommand_from skill validator help" -f -a "validator" -d 'Scaffold a new validator (AVP spec)'
complete -c mirdan -n "__fish_mirdan_using_subcommand new; and not __fish_seen_subcommand_from skill validator help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c mirdan -n "__fish_mirdan_using_subcommand new; and __fish_seen_subcommand_from skill" -l agent -d 'Limit operations to a single agent (e.g. claude-code, cursor)' -r
complete -c mirdan -n "__fish_mirdan_using_subcommand new; and __fish_seen_subcommand_from skill" -l global -d 'Create in agent global skill directories instead of project-level'
complete -c mirdan -n "__fish_mirdan_using_subcommand new; and __fish_seen_subcommand_from skill" -s d -l debug -d 'Enable debug output to stderr'
complete -c mirdan -n "__fish_mirdan_using_subcommand new; and __fish_seen_subcommand_from skill" -s y -l yes -d 'Skip confirmation prompts (useful for CI/CD)'
complete -c mirdan -n "__fish_mirdan_using_subcommand new; and __fish_seen_subcommand_from skill" -s h -l help -d 'Print help'
complete -c mirdan -n "__fish_mirdan_using_subcommand new; and __fish_seen_subcommand_from validator" -l agent -d 'Limit operations to a single agent (e.g. claude-code, cursor)' -r
complete -c mirdan -n "__fish_mirdan_using_subcommand new; and __fish_seen_subcommand_from validator" -l global -d 'Create in ~/.avp/validators/ instead of .avp/validators/'
complete -c mirdan -n "__fish_mirdan_using_subcommand new; and __fish_seen_subcommand_from validator" -s d -l debug -d 'Enable debug output to stderr'
complete -c mirdan -n "__fish_mirdan_using_subcommand new; and __fish_seen_subcommand_from validator" -s y -l yes -d 'Skip confirmation prompts (useful for CI/CD)'
complete -c mirdan -n "__fish_mirdan_using_subcommand new; and __fish_seen_subcommand_from validator" -s h -l help -d 'Print help'
complete -c mirdan -n "__fish_mirdan_using_subcommand new; and __fish_seen_subcommand_from help" -f -a "skill" -d 'Scaffold a new skill (agentskills.io spec)'
complete -c mirdan -n "__fish_mirdan_using_subcommand new; and __fish_seen_subcommand_from help" -f -a "validator" -d 'Scaffold a new validator (AVP spec)'
complete -c mirdan -n "__fish_mirdan_using_subcommand new; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c mirdan -n "__fish_mirdan_using_subcommand install" -l skill -d 'Install a specific skill/validator by name from a multi-package repo' -r
complete -c mirdan -n "__fish_mirdan_using_subcommand install" -l agent -d 'Limit operations to a single agent (e.g. claude-code, cursor)' -r
complete -c mirdan -n "__fish_mirdan_using_subcommand install" -l global -d 'Install globally (~/.avp/validators/ for validators, agent global dirs for skills)'
complete -c mirdan -n "__fish_mirdan_using_subcommand install" -l git -d 'Treat package as a git URL (clone instead of registry lookup)'
complete -c mirdan -n "__fish_mirdan_using_subcommand install" -s d -l debug -d 'Enable debug output to stderr'
complete -c mirdan -n "__fish_mirdan_using_subcommand install" -s y -l yes -d 'Skip confirmation prompts (useful for CI/CD)'
complete -c mirdan -n "__fish_mirdan_using_subcommand install" -s h -l help -d 'Print help'
complete -c mirdan -n "__fish_mirdan_using_subcommand uninstall" -l agent -d 'Limit operations to a single agent (e.g. claude-code, cursor)' -r
complete -c mirdan -n "__fish_mirdan_using_subcommand uninstall" -l global -d 'Remove from global locations'
complete -c mirdan -n "__fish_mirdan_using_subcommand uninstall" -s d -l debug -d 'Enable debug output to stderr'
complete -c mirdan -n "__fish_mirdan_using_subcommand uninstall" -s y -l yes -d 'Skip confirmation prompts (useful for CI/CD)'
complete -c mirdan -n "__fish_mirdan_using_subcommand uninstall" -s h -l help -d 'Print help'
complete -c mirdan -n "__fish_mirdan_using_subcommand list" -l agent -d 'Limit operations to a single agent (e.g. claude-code, cursor)' -r
complete -c mirdan -n "__fish_mirdan_using_subcommand list" -l skills -d 'Show only skills'
complete -c mirdan -n "__fish_mirdan_using_subcommand list" -l validators -d 'Show only validators'
complete -c mirdan -n "__fish_mirdan_using_subcommand list" -l json -d 'Output as JSON'
complete -c mirdan -n "__fish_mirdan_using_subcommand list" -s d -l debug -d 'Enable debug output to stderr'
complete -c mirdan -n "__fish_mirdan_using_subcommand list" -s y -l yes -d 'Skip confirmation prompts (useful for CI/CD)'
complete -c mirdan -n "__fish_mirdan_using_subcommand list" -s h -l help -d 'Print help'
complete -c mirdan -n "__fish_mirdan_using_subcommand search" -l agent -d 'Limit operations to a single agent (e.g. claude-code, cursor)' -r
complete -c mirdan -n "__fish_mirdan_using_subcommand search" -l json -d 'Output as JSON'
complete -c mirdan -n "__fish_mirdan_using_subcommand search" -s d -l debug -d 'Enable debug output to stderr'
complete -c mirdan -n "__fish_mirdan_using_subcommand search" -s y -l yes -d 'Skip confirmation prompts (useful for CI/CD)'
complete -c mirdan -n "__fish_mirdan_using_subcommand search" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c mirdan -n "__fish_mirdan_using_subcommand info" -l agent -d 'Limit operations to a single agent (e.g. claude-code, cursor)' -r
complete -c mirdan -n "__fish_mirdan_using_subcommand info" -s d -l debug -d 'Enable debug output to stderr'
complete -c mirdan -n "__fish_mirdan_using_subcommand info" -s y -l yes -d 'Skip confirmation prompts (useful for CI/CD)'
complete -c mirdan -n "__fish_mirdan_using_subcommand info" -s h -l help -d 'Print help'
complete -c mirdan -n "__fish_mirdan_using_subcommand login" -l agent -d 'Limit operations to a single agent (e.g. claude-code, cursor)' -r
complete -c mirdan -n "__fish_mirdan_using_subcommand login" -s d -l debug -d 'Enable debug output to stderr'
complete -c mirdan -n "__fish_mirdan_using_subcommand login" -s y -l yes -d 'Skip confirmation prompts (useful for CI/CD)'
complete -c mirdan -n "__fish_mirdan_using_subcommand login" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c mirdan -n "__fish_mirdan_using_subcommand logout" -l agent -d 'Limit operations to a single agent (e.g. claude-code, cursor)' -r
complete -c mirdan -n "__fish_mirdan_using_subcommand logout" -s d -l debug -d 'Enable debug output to stderr'
complete -c mirdan -n "__fish_mirdan_using_subcommand logout" -s y -l yes -d 'Skip confirmation prompts (useful for CI/CD)'
complete -c mirdan -n "__fish_mirdan_using_subcommand logout" -s h -l help -d 'Print help'
complete -c mirdan -n "__fish_mirdan_using_subcommand whoami" -l agent -d 'Limit operations to a single agent (e.g. claude-code, cursor)' -r
complete -c mirdan -n "__fish_mirdan_using_subcommand whoami" -s d -l debug -d 'Enable debug output to stderr'
complete -c mirdan -n "__fish_mirdan_using_subcommand whoami" -s y -l yes -d 'Skip confirmation prompts (useful for CI/CD)'
complete -c mirdan -n "__fish_mirdan_using_subcommand whoami" -s h -l help -d 'Print help'
complete -c mirdan -n "__fish_mirdan_using_subcommand publish" -l agent -d 'Limit operations to a single agent (e.g. claude-code, cursor)' -r
complete -c mirdan -n "__fish_mirdan_using_subcommand publish" -l dry-run -d 'Validate and show what would be published without uploading'
complete -c mirdan -n "__fish_mirdan_using_subcommand publish" -s d -l debug -d 'Enable debug output to stderr'
complete -c mirdan -n "__fish_mirdan_using_subcommand publish" -s y -l yes -d 'Skip confirmation prompts (useful for CI/CD)'
complete -c mirdan -n "__fish_mirdan_using_subcommand publish" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c mirdan -n "__fish_mirdan_using_subcommand unpublish" -l agent -d 'Limit operations to a single agent (e.g. claude-code, cursor)' -r
complete -c mirdan -n "__fish_mirdan_using_subcommand unpublish" -s d -l debug -d 'Enable debug output to stderr'
complete -c mirdan -n "__fish_mirdan_using_subcommand unpublish" -s y -l yes -d 'Skip confirmation prompts (useful for CI/CD)'
complete -c mirdan -n "__fish_mirdan_using_subcommand unpublish" -s h -l help -d 'Print help'
complete -c mirdan -n "__fish_mirdan_using_subcommand outdated" -l agent -d 'Limit operations to a single agent (e.g. claude-code, cursor)' -r
complete -c mirdan -n "__fish_mirdan_using_subcommand outdated" -s d -l debug -d 'Enable debug output to stderr'
complete -c mirdan -n "__fish_mirdan_using_subcommand outdated" -s y -l yes -d 'Skip confirmation prompts (useful for CI/CD)'
complete -c mirdan -n "__fish_mirdan_using_subcommand outdated" -s h -l help -d 'Print help'
complete -c mirdan -n "__fish_mirdan_using_subcommand update" -l agent -d 'Limit operations to a single agent (e.g. claude-code, cursor)' -r
complete -c mirdan -n "__fish_mirdan_using_subcommand update" -l global -d 'Update global packages'
complete -c mirdan -n "__fish_mirdan_using_subcommand update" -s d -l debug -d 'Enable debug output to stderr'
complete -c mirdan -n "__fish_mirdan_using_subcommand update" -s y -l yes -d 'Skip confirmation prompts (useful for CI/CD)'
complete -c mirdan -n "__fish_mirdan_using_subcommand update" -s h -l help -d 'Print help'
complete -c mirdan -n "__fish_mirdan_using_subcommand doctor" -l agent -d 'Limit operations to a single agent (e.g. claude-code, cursor)' -r
complete -c mirdan -n "__fish_mirdan_using_subcommand doctor" -s v -l verbose -d 'Show detailed output including fix suggestions'
complete -c mirdan -n "__fish_mirdan_using_subcommand doctor" -s d -l debug -d 'Enable debug output to stderr'
complete -c mirdan -n "__fish_mirdan_using_subcommand doctor" -s y -l yes -d 'Skip confirmation prompts (useful for CI/CD)'
complete -c mirdan -n "__fish_mirdan_using_subcommand doctor" -s h -l help -d 'Print help'
complete -c mirdan -n "__fish_mirdan_using_subcommand help; and not __fish_seen_subcommand_from agents new install uninstall list search info login logout whoami publish unpublish outdated update doctor help" -f -a "agents" -d 'Detect and list installed AI coding agents'
complete -c mirdan -n "__fish_mirdan_using_subcommand help; and not __fish_seen_subcommand_from agents new install uninstall list search info login logout whoami publish unpublish outdated update doctor help" -f -a "new" -d 'Create a new skill or validator from template'
complete -c mirdan -n "__fish_mirdan_using_subcommand help; and not __fish_seen_subcommand_from agents new install uninstall list search info login logout whoami publish unpublish outdated update doctor help" -f -a "install" -d 'Install a skill or validator package (type auto-detected from contents)'
complete -c mirdan -n "__fish_mirdan_using_subcommand help; and not __fish_seen_subcommand_from agents new install uninstall list search info login logout whoami publish unpublish outdated update doctor help" -f -a "uninstall" -d 'Remove an installed skill or validator package'
complete -c mirdan -n "__fish_mirdan_using_subcommand help; and not __fish_seen_subcommand_from agents new install uninstall list search info login logout whoami publish unpublish outdated update doctor help" -f -a "list" -d 'List installed skills and validators'
complete -c mirdan -n "__fish_mirdan_using_subcommand help; and not __fish_seen_subcommand_from agents new install uninstall list search info login logout whoami publish unpublish outdated update doctor help" -f -a "search" -d 'Search the registry for skills and validators'
complete -c mirdan -n "__fish_mirdan_using_subcommand help; and not __fish_seen_subcommand_from agents new install uninstall list search info login logout whoami publish unpublish outdated update doctor help" -f -a "info" -d 'Show detailed information about a package'
complete -c mirdan -n "__fish_mirdan_using_subcommand help; and not __fish_seen_subcommand_from agents new install uninstall list search info login logout whoami publish unpublish outdated update doctor help" -f -a "login" -d 'Authenticate with the registry'
complete -c mirdan -n "__fish_mirdan_using_subcommand help; and not __fish_seen_subcommand_from agents new install uninstall list search info login logout whoami publish unpublish outdated update doctor help" -f -a "logout" -d 'Log out from the registry and revoke token'
complete -c mirdan -n "__fish_mirdan_using_subcommand help; and not __fish_seen_subcommand_from agents new install uninstall list search info login logout whoami publish unpublish outdated update doctor help" -f -a "whoami" -d 'Show current authenticated user'
complete -c mirdan -n "__fish_mirdan_using_subcommand help; and not __fish_seen_subcommand_from agents new install uninstall list search info login logout whoami publish unpublish outdated update doctor help" -f -a "publish" -d 'Publish a skill or validator to the registry (type auto-detected)'
complete -c mirdan -n "__fish_mirdan_using_subcommand help; and not __fish_seen_subcommand_from agents new install uninstall list search info login logout whoami publish unpublish outdated update doctor help" -f -a "unpublish" -d 'Remove a published package version from the registry'
complete -c mirdan -n "__fish_mirdan_using_subcommand help; and not __fish_seen_subcommand_from agents new install uninstall list search info login logout whoami publish unpublish outdated update doctor help" -f -a "outdated" -d 'Check for available package updates'
complete -c mirdan -n "__fish_mirdan_using_subcommand help; and not __fish_seen_subcommand_from agents new install uninstall list search info login logout whoami publish unpublish outdated update doctor help" -f -a "update" -d 'Update installed packages to latest versions'
complete -c mirdan -n "__fish_mirdan_using_subcommand help; and not __fish_seen_subcommand_from agents new install uninstall list search info login logout whoami publish unpublish outdated update doctor help" -f -a "doctor" -d 'Diagnose Mirdan setup and configuration'
complete -c mirdan -n "__fish_mirdan_using_subcommand help; and not __fish_seen_subcommand_from agents new install uninstall list search info login logout whoami publish unpublish outdated update doctor help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c mirdan -n "__fish_mirdan_using_subcommand help; and __fish_seen_subcommand_from new" -f -a "skill" -d 'Scaffold a new skill (agentskills.io spec)'
complete -c mirdan -n "__fish_mirdan_using_subcommand help; and __fish_seen_subcommand_from new" -f -a "validator" -d 'Scaffold a new validator (AVP spec)'
