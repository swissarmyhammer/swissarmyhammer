_code-context() {
    local i cur prev opts cmd
    COMPREPLY=()
    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
        cur="$2"
    else
        cur="${COMP_WORDS[COMP_CWORD]}"
    fi
    prev="$3"
    cmd=""
    opts=""

    for i in "${COMP_WORDS[@]:0:COMP_CWORD}"
    do
        case "${cmd},${i}" in
            ",$1")
                cmd="code__context"
                ;;
            code__context,build)
                cmd="code__context__subcmd__build"
                ;;
            code__context,clear)
                cmd="code__context__subcmd__clear"
                ;;
            code__context,deinit)
                cmd="code__context__subcmd__deinit"
                ;;
            code__context,detect)
                cmd="code__context__subcmd__detect"
                ;;
            code__context,doctor)
                cmd="code__context__subcmd__doctor"
                ;;
            code__context,find)
                cmd="code__context__subcmd__find"
                ;;
            code__context,get)
                cmd="code__context__subcmd__get"
                ;;
            code__context,grep)
                cmd="code__context__subcmd__grep"
                ;;
            code__context,help)
                cmd="code__context__subcmd__help"
                ;;
            code__context,init)
                cmd="code__context__subcmd__init"
                ;;
            code__context,list)
                cmd="code__context__subcmd__list"
                ;;
            code__context,lsp)
                cmd="code__context__subcmd__lsp"
                ;;
            code__context,query)
                cmd="code__context__subcmd__query"
                ;;
            code__context,search)
                cmd="code__context__subcmd__search"
                ;;
            code__context,serve)
                cmd="code__context__subcmd__serve"
                ;;
            code__context,skill)
                cmd="code__context__subcmd__skill"
                ;;
            code__context__subcmd__build,help)
                cmd="code__context__subcmd__build__subcmd__help"
                ;;
            code__context__subcmd__build,status)
                cmd="code__context__subcmd__build__subcmd__status"
                ;;
            code__context__subcmd__build__subcmd__help,help)
                cmd="code__context__subcmd__build__subcmd__help__subcmd__help"
                ;;
            code__context__subcmd__build__subcmd__help,status)
                cmd="code__context__subcmd__build__subcmd__help__subcmd__status"
                ;;
            code__context__subcmd__clear,help)
                cmd="code__context__subcmd__clear__subcmd__help"
                ;;
            code__context__subcmd__clear,status)
                cmd="code__context__subcmd__clear__subcmd__status"
                ;;
            code__context__subcmd__clear__subcmd__help,help)
                cmd="code__context__subcmd__clear__subcmd__help__subcmd__help"
                ;;
            code__context__subcmd__clear__subcmd__help,status)
                cmd="code__context__subcmd__clear__subcmd__help__subcmd__status"
                ;;
            code__context__subcmd__detect,help)
                cmd="code__context__subcmd__detect__subcmd__help"
                ;;
            code__context__subcmd__detect,projects)
                cmd="code__context__subcmd__detect__subcmd__projects"
                ;;
            code__context__subcmd__detect__subcmd__help,help)
                cmd="code__context__subcmd__detect__subcmd__help__subcmd__help"
                ;;
            code__context__subcmd__detect__subcmd__help,projects)
                cmd="code__context__subcmd__detect__subcmd__help__subcmd__projects"
                ;;
            code__context__subcmd__find,duplicates)
                cmd="code__context__subcmd__find__subcmd__duplicates"
                ;;
            code__context__subcmd__find,help)
                cmd="code__context__subcmd__find__subcmd__help"
                ;;
            code__context__subcmd__find__subcmd__help,duplicates)
                cmd="code__context__subcmd__find__subcmd__help__subcmd__duplicates"
                ;;
            code__context__subcmd__find__subcmd__help,help)
                cmd="code__context__subcmd__find__subcmd__help__subcmd__help"
                ;;
            code__context__subcmd__get,blastradius)
                cmd="code__context__subcmd__get__subcmd__blastradius"
                ;;
            code__context__subcmd__get,callgraph)
                cmd="code__context__subcmd__get__subcmd__callgraph"
                ;;
            code__context__subcmd__get,code-actions)
                cmd="code__context__subcmd__get__subcmd__code__subcmd__actions"
                ;;
            code__context__subcmd__get,definition)
                cmd="code__context__subcmd__get__subcmd__definition"
                ;;
            code__context__subcmd__get,diagnostics)
                cmd="code__context__subcmd__get__subcmd__diagnostics"
                ;;
            code__context__subcmd__get,help)
                cmd="code__context__subcmd__get__subcmd__help"
                ;;
            code__context__subcmd__get,hover)
                cmd="code__context__subcmd__get__subcmd__hover"
                ;;
            code__context__subcmd__get,implementations)
                cmd="code__context__subcmd__get__subcmd__implementations"
                ;;
            code__context__subcmd__get,inbound-calls)
                cmd="code__context__subcmd__get__subcmd__inbound__subcmd__calls"
                ;;
            code__context__subcmd__get,references)
                cmd="code__context__subcmd__get__subcmd__references"
                ;;
            code__context__subcmd__get,rename-edits)
                cmd="code__context__subcmd__get__subcmd__rename__subcmd__edits"
                ;;
            code__context__subcmd__get,status)
                cmd="code__context__subcmd__get__subcmd__status"
                ;;
            code__context__subcmd__get,symbol)
                cmd="code__context__subcmd__get__subcmd__symbol"
                ;;
            code__context__subcmd__get,type-definition)
                cmd="code__context__subcmd__get__subcmd__type__subcmd__definition"
                ;;
            code__context__subcmd__get__subcmd__help,blastradius)
                cmd="code__context__subcmd__get__subcmd__help__subcmd__blastradius"
                ;;
            code__context__subcmd__get__subcmd__help,callgraph)
                cmd="code__context__subcmd__get__subcmd__help__subcmd__callgraph"
                ;;
            code__context__subcmd__get__subcmd__help,code-actions)
                cmd="code__context__subcmd__get__subcmd__help__subcmd__code__subcmd__actions"
                ;;
            code__context__subcmd__get__subcmd__help,definition)
                cmd="code__context__subcmd__get__subcmd__help__subcmd__definition"
                ;;
            code__context__subcmd__get__subcmd__help,diagnostics)
                cmd="code__context__subcmd__get__subcmd__help__subcmd__diagnostics"
                ;;
            code__context__subcmd__get__subcmd__help,help)
                cmd="code__context__subcmd__get__subcmd__help__subcmd__help"
                ;;
            code__context__subcmd__get__subcmd__help,hover)
                cmd="code__context__subcmd__get__subcmd__help__subcmd__hover"
                ;;
            code__context__subcmd__get__subcmd__help,implementations)
                cmd="code__context__subcmd__get__subcmd__help__subcmd__implementations"
                ;;
            code__context__subcmd__get__subcmd__help,inbound-calls)
                cmd="code__context__subcmd__get__subcmd__help__subcmd__inbound__subcmd__calls"
                ;;
            code__context__subcmd__get__subcmd__help,references)
                cmd="code__context__subcmd__get__subcmd__help__subcmd__references"
                ;;
            code__context__subcmd__get__subcmd__help,rename-edits)
                cmd="code__context__subcmd__get__subcmd__help__subcmd__rename__subcmd__edits"
                ;;
            code__context__subcmd__get__subcmd__help,status)
                cmd="code__context__subcmd__get__subcmd__help__subcmd__status"
                ;;
            code__context__subcmd__get__subcmd__help,symbol)
                cmd="code__context__subcmd__get__subcmd__help__subcmd__symbol"
                ;;
            code__context__subcmd__get__subcmd__help,type-definition)
                cmd="code__context__subcmd__get__subcmd__help__subcmd__type__subcmd__definition"
                ;;
            code__context__subcmd__grep,code)
                cmd="code__context__subcmd__grep__subcmd__code"
                ;;
            code__context__subcmd__grep,help)
                cmd="code__context__subcmd__grep__subcmd__help"
                ;;
            code__context__subcmd__grep__subcmd__help,code)
                cmd="code__context__subcmd__grep__subcmd__help__subcmd__code"
                ;;
            code__context__subcmd__grep__subcmd__help,help)
                cmd="code__context__subcmd__grep__subcmd__help__subcmd__help"
                ;;
            code__context__subcmd__help,build)
                cmd="code__context__subcmd__help__subcmd__build"
                ;;
            code__context__subcmd__help,clear)
                cmd="code__context__subcmd__help__subcmd__clear"
                ;;
            code__context__subcmd__help,deinit)
                cmd="code__context__subcmd__help__subcmd__deinit"
                ;;
            code__context__subcmd__help,detect)
                cmd="code__context__subcmd__help__subcmd__detect"
                ;;
            code__context__subcmd__help,doctor)
                cmd="code__context__subcmd__help__subcmd__doctor"
                ;;
            code__context__subcmd__help,find)
                cmd="code__context__subcmd__help__subcmd__find"
                ;;
            code__context__subcmd__help,get)
                cmd="code__context__subcmd__help__subcmd__get"
                ;;
            code__context__subcmd__help,grep)
                cmd="code__context__subcmd__help__subcmd__grep"
                ;;
            code__context__subcmd__help,help)
                cmd="code__context__subcmd__help__subcmd__help"
                ;;
            code__context__subcmd__help,init)
                cmd="code__context__subcmd__help__subcmd__init"
                ;;
            code__context__subcmd__help,list)
                cmd="code__context__subcmd__help__subcmd__list"
                ;;
            code__context__subcmd__help,lsp)
                cmd="code__context__subcmd__help__subcmd__lsp"
                ;;
            code__context__subcmd__help,query)
                cmd="code__context__subcmd__help__subcmd__query"
                ;;
            code__context__subcmd__help,search)
                cmd="code__context__subcmd__help__subcmd__search"
                ;;
            code__context__subcmd__help,serve)
                cmd="code__context__subcmd__help__subcmd__serve"
                ;;
            code__context__subcmd__help,skill)
                cmd="code__context__subcmd__help__subcmd__skill"
                ;;
            code__context__subcmd__help__subcmd__build,status)
                cmd="code__context__subcmd__help__subcmd__build__subcmd__status"
                ;;
            code__context__subcmd__help__subcmd__clear,status)
                cmd="code__context__subcmd__help__subcmd__clear__subcmd__status"
                ;;
            code__context__subcmd__help__subcmd__detect,projects)
                cmd="code__context__subcmd__help__subcmd__detect__subcmd__projects"
                ;;
            code__context__subcmd__help__subcmd__find,duplicates)
                cmd="code__context__subcmd__help__subcmd__find__subcmd__duplicates"
                ;;
            code__context__subcmd__help__subcmd__get,blastradius)
                cmd="code__context__subcmd__help__subcmd__get__subcmd__blastradius"
                ;;
            code__context__subcmd__help__subcmd__get,callgraph)
                cmd="code__context__subcmd__help__subcmd__get__subcmd__callgraph"
                ;;
            code__context__subcmd__help__subcmd__get,code-actions)
                cmd="code__context__subcmd__help__subcmd__get__subcmd__code__subcmd__actions"
                ;;
            code__context__subcmd__help__subcmd__get,definition)
                cmd="code__context__subcmd__help__subcmd__get__subcmd__definition"
                ;;
            code__context__subcmd__help__subcmd__get,diagnostics)
                cmd="code__context__subcmd__help__subcmd__get__subcmd__diagnostics"
                ;;
            code__context__subcmd__help__subcmd__get,hover)
                cmd="code__context__subcmd__help__subcmd__get__subcmd__hover"
                ;;
            code__context__subcmd__help__subcmd__get,implementations)
                cmd="code__context__subcmd__help__subcmd__get__subcmd__implementations"
                ;;
            code__context__subcmd__help__subcmd__get,inbound-calls)
                cmd="code__context__subcmd__help__subcmd__get__subcmd__inbound__subcmd__calls"
                ;;
            code__context__subcmd__help__subcmd__get,references)
                cmd="code__context__subcmd__help__subcmd__get__subcmd__references"
                ;;
            code__context__subcmd__help__subcmd__get,rename-edits)
                cmd="code__context__subcmd__help__subcmd__get__subcmd__rename__subcmd__edits"
                ;;
            code__context__subcmd__help__subcmd__get,status)
                cmd="code__context__subcmd__help__subcmd__get__subcmd__status"
                ;;
            code__context__subcmd__help__subcmd__get,symbol)
                cmd="code__context__subcmd__help__subcmd__get__subcmd__symbol"
                ;;
            code__context__subcmd__help__subcmd__get,type-definition)
                cmd="code__context__subcmd__help__subcmd__get__subcmd__type__subcmd__definition"
                ;;
            code__context__subcmd__help__subcmd__grep,code)
                cmd="code__context__subcmd__help__subcmd__grep__subcmd__code"
                ;;
            code__context__subcmd__help__subcmd__list,symbols)
                cmd="code__context__subcmd__help__subcmd__list__subcmd__symbols"
                ;;
            code__context__subcmd__help__subcmd__lsp,status)
                cmd="code__context__subcmd__help__subcmd__lsp__subcmd__status"
                ;;
            code__context__subcmd__help__subcmd__query,ast)
                cmd="code__context__subcmd__help__subcmd__query__subcmd__ast"
                ;;
            code__context__subcmd__help__subcmd__search,code)
                cmd="code__context__subcmd__help__subcmd__search__subcmd__code"
                ;;
            code__context__subcmd__help__subcmd__search,symbol)
                cmd="code__context__subcmd__help__subcmd__search__subcmd__symbol"
                ;;
            code__context__subcmd__help__subcmd__search,workspace-symbol)
                cmd="code__context__subcmd__help__subcmd__search__subcmd__workspace__subcmd__symbol"
                ;;
            code__context__subcmd__list,help)
                cmd="code__context__subcmd__list__subcmd__help"
                ;;
            code__context__subcmd__list,symbols)
                cmd="code__context__subcmd__list__subcmd__symbols"
                ;;
            code__context__subcmd__list__subcmd__help,help)
                cmd="code__context__subcmd__list__subcmd__help__subcmd__help"
                ;;
            code__context__subcmd__list__subcmd__help,symbols)
                cmd="code__context__subcmd__list__subcmd__help__subcmd__symbols"
                ;;
            code__context__subcmd__lsp,help)
                cmd="code__context__subcmd__lsp__subcmd__help"
                ;;
            code__context__subcmd__lsp,status)
                cmd="code__context__subcmd__lsp__subcmd__status"
                ;;
            code__context__subcmd__lsp__subcmd__help,help)
                cmd="code__context__subcmd__lsp__subcmd__help__subcmd__help"
                ;;
            code__context__subcmd__lsp__subcmd__help,status)
                cmd="code__context__subcmd__lsp__subcmd__help__subcmd__status"
                ;;
            code__context__subcmd__query,ast)
                cmd="code__context__subcmd__query__subcmd__ast"
                ;;
            code__context__subcmd__query,help)
                cmd="code__context__subcmd__query__subcmd__help"
                ;;
            code__context__subcmd__query__subcmd__help,ast)
                cmd="code__context__subcmd__query__subcmd__help__subcmd__ast"
                ;;
            code__context__subcmd__query__subcmd__help,help)
                cmd="code__context__subcmd__query__subcmd__help__subcmd__help"
                ;;
            code__context__subcmd__search,code)
                cmd="code__context__subcmd__search__subcmd__code"
                ;;
            code__context__subcmd__search,help)
                cmd="code__context__subcmd__search__subcmd__help"
                ;;
            code__context__subcmd__search,symbol)
                cmd="code__context__subcmd__search__subcmd__symbol"
                ;;
            code__context__subcmd__search,workspace-symbol)
                cmd="code__context__subcmd__search__subcmd__workspace__subcmd__symbol"
                ;;
            code__context__subcmd__search__subcmd__help,code)
                cmd="code__context__subcmd__search__subcmd__help__subcmd__code"
                ;;
            code__context__subcmd__search__subcmd__help,help)
                cmd="code__context__subcmd__search__subcmd__help__subcmd__help"
                ;;
            code__context__subcmd__search__subcmd__help,symbol)
                cmd="code__context__subcmd__search__subcmd__help__subcmd__symbol"
                ;;
            code__context__subcmd__search__subcmd__help,workspace-symbol)
                cmd="code__context__subcmd__search__subcmd__help__subcmd__workspace__subcmd__symbol"
                ;;
            *)
                ;;
        esac
    done

    case "${cmd}" in
        code__context)
            opts="-d -j -h -V --debug --json --help --version serve init deinit doctor skill get search list grep query find build clear lsp detect help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 1 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__build)
            opts="-d -j -h --debug --json --help status help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__build__subcmd__help)
            opts="status help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__build__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__build__subcmd__help__subcmd__status)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__build__subcmd__status)
            opts="-d -j -h --layer --debug --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --layer)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__clear)
            opts="-d -j -h --debug --json --help status help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__clear__subcmd__help)
            opts="status help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__clear__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__clear__subcmd__help__subcmd__status)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__clear__subcmd__status)
            opts="-d -j -h --debug --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__deinit)
            opts="-d -j -h --debug --json --help project local user"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__detect)
            opts="-d -j -h --debug --json --help projects help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__detect__subcmd__help)
            opts="projects help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__detect__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__detect__subcmd__help__subcmd__projects)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__detect__subcmd__projects)
            opts="-d -j -h --path --max-depth --include-guidelines --debug --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --path)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --max-depth)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --include-guidelines)
                    COMPREPLY=($(compgen -W "true false" -- "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__doctor)
            opts="-v -d -j -h --verbose --debug --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__find)
            opts="-d -j -h --debug --json --help duplicates help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__find__subcmd__duplicates)
            opts="-d -j -h --file-path --min-similarity --max-per-chunk --min-chunk-bytes --debug --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --file-path)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --min-similarity)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --max-per-chunk)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --min-chunk-bytes)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__find__subcmd__help)
            opts="duplicates help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__find__subcmd__help__subcmd__duplicates)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__find__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__get)
            opts="-d -j -h --debug --json --help symbol callgraph blastradius status definition type-definition hover references implementations code-actions inbound-calls rename-edits diagnostics help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__get__subcmd__blastradius)
            opts="-d -j -h --file-path --symbol --max-hops --debug --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --file-path)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --symbol)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --max-hops)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__get__subcmd__callgraph)
            opts="-d -j -h --symbol --direction --max-depth --debug --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --symbol)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --direction)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --max-depth)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__get__subcmd__code__subcmd__actions)
            opts="-d -j -h --file-path --start-line --start-character --end-line --end-character --filter-kind --debug --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --file-path)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --start-line)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --start-character)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --end-line)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --end-character)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --filter-kind)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__get__subcmd__definition)
            opts="-d -j -h --file-path --line --character --debug --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --file-path)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --line)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --character)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__get__subcmd__diagnostics)
            opts="-d -j -h --file-path --severity-filter --debug --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --file-path)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --severity-filter)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__get__subcmd__help)
            opts="symbol callgraph blastradius status definition type-definition hover references implementations code-actions inbound-calls rename-edits diagnostics help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__get__subcmd__help__subcmd__blastradius)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__get__subcmd__help__subcmd__callgraph)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__get__subcmd__help__subcmd__code__subcmd__actions)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__get__subcmd__help__subcmd__definition)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__get__subcmd__help__subcmd__diagnostics)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__get__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__get__subcmd__help__subcmd__hover)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__get__subcmd__help__subcmd__implementations)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__get__subcmd__help__subcmd__inbound__subcmd__calls)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__get__subcmd__help__subcmd__references)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__get__subcmd__help__subcmd__rename__subcmd__edits)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__get__subcmd__help__subcmd__status)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__get__subcmd__help__subcmd__symbol)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__get__subcmd__help__subcmd__type__subcmd__definition)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__get__subcmd__hover)
            opts="-d -j -h --file-path --line --character --debug --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --file-path)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --line)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --character)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__get__subcmd__implementations)
            opts="-d -j -h --file-path --line --character --max-results --debug --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --file-path)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --line)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --character)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --max-results)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__get__subcmd__inbound__subcmd__calls)
            opts="-d -j -h --file-path --line --character --depth --debug --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --file-path)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --line)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --character)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --depth)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__get__subcmd__references)
            opts="-d -j -h --file-path --line --character --include-declaration --max-results --debug --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --file-path)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --line)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --character)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --include-declaration)
                    COMPREPLY=($(compgen -W "true false" -- "${cur}"))
                    return 0
                    ;;
                --max-results)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__get__subcmd__rename__subcmd__edits)
            opts="-d -j -h --file-path --line --character --new-name --debug --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --file-path)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --line)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --character)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --new-name)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__get__subcmd__status)
            opts="-d -j -h --debug --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__get__subcmd__symbol)
            opts="-d -j -h --query --max-results --debug --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --query)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --max-results)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__get__subcmd__type__subcmd__definition)
            opts="-d -j -h --file-path --line --character --debug --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --file-path)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --line)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --character)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__grep)
            opts="-d -j -h --debug --json --help code help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__grep__subcmd__code)
            opts="-d -j -h --pattern --language --files --max-results --debug --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --pattern)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --language)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --files)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --max-results)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__grep__subcmd__help)
            opts="code help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__grep__subcmd__help__subcmd__code)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__grep__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__help)
            opts="serve init deinit doctor skill get search list grep query find build clear lsp detect help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__help__subcmd__build)
            opts="status"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__help__subcmd__build__subcmd__status)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__help__subcmd__clear)
            opts="status"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__help__subcmd__clear__subcmd__status)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__help__subcmd__deinit)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__help__subcmd__detect)
            opts="projects"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__help__subcmd__detect__subcmd__projects)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__help__subcmd__doctor)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__help__subcmd__find)
            opts="duplicates"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__help__subcmd__find__subcmd__duplicates)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__help__subcmd__get)
            opts="symbol callgraph blastradius status definition type-definition hover references implementations code-actions inbound-calls rename-edits diagnostics"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__help__subcmd__get__subcmd__blastradius)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__help__subcmd__get__subcmd__callgraph)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__help__subcmd__get__subcmd__code__subcmd__actions)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__help__subcmd__get__subcmd__definition)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__help__subcmd__get__subcmd__diagnostics)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__help__subcmd__get__subcmd__hover)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__help__subcmd__get__subcmd__implementations)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__help__subcmd__get__subcmd__inbound__subcmd__calls)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__help__subcmd__get__subcmd__references)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__help__subcmd__get__subcmd__rename__subcmd__edits)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__help__subcmd__get__subcmd__status)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__help__subcmd__get__subcmd__symbol)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__help__subcmd__get__subcmd__type__subcmd__definition)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__help__subcmd__grep)
            opts="code"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__help__subcmd__grep__subcmd__code)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__help__subcmd__init)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__help__subcmd__list)
            opts="symbols"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__help__subcmd__list__subcmd__symbols)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__help__subcmd__lsp)
            opts="status"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__help__subcmd__lsp__subcmd__status)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__help__subcmd__query)
            opts="ast"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__help__subcmd__query__subcmd__ast)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__help__subcmd__search)
            opts="symbol code workspace-symbol"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__help__subcmd__search__subcmd__code)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__help__subcmd__search__subcmd__symbol)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__help__subcmd__search__subcmd__workspace__subcmd__symbol)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__help__subcmd__serve)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__help__subcmd__skill)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__init)
            opts="-d -j -h --debug --json --help project local user"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__list)
            opts="-d -j -h --debug --json --help symbols help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__list__subcmd__help)
            opts="symbols help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__list__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__list__subcmd__help__subcmd__symbols)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__list__subcmd__symbols)
            opts="-d -j -h --file-path --debug --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --file-path)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__lsp)
            opts="-d -j -h --debug --json --help status help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__lsp__subcmd__help)
            opts="status help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__lsp__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__lsp__subcmd__help__subcmd__status)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__lsp__subcmd__status)
            opts="-d -j -h --debug --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__query)
            opts="-d -j -h --debug --json --help ast help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__query__subcmd__ast)
            opts="-d -j -h --query --language --files --max-results --debug --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --query)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --language)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --files)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --max-results)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__query__subcmd__help)
            opts="ast help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__query__subcmd__help__subcmd__ast)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__query__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__search)
            opts="-d -j -h --debug --json --help symbol code workspace-symbol help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__search__subcmd__code)
            opts="-d -j -h --query --top-k --min-similarity --file-pattern --language --debug --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --query)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --top-k)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --min-similarity)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --file-pattern)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --language)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__search__subcmd__help)
            opts="symbol code workspace-symbol help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__search__subcmd__help__subcmd__code)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__search__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__search__subcmd__help__subcmd__symbol)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__search__subcmd__help__subcmd__workspace__subcmd__symbol)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__search__subcmd__symbol)
            opts="-d -j -h --query --kind --max-results --debug --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --query)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --kind)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --max-results)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__search__subcmd__workspace__subcmd__symbol)
            opts="-d -j -h --query --max-results --debug --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --query)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --max-results)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__serve)
            opts="-d -j -h --debug --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        code__subcmd__context__subcmd__skill)
            opts="-d -j -h --debug --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
    esac
}

if [[ "${BASH_VERSINFO[0]}" -eq 4 && "${BASH_VERSINFO[1]}" -ge 4 || "${BASH_VERSINFO[0]}" -gt 4 ]]; then
    complete -F _code-context -o nosort -o bashdefault -o default code-context
else
    complete -F _code-context -o bashdefault -o default code-context
fi
