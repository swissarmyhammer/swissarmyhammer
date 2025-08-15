Local prompts needs to be in `./swissarmyhammer/prompts` not in ./prompts.

Need to walk 'up' to root looking for `.swissarmyhammer` directories and accumulate prompts.

The deeper path prompts must override the shorter parent path prompts.

Directories in this source tree that have markdown like pland and doc should not be located.

it's jsut -- built ins, which need to be compiled in as resources, those are sourced from 

-promtps/builtin: least specific

-then ~/.swissarmyhammer/prompts

then ./.swissarmyhammer -- recursively looking up, with the current directory as most specific

cargo run validate is picking up
./issues/complete/list_errors.md
  ERROR [1:1] Missing YAML front matter delimiter
    💡 Start file with '---' to begin YAML front matter
