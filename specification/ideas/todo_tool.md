  - Todo (this is a new tool group parallel to issues, but works on a named todo file parameter)
    - use the todo list to keep track of work while
    - this is not the same as an issue, in that the todo list is ephemeral and never checked in
    - store the todo list in a yaml nested list format, you will very likely have multiline text for context
      - use sequential ulid generation for the id
      ```yaml
      todo:
        - id: 01K1KQM85501ECE8XJGNZKNJQW
          task: "Implement file read tool"
          context: "Use cline's readTool.ts for inspiration"
          done: true
        - id: 01K1KQM85501ECE8XJGNZKNJQX
          task: "Add glob support"
          context: "Refer to qwen-code glob.ts"
          done: false
        - id: 01K1KQM85501ECE8XJGNZKNJQY
          task: "Integrate ripgrep for grep"
          context: "Improve search performance"
          done: false
        - id: 01K1KQM85501ECE8XJGNZKNJQZ
          task: "Write documentation"
          context: "Describe usage for each tool"
          done: false
      ```
    - Todo (todo list, thing to do, additional context)
      - add a new item to the todo list
      - auto creates the todo list file if it does not yet exist
    - Doing (todo list, UUID or "next")
      - UUID: read the whole item from the todo list as yaml
      - "next": read the very next todo that is not done as yaml - only one at a time, this forces a FIFO and avoids context pollution with too much to do
    - Done (todo list, UUID)
      - remove a done item from the todo list, leaving the work still to be done