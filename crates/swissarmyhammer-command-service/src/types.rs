//! Public types for the `command` operation tool.
//!
//! These types form the registration payload and execution context shared
//! between the host's `CommandService` and command-registering plugins. They
//! are designed to mirror every field today's command YAML supports, so
//! built-in plugins can convert their YAML manifests into
//! [`CommandRegistration`] without losing fidelity.

use rmcp::schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Opaque marker for a callback exposed by the registering plugin.
///
/// The plugin SDK strips function values from the registration payload
/// before sending it across the host/plugin boundary, replacing each with a
/// `{ "$callback": "cb_..." }` object. The service stores these markers and
/// later sends `notifications/callbacks/invoke` back to the registering
/// isolate when the command is run.
///
/// The marker is structurally a single string field named `$callback`. The
/// custom Serialize/Deserialize impls preserve the exact `$callback` wire
/// shape (raw serde-derive on a struct named `Callback` would emit
/// `{"$callback": ...}` only with a hand-written rename, which is fragile;
/// implementing serde manually keeps the wire format pinned).
#[derive(Debug, Clone, PartialEq, Eq, JsonSchema)]
pub struct CallbackMarker {
    /// The callback id assigned by the SDK (e.g. `"cb_42"`). Opaque to the
    /// host — the platform's callback dispatcher resolves it back to the
    /// originating isolate.
    pub callback_id: String,
}

impl CallbackMarker {
    /// Create a new callback marker with the given id.
    pub fn new(callback_id: impl Into<String>) -> Self {
        Self {
            callback_id: callback_id.into(),
        }
    }
}

impl Serialize for CallbackMarker {
    /// Serializes as `{ "$callback": "<id>" }` — the exact wire shape the
    /// plugin SDK emits.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_entry("$callback", &self.callback_id)?;
        map.end()
    }
}

impl<'de> Deserialize<'de> for CallbackMarker {
    /// Deserializes from `{ "$callback": "<id>" }`. Any other shape is an
    /// error — callers should never see a partially-formed marker.
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Wire {
            #[serde(rename = "$callback")]
            callback: String,
        }
        let Wire { callback } = Wire::deserialize(deserializer)?;
        Ok(CallbackMarker {
            callback_id: callback,
        })
    }
}

/// Where a parameter value comes from at command-invoke time.
///
/// Mirrors the YAML `from:` field on existing command parameters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ParamSource {
    /// Resolved from the scope chain (e.g. the active task entity).
    ScopeChain,
    /// Resolved from the context-menu target (the entity the menu fired
    /// over).
    Target,
    /// Provided in the args bag at dispatch time.
    Args,
    /// Falls back to the param's `default` literal.
    Default,
}

/// Shape of a parameter for runtime UI collection.
///
/// Mirrors today's `shape:` field, which tells the frontend how to collect
/// the value when it isn't already resolvable from the scope chain or
/// target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ParamShape {
    /// User picks from a list of options (resolver- or inline-supplied).
    Enum,
    /// Single-line free text.
    Text,
    /// Multiline expression (e.g. filter DSL).
    Expression,
    /// Numeric input.
    Number,
    /// Date input.
    Date,
    /// Boolean toggle.
    Boolean,
}

/// One option value for an enum-shaped param.
///
/// Used as an inline alternative to a backend `options_from` resolver, for
/// option lists known at YAML write time (e.g. sort directions).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ParamOption {
    /// Machine-readable value that flows into the args bag.
    pub value: String,
    /// Human-readable label shown in the picker UI.
    pub label: String,
}

/// A parameter definition for a registered command.
///
/// Mirrors the YAML `params:` entries, including the rich resolver/picker
/// metadata (`shape`, `options_from`, `options`, `clear_command`) used by
/// the frontend's `<CommandPopover>` and `<CommandButton>` surfaces.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ParamDef {
    /// Param name (also the key in the args bag at dispatch time).
    pub name: String,
    /// Where the value comes from at command-invoke time.
    pub from: ParamSource,
    /// For scope-chain / target params, the entity type that must be
    /// resolvable from that source (e.g. `"task"`, `"column"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity_type: Option<String>,
    /// Default literal value used when `from` is `Default` or when the
    /// designated source is absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<Value>,
    /// UI collection shape for this param. `None` means the param is
    /// resolved from the scope chain / target / default and the runtime
    /// never asks the user for it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape: Option<ParamShape>,
    /// Backend resolver name that supplies enum options at
    /// `commands_for_scope` emission time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options_from: Option<String>,
    /// Inline option list for enum-shaped params whose values are static
    /// and known at registration time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<ParamOption>>,
    /// Sibling command id to dispatch in place of this command when the
    /// user picks the "clear" sentinel (empty-string value) for an
    /// enum-shaped param. Surfaces a "(none)" affordance inside the
    /// picker popover.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clear_command: Option<String>,
}

/// The execution context passed to `available` and `execute` callbacks.
///
/// Carries the dispatch surface a command needs to resolve its params:
/// the scope chain (active entities), the context-menu target (if any),
/// and the free-form args bag the palette / popover / menu populated.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CommandContext {
    /// Active scope monikers (e.g. `["board:01ABC", "task:42"]`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scope_chain: Vec<String>,
    /// Context-menu target moniker (the entity the menu fired over).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// Free-form args bag populated by the dispatching surface.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub args: HashMap<String, Value>,
}

/// A public, callback-free projection of [`CommandRegistration`].
///
/// Returned by `list command` and `schema command` so the palette / menu
/// / hotkey systems can render commands without seeing the
/// [`CallbackMarker`]s (which are only meaningful to the registering
/// isolate).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CommandMetadata {
    /// Stable command id.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Optional display name override for native menus.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub menu_name: Option<String>,
    /// Optional long-form description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Optional category.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    /// Scope expression list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<Vec<String>>,
    /// Keybindings keyed by keymap mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keys: Option<HashMap<String, String>>,
    /// Native menu-bar placement payload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub menu: Option<Value>,
    /// Whether the command appears in the context menu.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_menu: Option<bool>,
    /// Context-menu group bucket.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_menu_group: Option<u32>,
    /// Sort order within the context-menu group.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_menu_order: Option<u32>,
    /// Tab-button affordance payload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab_button: Option<Value>,
    /// View-kind UI-surface filter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view_kinds: Option<Vec<String>>,
    /// Whether the command is undoable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub undoable: Option<bool>,
    /// Whether the command is visible in surfaces.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible: Option<bool>,
    /// Param definitions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<Vec<ParamDef>>,
}

/// Stable wrapper around a registered command's param schema.
///
/// Returned by `schema command` so the wire shape can grow new fields
/// (e.g. `version`, `examples`) without breaking palette / popover callers.
/// The `params` array is the exact list the command was registered with —
/// `None` for commands that take no dispatch-time arguments.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CommandSchema {
    /// Stable command id this schema belongs to.
    pub id: String,
    /// Param definitions as registered. `None` means the command takes
    /// no dispatch-time arguments.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<Vec<ParamDef>>,
}

impl CommandSchema {
    /// Project a [`crate::RegisterCommand`] registration payload onto its
    /// schema-only public shape.
    ///
    /// Used by `schema command` to return only the dispatch-relevant
    /// metadata (the params array) without copying the rest of the
    /// registration.
    pub fn from_registration(reg: &crate::RegisterCommand) -> Self {
        Self {
            id: reg.id.clone(),
            params: reg.params.clone(),
        }
    }
}

impl CommandMetadata {
    /// Project a [`crate::RegisterCommand`] registration payload onto its
    /// public, callback-free shape.
    ///
    /// Used by `list command` and `schema command` to return registration
    /// data without exposing the [`CallbackMarker`]s — those are only
    /// meaningful to the registering isolate.
    pub fn from_registration(reg: &crate::RegisterCommand) -> Self {
        Self {
            id: reg.id.clone(),
            name: reg.name.clone(),
            menu_name: reg.menu_name.clone(),
            description: reg.description.clone(),
            category: reg.category.clone(),
            scope: reg.scope.clone(),
            keys: reg.keys.clone(),
            menu: reg.menu.clone(),
            context_menu: reg.context_menu,
            context_menu_group: reg.context_menu_group,
            context_menu_order: reg.context_menu_order,
            tab_button: reg.tab_button.clone(),
            view_kinds: reg.view_kinds.clone(),
            undoable: reg.undoable,
            visible: reg.visible,
            params: reg.params.clone(),
        }
    }
}

/// Error types returned by the `command` operation tool.
///
/// Structured so downstream callers (palette, menu, dispatcher) can
/// branch on the variant rather than parsing error strings.
#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    /// No command with the requested id is registered.
    #[error("unknown command: {id}")]
    UnknownCommand {
        /// The id that was looked up.
        id: String,
    },
    /// The command is registered but its `available` callback returned
    /// `false` or a non-ok reason.
    #[error("command unavailable: {reason}")]
    CommandUnavailable {
        /// Caller-supplied reason from the `available` callback.
        reason: String,
    },
    /// The `available` or `execute` callback failed in the registering
    /// isolate (transport error, runtime error, thrown exception).
    #[error("callback failed: {message}")]
    CallbackFailed {
        /// Message describing the failure.
        message: String,
    },
    /// The `available` callback exceeded its soft latency budget and was
    /// force-cancelled.
    #[error("latency budget exceeded for command {id}")]
    LatencyBudgetExceeded {
        /// The id of the command whose `available` check timed out.
        id: String,
    },
    /// The registration payload's `id` field was empty. Command ids must
    /// be non-empty so callers can address the command unambiguously.
    #[error("registration rejected: `id` must be non-empty")]
    EmptyId,
    /// The registration payload's `name` field was empty. Command names
    /// must be non-empty so they can render in the palette and menus.
    #[error("registration rejected: `name` must be non-empty for command {id}")]
    EmptyName {
        /// The id of the registration whose name was empty.
        id: String,
    },
    /// The registration payload's required `execute` callback was missing
    /// — the SDK should always populate it before sending the payload
    /// across the host/plugin boundary. A missing or empty `$callback` id
    /// signals an SDK or serializer bug.
    #[error("registration rejected: required `execute` callback is missing for command {id}")]
    MissingExecuteCallback {
        /// The id of the registration whose `execute` callback was absent.
        id: String,
    },
    /// The registration payload supplied an `available` callback marker
    /// but its `$callback` id was empty. Like
    /// [`Self::MissingExecuteCallback`] this signals an SDK or serializer
    /// bug — when present, the marker must carry a routable id. Storing
    /// an empty id would silently surface as an opaque dispatch failure
    /// later, so the service rejects it at registration time.
    #[error(
        "registration rejected: optional `available` callback marker is empty for command {id}"
    )]
    MissingAvailableCallback {
        /// The id of the registration whose `available` callback id was
        /// empty.
        id: String,
    },
}
