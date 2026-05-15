import { memo } from "react";
import CodeMirror from "@uiw/react-codemirror";
import { EditorState, type Extension } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { shadcnTheme } from "@/lib/cm-keymap";

/** Props for the read-only CM6 text viewer. */
export interface TextViewerProps {
  /** Document content; fully controlled — reconfigures when text changes. */
  text: string;
  /** CM6 extensions to attach (e.g. mention decorations, markdown language). */
  extensions?: Extension[];
  /** CSS class for the wrapping div. Defaults to "text-sm". */
  className?: string;
}

/** Static basicSetup config — all editor chrome disabled for read-only display. */
const BASIC_SETUP = {
  lineNumbers: false,
  foldGutter: false,
  highlightActiveLine: false,
  highlightActiveLineGutter: false,
  indentOnInput: false,
  bracketMatching: false,
  autocompletion: false,
} as const;

/** Read-only extensions — disable editing and content editability. */
const READ_ONLY_EXTENSIONS = [
  EditorState.readOnly.of(true),
  EditorView.editable.of(false),
];

/**
 * Minimal read-only CM6 viewer for rendering text with extensions.
 *
 * Unlike TextEditor, this component has no editing concerns — no keymaps,
 * no submit/cancel, no vim mode, no onChange. It simply mounts CM6 in
 * read-only mode with the caller's extensions and the shared shadcn theme.
 *
 * Returns null when text is empty so callers don't get an empty editor chrome.
 */
export const TextViewer = memo(function TextViewer({
  text,
  extensions,
  className = "text-sm",
}: TextViewerProps) {
  if (!text) return null;

  return (
    <CodeMirror
      value={text}
      editable={false}
      readOnly={true}
      extensions={[...READ_ONLY_EXTENSIONS, ...(extensions ?? [])]}
      theme={shadcnTheme}
      basicSetup={BASIC_SETUP}
      className={className}
    />
  );
});
