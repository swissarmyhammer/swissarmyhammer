# Fix Table Alignment in CLI Output

## Problem
Table output in the CLI is not aligning properly. Columns are misaligned with their separators, and rows appear to be cut off or incorrectly formatted.

## Example of Current Broken Output
```
├────────┼─────────────────────────────────────────────┼────────────────────────────────────────────────────────────────────────────────────────────────────────┤
│ ✅     │ Workflows Directory                         │ 0 items                                                                                                │
├────────┼─────────────────────────────────────────────┼────────────────────────────────────────────────────────────────────────────────────────────────────────┤
│ ⚠️     │ Prompts Directory                           │ Will be created when needed                                                                            │
├────────┼─────────────────────────────────────────────┼────────────────────────────────────────────────────────────────────────────────────────────────────────┤
│ ⚠️     │ Semantic Database                           │ Will be created when needed                                                                            │
├────────┼─────────────────────────────────────────────┼────────────────────────────────────────────────────────────────────────────────────────────────────────┤
│ ✅     │ Installation Method   
```

## Root Cause
The table library is likely not being used correctly, or table formatting logic is manually constructed instead of using the proper table rendering library.

## Requirements

1. **Use Table Library Correctly**: Ensure the code is using the table/formatting library (e.g., `comfy-table`, `tabled`, or similar) as intended
2. **Proper Column Width Calculation**: Calculate column widths based on content to ensure proper alignment
3. **Unicode Handling**: Ensure emoji and unicode characters (✅, ⚠️) are accounted for in width calculations
4. **Consistent Separators**: All row separators should align with column boundaries
5. **Complete Rows**: Ensure all rows are fully rendered and not cut off

## Investigation Steps

1. Locate the code generating this table output
2. Verify which table library is being used
3. Check if table is being constructed manually vs. using library methods
4. Review width calculation logic, especially for unicode/emoji characters
5. Test with various terminal widths and content lengths

## Expected Behavior
Tables should render with properly aligned columns, consistent separators, and all content fully displayed within the appropriate cells.