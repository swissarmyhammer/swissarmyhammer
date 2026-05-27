import * as React from "react";
import { Label as LabelPrimitive } from "radix-ui";

import { cn } from "@/lib/utils";

/**
 * A form label wrapping the Radix `Label` primitive.
 *
 * Associates with a control via the standard `htmlFor`/`id` pairing and shares
 * the disabled-dimming behavior the rest of the shadcn controls use: when the
 * label sits in a `group` whose peer is disabled, or wraps a disabled control,
 * it dims and drops pointer events.
 */
function Label({
  className,
  ...props
}: React.ComponentProps<typeof LabelPrimitive.Root>) {
  return (
    <LabelPrimitive.Root
      data-slot="label"
      className={cn(
        "flex items-center gap-2 text-sm leading-none font-medium select-none group-data-[disabled=true]:pointer-events-none group-data-[disabled=true]:opacity-50 peer-disabled:cursor-not-allowed peer-disabled:opacity-50",
        className,
      )}
      {...props}
    />
  );
}

export { Label };
