//! Shared mock [`Operation`] fixtures for in-crate tests.
//!
//! A single canonical set of test-double operations used by both the
//! `schema` and `cli_gen` test modules, so the mock-op contract is defined
//! once instead of being re-declared per test module.
//!
//! The set models a small board tool:
//! - `board` → `init`, `get`, `update`
//! - `task` → `add`, `get`
//! - `tasks` → `list`
//! - `column` → `add`
//! - `tag` → `add`
//!
//! It deliberately spans the variations the tests exercise: a required string
//! arg (`board init --name`), the same arg optional elsewhere (`board update
//! --name`), an array arg (`task add --assignees`), a boolean filter
//! (`tasks list --ready`), and verbs that reuse a name across nouns (`add`).

use crate::{Operation, ParamMeta, ParamType};

/// `board init` — required `name`, optional `description`.
pub struct MockInitBoard;
static MOCK_INIT_BOARD_PARAMS: &[ParamMeta] = &[
    ParamMeta::new("name")
        .description("The board name")
        .param_type(ParamType::String)
        .required(),
    ParamMeta::new("description")
        .description("Optional board description")
        .param_type(ParamType::String),
];
impl Operation for MockInitBoard {
    fn verb(&self) -> &'static str {
        "init"
    }
    fn noun(&self) -> &'static str {
        "board"
    }
    fn description(&self) -> &'static str {
        "Initialize a new board"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        MOCK_INIT_BOARD_PARAMS
    }
}

/// `board get` — no parameters.
pub struct MockGetBoard;
static MOCK_GET_BOARD_PARAMS: &[ParamMeta] = &[];
impl Operation for MockGetBoard {
    fn verb(&self) -> &'static str {
        "get"
    }
    fn noun(&self) -> &'static str {
        "board"
    }
    fn description(&self) -> &'static str {
        "Get the board"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        MOCK_GET_BOARD_PARAMS
    }
}

/// `board update` — optional `name` (the same name is required on `board init`).
pub struct MockUpdateBoard;
static MOCK_UPDATE_BOARD_PARAMS: &[ParamMeta] = &[ParamMeta::new("name")
    .description("New board name")
    .param_type(ParamType::String)];
impl Operation for MockUpdateBoard {
    fn verb(&self) -> &'static str {
        "update"
    }
    fn noun(&self) -> &'static str {
        "board"
    }
    fn description(&self) -> &'static str {
        "Update the board"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        MOCK_UPDATE_BOARD_PARAMS
    }
}

/// `task add` — required `title`, optional `description`, array `assignees`.
pub struct MockAddTask;
static MOCK_ADD_TASK_PARAMS: &[ParamMeta] = &[
    ParamMeta::new("title")
        .description("Task title")
        .param_type(ParamType::String)
        .required(),
    ParamMeta::new("description")
        .description("Task description")
        .param_type(ParamType::String),
    ParamMeta::new("assignees")
        .description("Assignees for the task")
        .param_type(ParamType::Array),
];
impl Operation for MockAddTask {
    fn verb(&self) -> &'static str {
        "add"
    }
    fn noun(&self) -> &'static str {
        "task"
    }
    fn description(&self) -> &'static str {
        "Create a new task"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        MOCK_ADD_TASK_PARAMS
    }
}

/// `task get` — required `id`.
pub struct MockGetTask;
static MOCK_GET_TASK_PARAMS: &[ParamMeta] = &[ParamMeta::new("id")
    .description("Task ID")
    .param_type(ParamType::String)
    .required()];
impl Operation for MockGetTask {
    fn verb(&self) -> &'static str {
        "get"
    }
    fn noun(&self) -> &'static str {
        "task"
    }
    fn description(&self) -> &'static str {
        "Get a task"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        MOCK_GET_TASK_PARAMS
    }
}

/// `tasks list` — optional `assignee` filter and boolean `ready` filter.
pub struct MockListTasks;
static MOCK_LIST_TASKS_PARAMS: &[ParamMeta] = &[
    ParamMeta::new("assignee")
        .description("Filter by assignee")
        .param_type(ParamType::String),
    ParamMeta::new("ready")
        .description("Filter by ready status")
        .param_type(ParamType::Boolean),
];
impl Operation for MockListTasks {
    fn verb(&self) -> &'static str {
        "list"
    }
    fn noun(&self) -> &'static str {
        "tasks"
    }
    fn description(&self) -> &'static str {
        "List all tasks"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        MOCK_LIST_TASKS_PARAMS
    }
}

/// `column add` — required `id`.
pub struct MockAddColumn;
static MOCK_ADD_COLUMN_PARAMS: &[ParamMeta] = &[ParamMeta::new("id")
    .description("Column id")
    .param_type(ParamType::String)
    .required()];
impl Operation for MockAddColumn {
    fn verb(&self) -> &'static str {
        "add"
    }
    fn noun(&self) -> &'static str {
        "column"
    }
    fn description(&self) -> &'static str {
        "Add a column"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        MOCK_ADD_COLUMN_PARAMS
    }
}

/// `tag add` — required `name`.
pub struct MockAddTag;
static MOCK_ADD_TAG_PARAMS: &[ParamMeta] = &[ParamMeta::new("name")
    .description("Tag name")
    .param_type(ParamType::String)
    .required()];
impl Operation for MockAddTag {
    fn verb(&self) -> &'static str {
        "add"
    }
    fn noun(&self) -> &'static str {
        "tag"
    }
    fn description(&self) -> &'static str {
        "Add a tag"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        MOCK_ADD_TAG_PARAMS
    }
}
