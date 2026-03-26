import { useRef, useEffect, forwardRef } from "react";

type FocusHighlightProps = {
  /** Whether this element is focused — controls data-focused attribute and scroll behavior. */
  focused: boolean;
  /** HTML tag to render. Defaults to "div". */
  as?: "div" | "section";
} & Omit<React.HTMLAttributes<HTMLElement>, "ref">;

/**
 * Low-level visual primitive: sets `data-focused` and scrolls into view.
 *
 * For entity focus, use FocusScope instead — it is the single decorator
 * that owns focus identity, command scope, and visual rendering.
 * FocusHighlight is only used directly for non-entity focus indicators
 * (e.g. column headers, inspector field navigation).
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
