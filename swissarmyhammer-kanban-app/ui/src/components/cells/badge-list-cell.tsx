/** Multi-badge cell for computed tag lists and other string arrays. */
export function BadgeListCell({ value }: { value: unknown }) {
  const items = Array.isArray(value) ? (value as string[]) : [];
  if (items.length === 0) return <span className="text-muted-foreground/50">-</span>;
  return (
    <div className="flex flex-wrap gap-1">
      {items.map((item) => (
        <span
          key={item}
          className="inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium bg-muted text-muted-foreground"
        >
          {item}
        </span>
      ))}
    </div>
  );
}
