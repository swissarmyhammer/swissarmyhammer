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
vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({
    onDragDropEvent: vi.fn(() => Promise.resolve(() => {})),
  }),
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

/** Wraps component in FileDropProvider for display tests that use useFileDrop. */
function Wrapper({ children }: { children: React.ReactNode }) {
  return <FileDropProvider>{children}</FileDropProvider>;
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
    render(<AttachmentItem attachment={imageAttachment} />);
    expect(screen.getByText("screenshot.png")).toBeTruthy();
    expect(screen.getByText("12.1 KB")).toBeTruthy();
  });

  it("has cursor-pointer class for interactivity", () => {
    const { container } = render(
      <AttachmentItem attachment={imageAttachment} />,
    );
    const item = container.firstElementChild;
    expect(item?.className).toContain("cursor-pointer");
  });

  it("calls dispatch_command on double-click", async () => {
    mockInvoke.mockClear();
    const { container } = render(
      <AttachmentItem attachment={imageAttachment} />,
    );
    fireEvent.doubleClick(container.firstElementChild!);
    // backendDispatch calls invoke("dispatch_command", ...) asynchronously
    await vi.waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("dispatch_command", {
        cmd: "attachment.open",
        scopeChain: [`attachment:${imageAttachment.path}`],
      });
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
      <AttachmentItem attachment={imageAttachment} />,
    );
    fireEvent.contextMenu(container.firstElementChild!);
    await vi.waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("show_context_menu", {
        items: [
          { id: `attachment.open:${imageAttachment.path}`, name: "Open" },
          {
            id: `attachment.reveal:${imageAttachment.path}`,
            name: "Show in Finder",
          },
        ],
      });
    });
    // Restore default mock
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
});
