import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, fireEvent, act } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Mocks — must be before component imports
// ---------------------------------------------------------------------------

const mockOpen = vi.fn();
vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: (...args: unknown[]) => mockOpen(...args),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve(null)),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));
vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({
    onDragDropEvent: vi.fn(() => Promise.resolve(() => {})),
  }),
}));
vi.mock("@tauri-apps/plugin-log", () => ({
  error: vi.fn(),
  warn: vi.fn(),
  info: vi.fn(),
  debug: vi.fn(),
  trace: vi.fn(),
  attachConsole: vi.fn(() => Promise.resolve()),
}));

// ---------------------------------------------------------------------------
// Imports — after mocks
// ---------------------------------------------------------------------------

import { AttachmentEditor } from "./attachment-editor";
import { FileDropProvider, useFileDrop } from "@/lib/file-drop-context";
import type { FieldDef } from "@/types/kanban";
import type { AttachmentMeta } from "@/components/fields/displays/attachment-display";

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

const ATTACHMENT_FIELD: FieldDef = {
  id: "f1",
  name: "attachments",
  type: { kind: "attachment", multiple: true },
  section: "body",
};

const SINGLE_ATTACHMENT_FIELD: FieldDef = {
  id: "f2",
  name: "cover_image",
  type: { kind: "attachment", multiple: false },
  section: "body",
};

const SAMPLE_ATTACHMENTS: AttachmentMeta[] = [
  {
    id: "att-1",
    name: "readme.md",
    size: 1024,
    mime_type: "text/markdown",
    path: "/home/user/readme.md",
  },
  {
    id: "att-2",
    name: "screenshot.png",
    size: 204800,
    mime_type: "image/png",
    path: "/home/user/screenshot.png",
  },
];

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function renderEditor(props: {
  field?: FieldDef;
  value?: unknown;
  onCommit?: (val: unknown) => void;
  onCancel?: () => void;
  onChange?: (val: unknown) => void;
}) {
  return render(
    <FileDropProvider>
      <AttachmentEditor
        field={props.field ?? ATTACHMENT_FIELD}
        value={props.value ?? []}
        onCommit={props.onCommit ?? vi.fn()}
        onCancel={props.onCancel ?? vi.fn()}
        onChange={props.onChange}
        mode="compact"
      />
    </FileDropProvider>,
  );
}

/**
 * Helper component that exposes FileDropProvider internals for testing
 * the attachment editor's drop zone integration.
 */
function FileDropTestHarness({
  children,
  isDragging,
}: {
  children: React.ReactNode;
  isDragging: boolean;
}) {
  return (
    <FileDropProvider _testOverride={{ isDragging }}>
      {children}
    </FileDropProvider>
  );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("AttachmentEditor", () => {
  beforeEach(() => {
    mockOpen.mockReset();
  });

  describe("rendering existing attachments", () => {
    it("renders attachment filenames", () => {
      const { getByText } = renderEditor({ value: SAMPLE_ATTACHMENTS });

      expect(getByText("readme.md")).toBeTruthy();
      expect(getByText("screenshot.png")).toBeTruthy();
    });

    it("renders remove buttons for each attachment", () => {
      const { getAllByRole } = renderEditor({ value: SAMPLE_ATTACHMENTS });

      // Each attachment has a remove button
      const removeButtons = getAllByRole("button", { name: /remove/i });
      expect(removeButtons).toHaveLength(2);
    });

    it("renders empty state when no attachments", () => {
      const { getByText } = renderEditor({ value: [] });

      expect(getByText(/no attachments/i)).toBeTruthy();
    });
  });

  describe("remove button", () => {
    it("fires onChange with attachment removed from array", () => {
      const onChange = vi.fn();
      const { getAllByRole } = renderEditor({
        value: SAMPLE_ATTACHMENTS,
        onChange,
      });

      const removeButtons = getAllByRole("button", { name: /remove/i });
      // Remove the first attachment (readme.md)
      fireEvent.click(removeButtons[0]);

      expect(onChange).toHaveBeenCalledWith([SAMPLE_ATTACHMENTS[1]]);
    });

    it("fires onChange with empty array when removing last attachment", () => {
      const onChange = vi.fn();
      const { getAllByRole } = renderEditor({
        value: [SAMPLE_ATTACHMENTS[0]],
        onChange,
      });

      const removeButtons = getAllByRole("button", { name: /remove/i });
      fireEvent.click(removeButtons[0]);

      expect(onChange).toHaveBeenCalledWith([]);
    });
  });

  describe("add file button", () => {
    it("renders an add file button", () => {
      const { getByRole } = renderEditor({});

      expect(getByRole("button", { name: /add file/i })).toBeTruthy();
    });

    it("calls open() with multiple: true for multiple attachment fields", async () => {
      mockOpen.mockResolvedValue(["/tmp/newfile.txt"]);
      const onChange = vi.fn();
      const { getByRole } = renderEditor({
        field: ATTACHMENT_FIELD,
        value: [],
        onChange,
      });

      await act(async () => {
        fireEvent.click(getByRole("button", { name: /add file/i }));
      });

      expect(mockOpen).toHaveBeenCalledWith(
        expect.objectContaining({ multiple: true, directory: false }),
      );
    });

    it("calls open() with multiple: false for single attachment fields", async () => {
      mockOpen.mockResolvedValue("/tmp/newfile.txt");
      const onChange = vi.fn();
      const { getByRole } = renderEditor({
        field: SINGLE_ATTACHMENT_FIELD,
        value: null,
        onChange,
      });

      await act(async () => {
        fireEvent.click(getByRole("button", { name: /add file/i }));
      });

      expect(mockOpen).toHaveBeenCalledWith(
        expect.objectContaining({ multiple: false, directory: false }),
      );
    });

    it("fires onChange with path appended when open() resolves with paths", async () => {
      mockOpen.mockResolvedValue(["/tmp/newfile.txt"]);
      const onChange = vi.fn();
      const { getByRole } = renderEditor({
        value: SAMPLE_ATTACHMENTS,
        onChange,
      });

      await act(async () => {
        fireEvent.click(getByRole("button", { name: /add file/i }));
      });

      // onChange called with existing attachments plus the new path
      expect(onChange).toHaveBeenCalledWith([
        ...SAMPLE_ATTACHMENTS,
        "/tmp/newfile.txt",
      ]);
    });

    it("does NOT fire onChange when open() resolves with null (cancelled)", async () => {
      mockOpen.mockResolvedValue(null);
      const onChange = vi.fn();
      const { getByRole } = renderEditor({
        value: SAMPLE_ATTACHMENTS,
        onChange,
      });

      await act(async () => {
        fireEvent.click(getByRole("button", { name: /add file/i }));
      });

      expect(onChange).not.toHaveBeenCalled();
    });

    it("appends multiple files when open() returns multiple paths", async () => {
      mockOpen.mockResolvedValue(["/tmp/a.txt", "/tmp/b.txt"]);
      const onChange = vi.fn();
      const { getByRole } = renderEditor({
        value: [],
        onChange,
      });

      await act(async () => {
        fireEvent.click(getByRole("button", { name: /add file/i }));
      });

      expect(onChange).toHaveBeenCalledWith(["/tmp/a.txt", "/tmp/b.txt"]);
    });
  });

  describe("normalizeAttachments validation", () => {
    it("filters out numbers from an attachment array", () => {
      const onChange = vi.fn();
      const { queryByText } = renderEditor({
        value: [SAMPLE_ATTACHMENTS[0], 42, SAMPLE_ATTACHMENTS[1]],
      });

      // Valid attachments should render, invalid ones silently dropped
      expect(queryByText("readme.md")).toBeTruthy();
      expect(queryByText("screenshot.png")).toBeTruthy();
    });

    it("filters out objects without an id property", () => {
      const { queryByText } = renderEditor({
        value: [SAMPLE_ATTACHMENTS[0], { notAnId: "oops" }],
      });

      expect(queryByText("readme.md")).toBeTruthy();
    });

    it("keeps string paths in the array", () => {
      const { getByText } = renderEditor({
        value: ["/tmp/new-file.txt"],
      });

      // String paths render as the full path
      expect(getByText("/tmp/new-file.txt")).toBeTruthy();
    });

    it("drops a bare number passed as single value", () => {
      const { getByText } = renderEditor({ value: 99 });

      // Should render empty state since the number is filtered out
      expect(getByText(/no attachments/i)).toBeTruthy();
    });

    it("keeps a single string value", () => {
      const { getByText } = renderEditor({ value: "/tmp/solo.txt" });

      expect(getByText("/tmp/solo.txt")).toBeTruthy();
    });

    it("keeps a single AttachmentMeta object", () => {
      const { getByText } = renderEditor({ value: SAMPLE_ATTACHMENTS[0] });

      expect(getByText("readme.md")).toBeTruthy();
    });
  });

  describe("file drop zone", () => {
    it("shows highlight class when isDragging is true from context", () => {
      const { container } = render(
        <FileDropTestHarness isDragging={true}>
          <AttachmentEditor
            field={ATTACHMENT_FIELD}
            value={[]}
            onCommit={vi.fn()}
            onCancel={vi.fn()}
            onChange={vi.fn()}
            mode="compact"
          />
        </FileDropTestHarness>,
      );

      // The outermost container div should have the drop-highlight class
      const dropZone = container.querySelector("[data-file-drop-zone]");
      expect(dropZone).toBeTruthy();
      expect(dropZone!.className).toContain("ring-2");
    });

    it("does NOT show highlight class when isDragging is false", () => {
      const { container } = render(
        <FileDropTestHarness isDragging={false}>
          <AttachmentEditor
            field={ATTACHMENT_FIELD}
            value={[]}
            onCommit={vi.fn()}
            onCancel={vi.fn()}
            onChange={vi.fn()}
            mode="compact"
          />
        </FileDropTestHarness>,
      );

      const dropZone = container.querySelector("[data-file-drop-zone]");
      expect(dropZone).toBeTruthy();
      expect(dropZone!.className).not.toContain("ring-2");
    });
  });
});
