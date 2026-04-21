import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Mocks — must be declared before importing the component under test
// ---------------------------------------------------------------------------

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const mockInvoke = vi.fn(
  (..._args: any[]): Promise<any> => Promise.resolve("ok"),
);
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: any[]) => mockInvoke(...args),
}));
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const mockListen = vi.fn((..._args: any[]) => Promise.resolve(() => {}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: (...args: any[]) => mockListen(...args),
}));
vi.mock("@tauri-apps/api/webviewWindow", () => ({
  getCurrentWebviewWindow: () => ({
    label: "main",
    listen: vi.fn(() => Promise.resolve(() => {})),
  }),
}));
vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({
    onDragDropEvent: vi.fn(() => Promise.resolve(() => {})),
  }),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ label: "main" }),
}));
// ---------------------------------------------------------------------------

import {
  AttachmentDisplay,
  AttachmentListDisplay,
  AttachmentItem,
  formatFileSize,
  getFileIcon,
  type AttachmentMeta,
} from "./attachment-display";
import { FileDropProvider } from "@/lib/file-drop-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";

/** Wraps component in providers needed by hooks (useDispatchCommand, useFileDrop). */
function Wrapper({ children }: { children: React.ReactNode }) {
  return (
    <EntityFocusProvider>
      <FileDropProvider>{children}</FileDropProvider>
    </EntityFocusProvider>
  );
}
import {
  File,
  FileImage,
  FileCode,
  FileText,
  FileVideo,
  FileAudio,
  FileSpreadsheet,
  FileArchive,
} from "lucide-react";

// ---------------------------------------------------------------------------
// Test data
// ---------------------------------------------------------------------------

const imageAttachment: AttachmentMeta = {
  id: "att-1",
  name: "screenshot.png",
  size: 12345,
  mime_type: "image/png",
  path: "/path/to/.kanban/tasks/.attachments/att-1-screenshot.png",
};

const codeAttachment: AttachmentMeta = {
  id: "att-2",
  name: "main.rs",
  size: 2048,
  mime_type: "text/x-rust",
  path: "/path/to/.kanban/tasks/.attachments/att-2-main.rs",
};

const unknownAttachment: AttachmentMeta = {
  id: "att-3",
  name: "data.bin",
  size: 1048576,
  mime_type: "application/octet-stream",
  path: "/path/to/.kanban/tasks/.attachments/att-3-data.bin",
};

// ---------------------------------------------------------------------------
// formatFileSize
// ---------------------------------------------------------------------------

describe("formatFileSize", () => {
  it("formats bytes", () => {
    expect(formatFileSize(0)).toBe("0 B");
    expect(formatFileSize(500)).toBe("500 B");
  });

  it("formats kilobytes", () => {
    expect(formatFileSize(1024)).toBe("1.0 KB");
    expect(formatFileSize(12345)).toBe("12.1 KB");
  });

  it("formats megabytes", () => {
    expect(formatFileSize(1048576)).toBe("1.0 MB");
  });

  it("formats gigabytes", () => {
    expect(formatFileSize(1073741824)).toBe("1.0 GB");
  });
});

// ---------------------------------------------------------------------------
// getFileIcon
// ---------------------------------------------------------------------------

describe("getFileIcon", () => {
  it("returns FileImage for image MIME types", () => {
    expect(getFileIcon("image/png", "photo.png")).toBe(FileImage);
    expect(getFileIcon("image/jpeg", "photo.jpg")).toBe(FileImage);
  });

  it("returns FileVideo for video MIME types", () => {
    expect(getFileIcon("video/mp4", "demo.mp4")).toBe(FileVideo);
  });

  it("returns FileAudio for audio MIME types", () => {
    expect(getFileIcon("audio/mpeg", "song.mp3")).toBe(FileAudio);
  });

  it("returns FileText for text MIME types", () => {
    expect(getFileIcon("text/plain", "readme.txt")).toBe(FileText);
  });

  it("returns FileCode for code extensions with text MIME type", () => {
    expect(getFileIcon("text/x-rust", "main.rs")).toBe(FileCode);
    expect(getFileIcon("text/x-python", "script.py")).toBe(FileCode);
  });

  it("returns FileText for application/pdf", () => {
    expect(getFileIcon("application/pdf", "doc.pdf")).toBe(FileText);
  });

  it("returns FileCode for code MIME types", () => {
    expect(getFileIcon("application/javascript", "app.js")).toBe(FileCode);
    expect(getFileIcon("application/json", "data.json")).toBe(FileCode);
  });

  it("returns FileCode for code extensions with unknown MIME", () => {
    expect(getFileIcon("application/octet-stream", "main.ts")).toBe(FileCode);
    expect(getFileIcon("application/octet-stream", "lib.go")).toBe(FileCode);
  });

  it("returns FileSpreadsheet for spreadsheet extensions", () => {
    expect(getFileIcon("application/octet-stream", "data.csv")).toBe(
      FileSpreadsheet,
    );
    expect(getFileIcon("application/vnd.ms-excel", "report.xlsx")).toBe(
      FileSpreadsheet,
    );
  });

  it("returns FileArchive for archive extensions", () => {
    expect(getFileIcon("application/octet-stream", "backup.zip")).toBe(
      FileArchive,
    );
    expect(getFileIcon("application/octet-stream", "archive.tar")).toBe(
      FileArchive,
    );
  });

  it("returns File for unknown types", () => {
    expect(getFileIcon("application/octet-stream", "data.bin")).toBe(File);
  });
});

// ---------------------------------------------------------------------------
// AttachmentItem
// ---------------------------------------------------------------------------

describe("AttachmentItem", () => {
  it("renders filename and size", () => {
    render(
      <Wrapper>
        <AttachmentItem attachment={imageAttachment} />
      </Wrapper>,
    );
    expect(screen.getByText("screenshot.png")).toBeTruthy();
    expect(screen.getByText("12.1 KB")).toBeTruthy();
  });

  it("has cursor-pointer class for interactivity", () => {
    const { container } = render(
      <Wrapper>
        <AttachmentItem attachment={imageAttachment} />
      </Wrapper>,
    );
    expect(container.querySelector(".cursor-pointer")).toBeTruthy();
  });

  it("calls dispatch_command on double-click", async () => {
    mockInvoke.mockClear();
    const { container } = render(
      <Wrapper>
        <AttachmentItem attachment={imageAttachment} />
      </Wrapper>,
    );
    fireEvent.doubleClick(container.querySelector(".cursor-pointer")!);
    await vi.waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        "dispatch_command",
        expect.objectContaining({ cmd: "attachment.open" }),
      );
    });
  });

  it("shows context menu with Open and Show in Finder on right-click", async () => {
    mockInvoke.mockClear();
    mockListen.mockClear();
    mockListen.mockResolvedValue(() => {});
    // list_commands_for_scope returns resolved commands from the backend
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    mockInvoke.mockImplementation((cmd: any) => {
      if (cmd === "list_commands_for_scope") {
        return Promise.resolve([
          {
            id: "attachment.open",
            name: "Open",
            target: imageAttachment.path,
            group: "attachment",
          },
          {
            id: "attachment.reveal",
            name: "Show in Finder",
            target: imageAttachment.path,
            group: "attachment",
          },
        ]);
      }
      return Promise.resolve("ok");
    });
    const { container } = render(
      <Wrapper>
        <AttachmentItem attachment={imageAttachment} />
      </Wrapper>,
    );
    fireEvent.contextMenu(container.querySelector(".cursor-pointer")!);
    await vi.waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("show_context_menu", {
        items: [
          expect.objectContaining({
            cmd: "attachment.open",
            name: "Open",
            separator: false,
          }),
          expect.objectContaining({
            cmd: "attachment.reveal",
            name: "Show in Finder",
            separator: false,
          }),
        ],
      });
    });
    // Restore default mock
    mockInvoke.mockImplementation(() => Promise.resolve("ok"));
  });

  it("scope chain includes attachment moniker on right-click", async () => {
    mockInvoke.mockClear();
    mockListen.mockResolvedValue(() => {});
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    mockInvoke.mockImplementation((cmd: any) => {
      if (cmd === "list_commands_for_scope") return Promise.resolve([]);
      return Promise.resolve("ok");
    });
    const { container } = render(
      <Wrapper>
        <AttachmentItem attachment={imageAttachment} />
      </Wrapper>,
    );
    fireEvent.contextMenu(container.querySelector(".cursor-pointer")!);
    await vi.waitFor(() => {
      const call = mockInvoke.mock.calls.find(
        (c: unknown[]) => c[0] === "list_commands_for_scope",
      );
      expect(call).toBeDefined();
      const { scopeChain } = call![1] as { scopeChain: string[] };
      expect(scopeChain[0]).toBe(`attachment:${imageAttachment.path}`);
    });
    mockInvoke.mockImplementation(() => Promise.resolve("ok"));
  });

  it("nested inside parent FocusScope: right-click fires attachment scope, not parent", async () => {
    mockInvoke.mockClear();
    mockListen.mockResolvedValue(() => {});
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    mockInvoke.mockImplementation((cmd: any) => {
      if (cmd === "list_commands_for_scope") return Promise.resolve([]);
      return Promise.resolve("ok");
    });
    const FocusScope = (await import("@/components/focus-scope")).FocusScope;
    const { container } = render(
      <Wrapper>
        <FocusScope moniker="task:01ABC" commands={[]}>
          <AttachmentItem attachment={imageAttachment} />
        </FocusScope>
      </Wrapper>,
    );
    fireEvent.contextMenu(container.querySelector(".cursor-pointer")!);
    await vi.waitFor(() => {
      const calls = mockInvoke.mock.calls.filter(
        (c: unknown[]) => c[0] === "list_commands_for_scope",
      );
      // Should be exactly one call — the attachment's, not the parent task's
      expect(calls).toHaveLength(1);
      const { scopeChain } = calls[0][1] as { scopeChain: string[] };
      // Attachment moniker should be first (innermost)
      expect(scopeChain[0]).toBe(`attachment:${imageAttachment.path}`);
      // Parent task moniker should be second
      expect(scopeChain[1]).toBe("task:01ABC");
    });
    mockInvoke.mockImplementation(() => Promise.resolve("ok"));
  });
});

// ---------------------------------------------------------------------------
// AttachmentDisplay (single)
// ---------------------------------------------------------------------------

describe("AttachmentDisplay", () => {
  it("renders a single attachment", () => {
    render(<AttachmentDisplay value={imageAttachment} mode="full" />, {
      wrapper: Wrapper,
    });
    expect(screen.getByText("screenshot.png")).toBeTruthy();
    expect(screen.getByText("12.1 KB")).toBeTruthy();
  });

  it("renders empty state in full mode with drop hint", () => {
    render(<AttachmentDisplay value={null} mode="full" />, {
      wrapper: Wrapper,
    });
    expect(screen.getByText("Drop file here")).toBeTruthy();
  });

  it("renders dash in compact mode for empty", () => {
    render(<AttachmentDisplay value={null} mode="compact" />, {
      wrapper: Wrapper,
    });
    expect(screen.getByText("-")).toBeTruthy();
  });

  it("shows dashed border when empty", () => {
    const { container } = render(
      <AttachmentDisplay value={null} mode="full" />,
      { wrapper: Wrapper },
    );
    const zone = container.querySelector(".border-dashed");
    expect(zone).toBeTruthy();
  });

  it("shows highlight border when dragging", () => {
    const { container } = render(
      <FileDropProvider _testOverride={{ isDragging: true }}>
        <AttachmentDisplay value={null} mode="full" />
      </FileDropProvider>,
    );
    const zone = container.querySelector(".border-primary\\/60");
    expect(zone).toBeTruthy();
  });
});

// ---------------------------------------------------------------------------
// AttachmentListDisplay (multiple)
// ---------------------------------------------------------------------------

describe("AttachmentListDisplay", () => {
  it("renders multiple attachments with filenames and sizes", () => {
    const attachments = [imageAttachment, codeAttachment, unknownAttachment];
    render(<AttachmentListDisplay value={attachments} mode="full" />, {
      wrapper: Wrapper,
    });
    expect(screen.getByText("screenshot.png")).toBeTruthy();
    expect(screen.getByText("12.1 KB")).toBeTruthy();
    expect(screen.getByText("main.rs")).toBeTruthy();
    expect(screen.getByText("2.0 KB")).toBeTruthy();
    expect(screen.getByText("data.bin")).toBeTruthy();
    expect(screen.getByText("1.0 MB")).toBeTruthy();
  });

  it("renders empty state in full mode with drop hint", () => {
    render(<AttachmentListDisplay value={[]} mode="full" />, {
      wrapper: Wrapper,
    });
    expect(screen.getByText("Drop files here")).toBeTruthy();
  });

  it("renders dash in compact mode for empty", () => {
    render(<AttachmentListDisplay value={[]} mode="compact" />, {
      wrapper: Wrapper,
    });
    expect(screen.getByText("-")).toBeTruthy();
  });

  it("handles non-array value gracefully", () => {
    render(<AttachmentListDisplay value={null} mode="full" />, {
      wrapper: Wrapper,
    });
    expect(screen.getByText("Drop files here")).toBeTruthy();
  });

  it("shows highlight border when dragging", () => {
    const { container } = render(
      <FileDropProvider _testOverride={{ isDragging: true }}>
        <AttachmentListDisplay value={[]} mode="full" />
      </FileDropProvider>,
    );
    const zone = container.querySelector(".border-primary\\/60");
    expect(zone).toBeTruthy();
  });

  it("renders full enriched metadata shape with filenames and sizes", () => {
    const enrichedAttachments: AttachmentMeta[] = [
      {
        id: "01ENRICH1",
        name: "document.pdf",
        size: 51200,
        mime_type: "application/pdf",
        path: "/data/.kanban/tasks/.attachments/01ENRICH1-document.pdf",
      },
      {
        id: "01ENRICH2",
        name: "photo.jpg",
        size: 2097152,
        mime_type: "image/jpeg",
        path: "/data/.kanban/tasks/.attachments/01ENRICH2-photo.jpg",
      },
      {
        id: "01ENRICH3",
        name: "tiny.txt",
        size: 42,
        mime_type: "text/plain",
        path: "/data/.kanban/tasks/.attachments/01ENRICH3-tiny.txt",
      },
    ];

    render(<AttachmentListDisplay value={enrichedAttachments} mode="full" />, {
      wrapper: Wrapper,
    });

    // Verify all filenames render
    expect(screen.getByText("document.pdf")).toBeTruthy();
    expect(screen.getByText("photo.jpg")).toBeTruthy();
    expect(screen.getByText("tiny.txt")).toBeTruthy();

    // Verify formatted sizes render
    expect(screen.getByText("50.0 KB")).toBeTruthy();
    expect(screen.getByText("2.0 MB")).toBeTruthy();
    expect(screen.getByText("42 B")).toBeTruthy();
  });
});
