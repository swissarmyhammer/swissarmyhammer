/**
 * Smoke render tests for the vendored AI Elements component library.
 *
 * AI Elements is shadcn-style: the components are copied into
 * `src/components/ai-elements/` rather than imported from a package.
 * These components are purely presentational — they are rendered
 * directly from ACP conversation state, not driven by the AI SDK
 * `useChat` hook. This file is the safety net for the vendoring: it
 * mounts every public component with representative sample props and
 * asserts it renders without throwing.
 *
 * The intent is breadth, not depth — one render per component family,
 * enough to catch a missing dependency, a broken import, or a
 * type/runtime regression after a re-vendor. Behavioral coverage of
 * the conversation panel belongs with the panel feature itself.
 */

import { describe, it, expect } from "vitest";
import { renderInAct, flushActSettle } from "@/test/act-render";
import { TooltipProvider } from "@/components/ui/tooltip";

import {
  Conversation,
  ConversationContent,
  ConversationEmptyState,
  ConversationScrollButton,
} from "./conversation";
import {
  Message,
  MessageContent,
  MessageActions,
  MessageAction,
  MessageBranch,
  MessageBranchContent,
  MessageBranchSelector,
  MessageBranchPrevious,
  MessageBranchNext,
  MessageBranchPage,
  MessageResponse,
  MessageAttachment,
  MessageAttachments,
  MessageToolbar,
} from "./message";
import { Reasoning, ReasoningTrigger, ReasoningContent } from "./reasoning";
import {
  Tool,
  ToolHeader,
  ToolContent,
  ToolInput,
  ToolOutput,
} from "./tool";
import {
  Task,
  TaskTrigger,
  TaskContent,
  TaskItem,
  TaskItemFile,
} from "./task";
import {
  PromptInput,
  PromptInputBody,
  PromptInputTextarea,
  PromptInputFooter,
  PromptInputSubmit,
} from "./prompt-input";
import { Loader } from "./loader";
import { CodeBlock, CodeBlockCopyButton } from "./code-block";
import { Shimmer } from "./shimmer";

describe("AI Elements: Conversation", () => {
  it("mounts Conversation with content, empty state, and scroll button", async () => {
    const { container } = await renderInAct(
      <Conversation>
        <ConversationContent>
          <ConversationEmptyState
            title="No messages yet"
            description="Start a conversation to see messages here"
          />
          <ConversationScrollButton />
        </ConversationContent>
      </Conversation>,
    );

    // StickToBottom renders the conversation log region.
    expect(container.querySelector("[role='log']")).not.toBeNull();
    expect(container.textContent).toContain("No messages yet");
  });
});

describe("AI Elements: Message", () => {
  it("mounts a Message with content, actions, response, and toolbar", async () => {
    const { container } = await renderInAct(
      <Message from="assistant">
        <MessageContent>
          <MessageResponse>{"Hello **world**"}</MessageResponse>
        </MessageContent>
        <MessageToolbar>
          <MessageActions>
            <MessageAction tooltip="Copy" label="Copy">
              <span aria-hidden>C</span>
            </MessageAction>
          </MessageActions>
        </MessageToolbar>
      </Message>,
    );

    // MessageResponse renders the markdown via Streamdown.
    expect(container.textContent).toContain("Hello");
    expect(container.textContent).toContain("world");
    // The action button carries its accessible label.
    expect(container.querySelector("button")).not.toBeNull();
  });

  it("mounts a user Message with attachments", async () => {
    // A non-image MessageAttachment renders a Tooltip and expects the
    // host app to supply a TooltipProvider — the same shadcn pattern
    // the rest of this UI follows.
    const { container } = await renderInAct(
      <TooltipProvider>
        <Message from="user">
          <MessageAttachments>
            <MessageAttachment
              data={{
                type: "file",
                filename: "notes.txt",
                mediaType: "text/plain",
                url: "data:text/plain;base64,aGk=",
              }}
            />
          </MessageAttachments>
          <MessageContent>Please review this.</MessageContent>
        </Message>
      </TooltipProvider>,
    );

    expect(container.querySelector(".is-user")).not.toBeNull();
    expect(container.textContent).toContain("Please review this.");
  });

  it("mounts a MessageBranch carousel with two branches", async () => {
    const { container } = await renderInAct(
      <MessageBranch defaultBranch={0}>
        <MessageBranchContent>
          <Message key="a" from="assistant">
            <MessageContent>First answer</MessageContent>
          </Message>
          <Message key="b" from="assistant">
            <MessageContent>Second answer</MessageContent>
          </Message>
        </MessageBranchContent>
        <MessageBranchSelector from="assistant">
          <MessageBranchPrevious />
          <MessageBranchPage />
          <MessageBranchNext />
        </MessageBranchSelector>
      </MessageBranch>,
    );

    // Only the active branch is visible; the selector reports the page.
    expect(container.textContent).toContain("First answer");
    expect(container.textContent).toContain("1 of 2");
  });
});

describe("AI Elements: Reasoning", () => {
  it("mounts Reasoning with a trigger and streamed content", async () => {
    const { container } = await renderInAct(
      <Reasoning isStreaming={false} defaultOpen>
        <ReasoningTrigger />
        <ReasoningContent>{"Considering the **options**."}</ReasoningContent>
      </Reasoning>,
    );

    expect(container.textContent).toContain("Considering the");
    expect(container.textContent).toContain("options");
  });
});

describe("AI Elements: Tool", () => {
  it("mounts a Tool with header, input, and output", async () => {
    const { container } = await renderInAct(
      <Tool defaultOpen>
        <ToolHeader
          type="tool-search"
          state="output-available"
          title="search"
        />
        <ToolContent>
          <ToolInput input={{ query: "kanban" }} />
          <ToolOutput
            output={{ hits: 3 }}
            errorText={undefined}
          />
        </ToolContent>
      </Tool>,
    );

    expect(container.textContent).toContain("search");
    expect(container.textContent).toContain("Completed");
    expect(container.textContent).toContain("Parameters");
    expect(container.textContent).toContain("Result");
  });

  it("mounts a Tool in the error state", async () => {
    const { container } = await renderInAct(
      <Tool defaultOpen>
        <ToolHeader type="tool-fetch" state="output-error" />
        <ToolContent>
          <ToolOutput output={undefined} errorText="request failed" />
        </ToolContent>
      </Tool>,
    );

    expect(container.textContent).toContain("Error");
    expect(container.textContent).toContain("request failed");
  });
});

describe("AI Elements: Task", () => {
  it("mounts a Task with a trigger and item content", async () => {
    const { container } = await renderInAct(
      <Task defaultOpen>
        <TaskTrigger title="Searching the codebase" />
        <TaskContent>
          <TaskItem>
            Read <TaskItemFile>src/main.rs</TaskItemFile>
          </TaskItem>
        </TaskContent>
      </Task>,
    );

    expect(container.textContent).toContain("Searching the codebase");
    expect(container.textContent).toContain("src/main.rs");
  });
});

describe("AI Elements: PromptInput", () => {
  it("mounts a PromptInput with a textarea and submit button", async () => {
    const { container } = await renderInAct(
      <PromptInput onSubmit={() => {}}>
        <PromptInputBody>
          <PromptInputTextarea placeholder="Ask the agent..." />
        </PromptInputBody>
        <PromptInputFooter>
          <PromptInputSubmit status="ready" />
        </PromptInputFooter>
      </PromptInput>,
    );

    expect(container.querySelector("textarea")).not.toBeNull();
    expect(container.querySelector("button[type='submit']")).not.toBeNull();
  });
});

describe("AI Elements: Loader", () => {
  it("mounts the Loader spinner", async () => {
    const { container } = await renderInAct(<Loader size={24} />);

    expect(container.querySelector("svg")).not.toBeNull();
    expect(container.querySelector(".animate-spin")).not.toBeNull();
  });
});

describe("AI Elements: CodeBlock", () => {
  it("mounts a CodeBlock with a copy button", async () => {
    const { container } = await renderInAct(
      <CodeBlock code={'{"ok": true}'} language="json">
        <CodeBlockCopyButton />
      </CodeBlock>,
    );

    // The copy button mounts synchronously.
    expect(container.querySelector("button")).not.toBeNull();
    // Shiki highlights asynchronously — let the effect resolve before
    // asserting the highlighted source landed in the DOM.
    await flushActSettle(50);
    expect(container.textContent).toContain("ok");
  });
});

describe("AI Elements: Shimmer", () => {
  it("mounts the Shimmer text effect", async () => {
    const { container } = await renderInAct(<Shimmer>Thinking...</Shimmer>);

    expect(container.textContent).toContain("Thinking...");
  });
});
