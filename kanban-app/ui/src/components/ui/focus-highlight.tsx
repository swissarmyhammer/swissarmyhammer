import { useRef, useEffect, forwardRef } from "react";

type FocusHighlightProps = {
  /** Whether this element is focused — controls data-focused attribute and scroll behavior. */
  focused: boolean;
  /** HTML tag to render. Defaults to "div". */
  as?: "div" | "section";
} & Omit<React.HTMLAttributes<HTMLElement>, "ref">;

/**
 * Shared visual focus primitive used across the app.
 *
 * Sets `data-focused` when active, which is styled globally via
 * `[data-focused] { filter: brightness(0.97) }` in index.css.
 *
 * Also scrolls the element into view when it becomes focused.
 * No borders, rings, or layout-shifting effects — just the brightness filter.
 */
export const FocusHighlight = forwardRef<HTMLElement, FocusHighlightProps>(
  function FocusHighlight(
    { focused, as: Tag = "div", children, ...rest },
    forwardedRef,
  ) {
    const internalRef = useRef<HTMLElement>(null);
    const ref = (forwardedRef ?? internalRef) as React.RefObject<HTMLElement>;

    useEffect(() => {
      if (focused && ref.current?.scrollIntoView) {
        ref.current.scrollIntoView({ block: "nearest" });
      }
    }, [focused, ref]);

    return (
      <Tag
        ref={ref as React.RefObject<HTMLDivElement>}
        data-focused={focused || undefined}
        {...rest}
      >
        {children}
      </Tag>
    );
  },
);
