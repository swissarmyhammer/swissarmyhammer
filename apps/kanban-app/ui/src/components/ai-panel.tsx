/**
 * The AI panel — the conversation surface of the kanban app.
 *
 * `AiPanel` renders a live ACP conversation: streamed assistant markdown,
 * collapsible reasoning, tool-call cards, the agent's plan, and an inline
 * permission prompt — all on top of {@link useConversation}, which folds the
 * ACP `session/update` stream into renderable `UIMessage` state. A header
 * model selector and a {@link PromptInput} composer complete the surface.
 *
 * # A View, not a Container
 *
 * Per `ARCHITECTURE.md`'s Container/View split, `AiPanel` is a View: it takes
 * props and renders, and it never calls the Tauri backend directly. The two
 * backend seams it needs — the model list and starting an agent — are injected:
 *
 * - `models` / `modelId` / `onSelectModel` — the panel's hosting container
 *   (a later task) fetches `ai_list_models`, persists the per-board choice in
 *   `UIState`, and feeds the selected id back as a prop.
 * - `createConnect` — given a model id, returns a {@link ConversationConnect}
 *   factory. {@link aiPanelConnectFactory} is the production implementation,
 *   composing `ai_start_agent` -> {@link connectAcpStream} ->
 *   {@link createKanbanClient}; tests inject a mock that needs no transport.
 *
 * # Switching models starts a fresh session
 *
 * The conversation is keyed on the selected model id. Selecting a different
 * model remounts {@link AiPanelConversation}, which tears down the prior ACP
 * client and session and starts a brand-new stateless one for the new model —
 * exactly the "fresh session per model" the task requires.
 */
import {
  useCallback,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import type {
  ContentBlock,
  PlanEntry,
  PlanEntryStatus,
  RequestPermissionRequest,
} from "@agentclientprotocol/sdk";
import type { DynamicToolUIPart, ToolUIPart } from "ai";
import {
  CheckIcon,
  ChevronDownIcon,
  CircleCheckIcon,
  CircleDotIcon,
  CircleIcon,
  CopyIcon,
  PlusIcon,
  RotateCcwIcon,
  SparklesIcon,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  AiPanelFocusScope,
  AiPanelPressable,
} from "@/components/ai-panel-focus";
import { asSegment } from "@/types/spatial";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { cn } from "@/lib/utils";
import { createKanbanClient } from "@/ai/acp-client";
import { connectAcpStream } from "@/ai/acp-stream";
import {
  useConversation,
  type ConversationConnect,
  type ConversationMessage,
  type PlanPartData,
} from "@/ai/conversation";
import { registerAiCommandHandlers, setAiStreaming } from "@/ai/commands";
import {
  Conversation,
  ConversationContent,
  ConversationEmptyState,
  ConversationScrollButton,
} from "@/components/ai-elements/conversation";
import { Loader } from "@/components/ai-elements/loader";
import {
  Message,
  MessageActions,
  MessageContent,
  MessageResponse,
} from "@/components/ai-elements/message";
import {
  PromptInput,
  PromptInputBody,
  PromptInputFooter,
  PromptInputSubmit,
  PromptInputTextarea,
  type PromptInputMessage,
} from "@/components/ai-elements/prompt-input";
import {
  Reasoning,
  ReasoningContent,
  ReasoningTrigger,
} from "@/components/ai-elements/reasoning";
import {
  Task,
  TaskContent,
  TaskItem,
  TaskTrigger,
} from "@/components/ai-elements/task";
import {
  Tool,
  ToolContent,
  ToolHeader,
  ToolInput,
  ToolOutput,
} from "@/components/ai-elements/tool";

/**
 * A model the user can pick, mirroring the Rust `Model` struct that
 * `ai_list_models` returns (camelCased on the wire).
 */
export interface AiModel {
  /** Stable id — passed to `ai_start_agent` to select this model. */
  id: string;
  /** Human-readable label for the picker. */
  label: string;
  /** Which agent backend this model drives. */
  kind: "claude-code" | "local-llama";
  /** Whether the model can be started right now. */
  available: boolean;
  /** Optional note — the "install Claude Code" hint, or a description. */
  hint: string | null;
}

/**
 * Builds the {@link ConversationConnect} factory for a chosen model.
 *
 * Given a model id, returns the `connect` factory {@link useConversation}
 * calls on the first `sendPrompt`. The factory receives the hook's
 * session-update and permission handlers and must return a connected
 * {@link KanbanAcpClient} (or `null` when the agent cannot be started).
 *
 * {@link aiPanelConnectFactory} is the production implementation;
 * tests inject a mock.
 */
export type AiPanelConnectFactory = (modelId: string) => ConversationConnect;

/** The two endpoint URLs `ai_start_agent` hands back, camelCased on the wire. */
export interface AgentEndpoint {
  /** Loopback `ws://127.0.0.1:<port>` URL for the in-process ACP agent. */
  wsUrl: string;
  /** The board's full-SAH-toolset MCP URL, or `null` when the board has none. */
  mcpUrl: string | null;
}

/**
 * Build the production {@link AiPanelConnectFactory}.
 *
 * The returned factory composes the real handoff: `startAgent(modelId)` yields
 * the loopback `ws://` and `mcp` URLs, {@link connectAcpStream} opens the ACP
 * WebSocket, and {@link createKanbanClient} performs the `initialize`
 * handshake. The hosting container supplies `startAgent` — a thin wrapper over
 * the `ai_start_agent` Tauri command — and the board directory.
 *
 * @param boardDir - Absolute path of the open board (becomes `newSession.cwd`).
 * @param startAgent - Calls `ai_start_agent`; yields the agent's two URLs.
 * @returns A factory that, per model id, builds a {@link ConversationConnect}.
 */
export function aiPanelConnectFactory(
  boardDir: string,
  startAgent: (modelId: string) => Promise<AgentEndpoint>,
): AiPanelConnectFactory {
  return (modelId) => async (handlers) => {
    const endpoint = await startAgent(modelId);
    const connection = await connectAcpStream(endpoint.wsUrl);
    return createKanbanClient({
      stream: connection.stream,
      boardDir,
      mcpUrl: endpoint.mcpUrl,
      onSessionUpdate: handlers.onSessionUpdate,
      onRequestPermission: handlers.onRequestPermission,
    });
  };
}

/** Props for {@link AiPanel}. */
export interface AiPanelProps {
  /** Absolute path of the open board — the agent's `newSession.cwd`. */
  boardDir: string;
  /**
   * The selectable models, or `undefined` while the container is still
   * fetching `ai_list_models`.
   */
  models: AiModel[] | undefined;
  /**
   * The currently selected model id, or `null` when none is chosen yet. The
   * container persists this per board in `UIState` and feeds it back here.
   */
  modelId: string | null;
  /**
   * Report the user's model choice. The container persists it per board and
   * feeds the new id back via {@link AiPanelProps.modelId}.
   */
  onSelectModel: (modelId: string) => void;
  /** Builds the {@link ConversationConnect} factory for a model id. */
  createConnect: AiPanelConnectFactory;
}

/**
 * The AI conversation panel.
 *
 * Renders the model selector, the conversation log, the inline permission
 * prompt, and the composer. The conversation itself lives in
 * {@link AiPanelConversation}, keyed on the selected model id so a model
 * switch starts a fresh stateless ACP session.
 */
export function AiPanel({
  boardDir,
  models,
  modelId,
  onSelectModel,
  createConnect,
}: AiPanelProps): ReactNode {
  void boardDir;
  const selectedModel = useMemo(
    () => models?.find((model) => model.id === modelId) ?? null,
    [models, modelId],
  );

  return (
    // The panel body is a spatial-nav ZONE — `ui:ai-panel`, a child of the
    // window-root `<FocusLayer>` (see `App.tsx`). Its FQM is the path
    // `/window/ui:ai-panel`; the header selector, the conversation
    // scrollback, the composer, and the per-message action buttons each
    // register a `<FocusScope>` leaf one level deeper. `showFocus={false}`
    // because a focus rectangle around the whole panel is visual noise —
    // the inner leaves render their own indicator. See `ai-panel-focus.tsx`
    // for the path-monikers rationale and the zone-vs-layer decision.
    <AiPanelFocusScope
      moniker={asSegment("ui:ai-panel")}
      showFocus={false}
      className="flex h-full min-h-0 flex-col bg-background"
    >
      {/* The inner `data-slot='ai-panel'` div fills the zone wrapper. When
          the spatial stack is absent (standalone unit test) the zone wrapper
          collapses to a fragment and this div is the panel root, so it
          carries the full `h-full` layout chain either way. */}
      <div
        className="flex h-full min-h-0 flex-1 flex-col bg-background"
        data-slot="ai-panel"
      >
        <AiPanelHeader
          models={models}
          selectedModel={selectedModel}
          onSelectModel={onSelectModel}
        />
        {modelId === null ? (
          <NoModelState hasModels={(models?.length ?? 0) > 0} />
        ) : (
          <AiPanelConversation
            // Keying on the model id remounts the conversation on a model
            // switch — tearing down the prior ACP session and starting a
            // fresh, stateless one for the newly selected model.
            key={modelId}
            modelId={modelId}
            modelReady={selectedModel?.available ?? false}
            createConnect={createConnect}
          />
        )}
      </div>
    </AiPanelFocusScope>
  );
}

/** Props for {@link AiPanelHeader}. */
interface AiPanelHeaderProps {
  models: AiModel[] | undefined;
  selectedModel: AiModel | null;
  onSelectModel: (modelId: string) => void;
}

/**
 * The panel header: the title and the model selector dropdown.
 *
 * The selector lists every model from `ai_list_models`. An unavailable model
 * is a disabled menu item that still shows its hint (e.g. "install Claude
 * Code"); selecting an available one reports the choice upward.
 */
function AiPanelHeader({
  models,
  selectedModel,
  onSelectModel,
}: AiPanelHeaderProps): ReactNode {
  const triggerLabel = selectedModel?.label ?? "Select a model";

  return (
    <header className="flex items-center justify-between gap-2 border-b px-3 py-2">
      <div className="flex items-center gap-1.5 font-medium text-sm">
        <SparklesIcon className="size-4 text-muted-foreground" />
        <span>AI</span>
      </div>
      <DropdownMenu>
        {/* The model selector is a focusable spatial-nav leaf:
            `ui:ai-panel.model-selector`, composed under the `ui:ai-panel`
            zone. `<AiPanelPressable asChild>` mounts the leaf and the
            Enter / Space keyboard-activation CommandDefs; the
            `DropdownMenuTrigger`'s slot becomes the host `<button>` so the
            chain renders exactly one button. `onPress` is a no-op — Radix's
            own trigger handler opens the menu; the Pressable is here purely
            for the focus leaf + keyboard activation, and a `<button>`
            press toggles the dropdown natively.

            `ariaLabel` is the trigger's visible label (the model name, or
            "Select a model"): the button already shows that text, so the
            accessible name must match it rather than introduce a competing
            label. */}
        <DropdownMenuTrigger asChild>
          <AiPanelPressable
            asChild
            moniker={asSegment("ui:ai-panel.model-selector")}
            ariaLabel={triggerLabel}
            onPress={() => {}}
            disabled={!models || models.length === 0}
          >
            <Button
              className="gap-1.5"
              disabled={!models || models.length === 0}
              size="sm"
              variant="outline"
            >
              {triggerLabel}
              <ChevronDownIcon className="size-4 opacity-60" />
            </Button>
          </AiPanelPressable>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" className="w-64">
          <DropdownMenuLabel>Model</DropdownMenuLabel>
          <DropdownMenuSeparator />
          {(models ?? []).map((model) => (
            <ModelMenuItem
              key={model.id}
              model={model}
              selected={model.id === selectedModel?.id}
              onSelect={onSelectModel}
            />
          ))}
        </DropdownMenuContent>
      </DropdownMenu>
    </header>
  );
}

/** Props for {@link ModelMenuItem}. */
interface ModelMenuItemProps {
  model: AiModel;
  selected: boolean;
  onSelect: (modelId: string) => void;
}

/**
 * One model entry in the selector dropdown.
 *
 * An unavailable model is disabled — it cannot be picked — but still shows its
 * hint so the user knows *why* (e.g. the Claude Code CLI was not found).
 */
function ModelMenuItem({
  model,
  selected,
  onSelect,
}: ModelMenuItemProps): ReactNode {
  return (
    <DropdownMenuItem
      className="flex flex-col items-start gap-0.5"
      disabled={!model.available}
      onSelect={() => onSelect(model.id)}
      title={model.hint ?? undefined}
    >
      <span className={cn("text-sm", selected && "font-semibold")}>
        {model.label}
        {selected ? " (current)" : ""}
      </span>
      {model.hint && (
        <span className="text-muted-foreground text-xs">{model.hint}</span>
      )}
    </DropdownMenuItem>
  );
}

/** Props for {@link NoModelState}. */
interface NoModelStateProps {
  hasModels: boolean;
}

/**
 * The empty state shown before a model is selected.
 *
 * Mirrors the disabled-composer affordance: the panel is inert until the user
 * picks a model from the header selector.
 */
function NoModelState({ hasModels }: NoModelStateProps): ReactNode {
  return (
    <div className="flex flex-1 flex-col">
      <ConversationEmptyState
        description={
          hasModels
            ? "Pick a model from the selector above to start chatting."
            : "No AI models are configured."
        }
        icon={<SparklesIcon className="size-6" />}
        title="Choose a model"
      />
      <ComposerArea
        disabled
        placeholder="Select a model to start..."
        status="ready"
        onCancel={() => {}}
        onNewConversation={() => {}}
        onSend={() => {}}
      />
    </div>
  );
}

/** Props for {@link AiPanelConversation}. */
interface AiPanelConversationProps {
  modelId: string;
  modelReady: boolean;
  createConnect: AiPanelConnectFactory;
}

/**
 * The live conversation for one selected model.
 *
 * Owns the {@link useConversation} hook bound to the model's
 * {@link ConversationConnect} factory, renders the message log and the
 * permission prompt, and drives the composer's `sendPrompt`/`cancel`. Mounted
 * with a `key` of the model id, so a model switch remounts it — dropping the
 * prior session and starting a fresh stateless one.
 */
function AiPanelConversation({
  modelId,
  modelReady,
  createConnect,
}: AiPanelConversationProps): ReactNode {
  const connect = useMemo(
    () => createConnect(modelId),
    [createConnect, modelId],
  );
  const conversation = useConversation({ connect });
  const {
    messages,
    status,
    sendPrompt,
    cancel,
    newConversation,
    permissionRequest,
    respondPermission,
  } = conversation;

  // Register the conversation-owned `ai.*` command handlers into the
  // `ai/commands.ts` registry so the window-layer `ai.newChat` / `ai.cancel`
  // commands (in `AppShell`'s global scope) can drive this conversation.
  // `cancel` and `newConversation` are stable `useCallback`s, so the registry
  // slot is rewritten only when the conversation hook re-mints them.
  useEffect(() => {
    return registerAiCommandHandlers({
      newChat: newConversation,
      cancel: () => {
        void cancel();
      },
    });
  }, [newConversation, cancel]);

  // Mirror the ACP turn status into the AI command registry so `ai.cancel`'s
  // `available` gate (and the backend `UIState.ai_streaming` flag) track the
  // live conversation. `setAiStreaming` is a no-op on an unchanged value.
  useEffect(() => {
    setAiStreaming(status === "streaming");
  }, [status]);

  // Leaving streaming behind on unmount avoids a stuck "available" `ai.cancel`
  // if the panel is torn down mid-turn.
  useEffect(() => {
    return () => {
      setAiStreaming(false);
    };
  }, []);

  const handleSend = useCallback(
    (message: PromptInputMessage) => {
      const text = message.text.trim();
      if (text.length === 0) {
        return;
      }
      const blocks: ContentBlock[] = [{ type: "text", text }];
      void sendPrompt(blocks);
    },
    [sendPrompt],
  );

  const handleCancel = useCallback(() => {
    void cancel();
  }, [cancel]);

  // Resend a user message's text as a fresh prompt — the per-message
  // retry action. A no-op while a turn is streaming, mirroring the
  // composer being inert mid-turn.
  const handleRetry = useCallback(
    (text: string) => {
      const trimmed = text.trim();
      if (trimmed.length === 0 || status === "streaming") {
        return;
      }
      void sendPrompt([{ type: "text", text: trimmed }]);
    },
    [sendPrompt, status],
  );

  // `ChatStatus` has no "idle" — the composer is `ready` between turns, and
  // `error` after a failed turn so the submit button shows the error glyph.
  const composerStatus: "streaming" | "ready" | "error" =
    status === "idle" ? "ready" : status;

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      {/* The conversation scrollback is its own focus scope —
          `ui:ai-panel.scrollback` under the panel zone — so jump-to and
          arrow-nav can land on the transcript region. `showFocus={false}`:
          the scrollback is a large bounded region, and a focus rectangle
          framing the whole transcript would be noise; the per-message
          action leaves render their own indicators. */}
      <AiPanelFocusScope
        moniker={asSegment("ui:ai-panel.scrollback")}
        showFocus={false}
        className="flex min-h-0 flex-1 flex-col"
      >
        <Conversation>
          <ConversationContent>
            {messages.length === 0 ? (
              <ConversationEmptyState
                description="Send a message to start the conversation."
                icon={<SparklesIcon className="size-6" />}
                title="New conversation"
              />
            ) : (
              messages.map((message) => (
                <ConversationMessageView
                  key={message.id}
                  message={message}
                  onRetry={handleRetry}
                />
              ))
            )}
            {status === "streaming" && (
              <div className="flex items-center gap-2 text-muted-foreground text-sm">
                <Loader size={16} />
                <span>Thinking...</span>
              </div>
            )}
            {permissionRequest && (
              <PermissionPrompt
                request={permissionRequest}
                onRespond={respondPermission}
              />
            )}
          </ConversationContent>
          <ConversationScrollButton />
        </Conversation>
      </AiPanelFocusScope>
      <ComposerArea
        disabled={!modelReady}
        placeholder="Ask the AI agent..."
        status={composerStatus}
        onCancel={handleCancel}
        onNewConversation={newConversation}
        onSend={handleSend}
      />
    </div>
  );
}

/**
 * Concatenate a message's plain-text parts into one copyable string.
 *
 * Only `text` parts carry user-visible prose; reasoning, tool, and plan
 * parts are structured chrome and are skipped. Returns the empty string
 * when the message has no text parts.
 */
function messagePlainText(message: ConversationMessage): string {
  return message.parts
    .filter(
      (part): part is Extract<typeof part, { type: "text" }> =>
        part.type === "text",
    )
    .map((part) => part.text)
    .join("\n\n");
}

/** Props for {@link ConversationMessageView}. */
interface ConversationMessageViewProps {
  message: ConversationMessage;
  /**
   * Resend the given user message's text as a fresh prompt — wired to the
   * per-message retry action. `undefined` for assistant messages, which
   * have no retry affordance.
   */
  onRetry?: (text: string) => void;
}

/**
 * Render one conversation message — every part, in order — plus the
 * per-message action bar (copy, and retry for user messages).
 *
 * Dispatches each {@link ConversationMessage} part to the matching AI Elements
 * component: `text` -> {@link MessageResponse}, `reasoning` -> {@link Reasoning},
 * `dynamic-tool` -> {@link Tool}, the custom `data-plan` -> {@link Task}.
 */
function ConversationMessageView({
  message,
  onRetry,
}: ConversationMessageViewProps): ReactNode {
  return (
    <Message from={message.role}>
      <MessageContent>
        {message.parts.map((part, index) => (
          <MessagePartView
            // Parts have no stable id; the index is stable within a message
            // because parts are only appended or replaced in place.
            key={`${message.id}-${index}`}
            part={part}
          />
        ))}
      </MessageContent>
      <MessageActionBar message={message} onRetry={onRetry} />
    </Message>
  );
}

/**
 * Shared Tailwind class string for a per-message action icon button —
 * a small ghost button mirroring the AI Elements `MessageAction` look so
 * the copy / retry controls stay visually consistent with the rest of
 * the conversation chrome.
 */
const MESSAGE_ACTION_BUTTON_CLASS =
  "inline-flex size-7 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-muted hover:text-foreground";

/** Props for {@link MessageActionBar}. */
interface MessageActionBarProps {
  message: ConversationMessage;
  onRetry?: (text: string) => void;
}

/**
 * The per-message action bar: a copy button on every message and a retry
 * button on user messages.
 *
 * Each button is an `<AiPanelPressable>` leaf — moniker
 * `ui:ai-panel.message-action:{messageId}:{kind}` — so jump-to and arrow
 * navigation reach the action exactly like any other panel control. The
 * leaf composes under the `ui:ai-panel` zone, keeping the per-message
 * action a path-addressable spatial target.
 *
 * The bar renders nothing for a message with no copyable text (e.g. an
 * assistant turn that produced only tool calls) — there is nothing to
 * copy and no user prompt to retry.
 */
function MessageActionBar({
  message,
  onRetry,
}: MessageActionBarProps): ReactNode {
  const text = useMemo(() => messagePlainText(message), [message]);
  const [copied, setCopied] = useState(false);

  const handleCopy = useCallback(() => {
    void navigator.clipboard
      .writeText(text)
      .then(() => {
        setCopied(true);
        // Revert the checkmark to the copy glyph after a short beat so the
        // button is ready for the next copy.
        window.setTimeout(() => setCopied(false), 1500);
      })
      .catch((err) => {
        console.error("[AiPanel] copy to clipboard failed", err);
      });
  }, [text]);

  if (text.length === 0) {
    return null;
  }

  const isUser = message.role === "user";

  return (
    <MessageActions>
      <AiPanelPressable
        moniker={asSegment(`ui:ai-panel.message-action:${message.id}:copy`)}
        ariaLabel="Copy message"
        onPress={handleCopy}
        className={MESSAGE_ACTION_BUTTON_CLASS}
        title={copied ? "Copied" : "Copy"}
      >
        {copied ? (
          <CheckIcon className="size-3.5" />
        ) : (
          <CopyIcon className="size-3.5" />
        )}
      </AiPanelPressable>
      {isUser && onRetry && (
        <AiPanelPressable
          moniker={asSegment(`ui:ai-panel.message-action:${message.id}:retry`)}
          ariaLabel="Retry message"
          onPress={() => onRetry(text)}
          className={MESSAGE_ACTION_BUTTON_CLASS}
          title="Retry"
        >
          <RotateCcwIcon className="size-3.5" />
        </AiPanelPressable>
      )}
    </MessageActions>
  );
}

/** Props for {@link MessagePartView}. */
interface MessagePartViewProps {
  part: ConversationMessage["parts"][number];
}

/** The `data-plan` arm of the conversation part union, carrying a plan. */
type PlanPart = ConversationMessage["parts"][number] & {
  type: "data-plan";
  data: PlanPartData;
};

/**
 * Narrow a message part to the `data-plan` arm carrying a {@link PlanPartData}.
 *
 * `ConversationDataParts` has an index signature (`Record<string, unknown>`)
 * alongside the concrete `plan` entry, so the AI SDK's `DataUIPart` union also
 * produces a `data: unknown` arm whose `type` (`` `data-${string}` ``) overlaps
 * `"data-plan"`. A `part.type === "data-plan"` check therefore leaves `data` as
 * `unknown`. This predicate asserts the concrete arm, letting `MessagePartView`
 * narrow `data` to `PlanPartData` without a bare cast.
 */
function isPlanPart(
  part: ConversationMessage["parts"][number],
): part is PlanPart {
  return part.type === "data-plan";
}

/**
 * Render a single message part with the AI Elements component for its kind.
 *
 * An unrecognized part kind renders nothing — the conversation reducer only
 * ever emits `text`, `reasoning`, `dynamic-tool`, and `data-plan`, so the
 * fall-through is unreachable in practice and just keeps the switch total.
 */
function MessagePartView({ part }: MessagePartViewProps): ReactNode {
  // `isPlanPart` is checked before the `switch` because the `data-plan` arm
  // needs the predicate to narrow `part.data` to `PlanPartData` — a `case`
  // label only narrows `part.type`, not the index-signature-widened `data`.
  if (isPlanPart(part)) {
    return <PlanView data={part.data} />;
  }
  switch (part.type) {
    case "text":
      return <MessageResponse>{part.text}</MessageResponse>;
    case "reasoning":
      return (
        <Reasoning isStreaming={part.state === "streaming"}>
          <ReasoningTrigger />
          <ReasoningContent>{part.text}</ReasoningContent>
        </Reasoning>
      );
    case "dynamic-tool":
      return <ToolCallView part={part} />;
    default:
      return null;
  }
}

/** Props for {@link ToolCallView}. */
interface ToolCallViewProps {
  part: DynamicToolUIPart;
}

/**
 * Render an ACP tool call as a collapsible {@link Tool} card.
 *
 * Shows the tool name and status badge in the header, the call's input
 * parameters, and — once the call finishes — its output or error.
 */
function ToolCallView({ part }: ToolCallViewProps): ReactNode {
  const hasOutput = part.state === "output-available";
  const hasError = part.state === "output-error";
  return (
    <Tool>
      <ToolHeader
        state={part.state}
        title={part.title ?? part.toolName}
        // `ToolHeader` derives a fallback label from the part `type`; the ACP
        // adapter emits `dynamic-tool` parts, so pass that union member.
        type={"dynamic-tool" as ToolUIPart["type"]}
      />
      <ToolContent>
        <ToolInput input={part.input} />
        {(hasOutput || hasError) && (
          <ToolOutput
            errorText={hasError ? part.errorText : undefined}
            output={hasOutput ? part.output : undefined}
          />
        )}
      </ToolContent>
    </Tool>
  );
}

/** Props for {@link PlanView}. */
interface PlanViewProps {
  data: PlanPartData;
}

/** The lucide icon for each ACP plan-entry status. */
const PLAN_STATUS_ICON: Record<PlanEntryStatus, ReactNode> = {
  pending: <CircleIcon className="size-3.5 text-muted-foreground" />,
  in_progress: <CircleDotIcon className="size-3.5 text-blue-600" />,
  completed: <CircleCheckIcon className="size-3.5 text-green-600" />,
};

/**
 * Render the agent's execution plan with the {@link Task} components.
 *
 * Each {@link PlanEntry} becomes a {@link TaskItem} prefixed with a
 * status-coloured icon, so the plan reads as a live checklist.
 */
function PlanView({ data }: PlanViewProps): ReactNode {
  const done = data.entries.filter(
    (entry) => entry.status === "completed",
  ).length;
  return (
    <Task className="mb-4">
      <TaskTrigger title={`Plan (${done}/${data.entries.length})`} />
      <TaskContent>
        {data.entries.map((entry: PlanEntry, index) => (
          <TaskItem
            className="flex items-center gap-2"
            key={`${index}-${entry.content}`}
          >
            {PLAN_STATUS_ICON[entry.status]}
            <span>{entry.content}</span>
          </TaskItem>
        ))}
      </TaskContent>
    </Task>
  );
}

/** Props for {@link PermissionPrompt}. */
interface PermissionPromptProps {
  request: RequestPermissionRequest;
  onRespond: (response: {
    outcome:
      | { outcome: "cancelled" }
      | { outcome: "selected"; optionId: string };
  }) => void;
}

/** The button variant for each ACP permission-option kind. */
const PERMISSION_OPTION_VARIANT: Record<
  string,
  "default" | "secondary" | "destructive" | "outline"
> = {
  allow_once: "default",
  allow_always: "secondary",
  reject_once: "outline",
  reject_always: "destructive",
};

/**
 * The inline permission approval UI.
 *
 * When the agent calls `session/request_permission`, the conversation hook
 * surfaces the request here. This renders the tool's title and one button per
 * {@link RequestPermissionRequest.options} entry (allow once / allow for
 * session / deny). Clicking a button resolves the agent's request with that
 * option id; the prompt then disappears.
 */
function PermissionPrompt({
  request,
  onRespond,
}: PermissionPromptProps): ReactNode {
  const toolTitle = request.toolCall.title ?? "a tool call";
  return (
    <div
      className="rounded-md border border-yellow-500/40 bg-yellow-500/5 p-3"
      data-slot="ai-permission-prompt"
      role="group"
    >
      <p className="font-medium text-sm">Permission required</p>
      <p className="mt-1 text-muted-foreground text-sm">
        The agent wants to run <span className="font-medium">{toolTitle}</span>.
      </p>
      <div className="mt-3 flex flex-wrap gap-2">
        {request.options.map((option) => (
          <Button
            key={option.optionId}
            onClick={() =>
              onRespond({
                outcome: { outcome: "selected", optionId: option.optionId },
              })
            }
            size="sm"
            variant={PERMISSION_OPTION_VARIANT[option.kind] ?? "outline"}
          >
            {option.name}
          </Button>
        ))}
      </div>
    </div>
  );
}

/** Props for {@link ComposerArea}. */
interface ComposerAreaProps {
  disabled: boolean;
  placeholder: string;
  status: "submitted" | "streaming" | "ready" | "error";
  onSend: (message: PromptInputMessage) => void;
  onCancel: () => void;
  onNewConversation: () => void;
}

/**
 * The composer: the prompt textarea, the submit/stop button, and the
 * "new conversation" action.
 *
 * Submitting calls `onSend`; while a turn streams the submit button becomes a
 * stop control that calls `onCancel`. "New conversation" tears the session
 * down and starts fresh.
 */
function ComposerArea({
  disabled,
  placeholder,
  status,
  onSend,
  onCancel,
  onNewConversation,
}: ComposerAreaProps): ReactNode {
  const streaming = status === "streaming";

  const handleSubmit = useCallback(
    (message: PromptInputMessage) => {
      // While streaming, the submit button is the stop control.
      if (streaming) {
        onCancel();
        return;
      }
      onSend(message);
    },
    [streaming, onCancel, onSend],
  );

  return (
    <div className="border-t p-2">
      <div className="mb-1 flex justify-end">
        <Button
          disabled={disabled}
          onClick={onNewConversation}
          size="sm"
          variant="ghost"
        >
          <PlusIcon className="size-4" />
          New conversation
        </Button>
      </div>
      {/* The composer is a focus scope — `ui:ai-panel.composer` under the
          panel zone — so jump-to lands directly on the prompt box and
          arrow-nav reaches it. `<FocusScope>` deliberately does NOT steal a
          click that lands on a `<textarea>`, so caret placement inside the
          prompt is untouched; the scope just registers the leaf and paints
          its focus indicator. */}
      <AiPanelFocusScope moniker={asSegment("ui:ai-panel.composer")}>
        <PromptInput onSubmit={handleSubmit}>
          <PromptInputBody>
            <PromptInputTextarea
              aria-label="Message the AI agent"
              disabled={disabled}
              placeholder={placeholder}
            />
          </PromptInputBody>
          <PromptInputFooter>
            <span className="text-muted-foreground text-xs">
              {streaming ? "Streaming - click to stop" : ""}
            </span>
            <PromptInputSubmit
              aria-label={streaming ? "Stop" : "Submit"}
              disabled={disabled}
              status={status}
            />
          </PromptInputFooter>
        </PromptInput>
      </AiPanelFocusScope>
    </div>
  );
}
