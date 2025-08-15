You have created an inconsistent mess loading prompts.


PromptResolver.load_all_prompts makes sense.

load_builtin_prompts is loading from files -- but no shit, and it said in the spec, these prompts need to be compiled in to the binary. There is a TODO that points to this in the code.

    #[allow(dead_code)]
    pub fn get_prompt_directories() -> Vec<PathBuf> {

this needs to go

in fact - all the dead code marked with #[allow(dead_code)]


needs to go

the validate command has it's on notion of prompt loading -- manually adding directories

-- WHY __ just make the prompt loading in the CLI consistent and load_all_prompts


`should_exclude_file` MUST NOT EXIST, this is a terrible approach -- just look in the correct directories