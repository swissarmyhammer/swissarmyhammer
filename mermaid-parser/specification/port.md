Create an intelligent port of [mermaid](https://github.com/mermaid-js/mermaid) to rust.

Our goal is just to parse in this crate, we will not be generating any graphics.

- create a new crate mermaid-parser
- use [chumsky](https://docs.rs/chumsky/latest/chumsky/)
- extract all the .jison grammars from the mermaid source
- at this point we are just making a plan, do not create any rust code yet
- for each grammar, create a `./plan/step_<grammar>.md` plan so we can implement one at a time
  - each plan step has a several sub steps
  - create an abstract syntax tree for using enums with values
  - create a lexer to extract tokens based on the jison
  - create a parser to create the abstract syntax tree based on the jison
  - identify all .mermaid files that use this grammar and copy them to `./test/<grammar>/ as individual files`
  - create a file case unit test with [rstest](https://docs.rs/rstest/latest/rstest/attr.rstest.html#files-path-as-input-arguments)

