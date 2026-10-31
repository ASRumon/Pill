import type { WidgetKey } from "../types";

interface WidgetTheme {
  label: string;
  hint: string;
  accent: string;
  from: string;
  to: string;
}

export const widgetTheme: Record<WidgetKey, WidgetTheme> = {
  clipboard: {
    label: "Clipboard",
    hint: "Capture · convert · drag out",
    accent: "#38e1ff",
    from: "#22d3ee",
    to: "#3b82f6",
  },
  shelf: {
    label: "File Shelf",
    hint: "Drop files to stash them",
    accent: "#c084fc",
    from: "#a855f7",
    to: "#6366f1",
  },
  search: {
    label: "Search",
    hint: "Everything files · web engines",
    accent: "#facc15",
    from: "#fde047",
    to: "#f59e0b",
  },
  todo: {
    label: "Tasks",
    hint: "Today's progress",
    accent: "#4ade80",
    from: "#34d399",
    to: "#10b981",
  },
  media: {
    label: "Now Playing",
    hint: "System media session",
    accent: "#fb7185",
    from: "#fb7185",
    to: "#f43f5e",
  },
  pomodoro: {
    label: "Focus",
    hint: "25 minute sessions",
    accent: "#fbbf24",
    from: "#fbbf24",
    to: "#f97316",
  },
};

interface WidgetIconProps {
  widget: WidgetKey;
  className?: string;
}

export function WidgetIcon({ widget, className }: WidgetIconProps) {
  const props = {
    className,
    viewBox: "0 0 24 24",
    fill: "none",
    stroke: "currentColor",
    strokeWidth: 1.7,
    strokeLinecap: "round" as const,
    strokeLinejoin: "round" as const,
  };

  switch (widget) {
    case "clipboard":
      return (
        <svg {...props}>
          <rect x="7" y="4" width="10" height="4" rx="1.5" />
          <path d="M17 6h1.6A1.4 1.4 0 0 1 20 7.4v11.2A1.4 1.4 0 0 1 18.6 20H5.4A1.4 1.4 0 0 1 4 18.6V7.4A1.4 1.4 0 0 1 5.4 6H7" />
          <path d="M8.5 12h7M8.5 15.5h4.5" />
        </svg>
      );
    case "shelf":
      return (
        <svg {...props}>
          <path d="M4 8.5A1.5 1.5 0 0 1 5.5 7h3.1l1.4 1.7h8.5A1.5 1.5 0 0 1 20 10.2v6.3A1.5 1.5 0 0 1 18.5 18h-13A1.5 1.5 0 0 1 4 16.5z" />
          <path d="M4 12.5h16" />
        </svg>
      );
    case "todo":
      return (
        <svg {...props}>
          <circle cx="12" cy="12" r="8" />
          <path d="M8.6 12.3l2.3 2.2 4.5-4.6" />
        </svg>
      );
    case "search":
      return (
        <svg {...props}>
          <circle cx="10.8" cy="10.8" r="6.3" />
          <path d="m15.4 15.4 4 4" />
        </svg>
      );
    case "media":
      return (
        <svg {...props}>
          <circle cx="8" cy="17" r="2.4" />
          <circle cx="17.5" cy="14.6" r="2.4" />
          <path d="M10.4 17V7.6l9.5-1.9v8.9" />
        </svg>
      );
    case "pomodoro":
      return (
        <svg {...props}>
          <circle cx="12" cy="13.5" r="7" />
          <path d="M12 10.2v3.5l2.2 1.4M9.6 3.4h4.8" />
        </svg>
      );
    default:
      return null;
  }
}

export function ActionIcon({ name, className }: { name: "add" | "delete" | "reset" | "drop" | "grid" | "spark" | "clock" | "chevron" | "close" | "check"; className?: string }) {
  const props = { viewBox: "0 0 24 24", className, fill: "none", stroke: "currentColor", strokeWidth: 1.8, strokeLinecap: "round" as const, strokeLinejoin: "round" as const };
  switch (name) {
    case "add": return <svg {...props}><path d="M12 5v14M5 12h14" /></svg>;
    case "chevron": return <svg {...props}><path d="m9 5 7 7-7 7" /></svg>;
    case "close": return <svg {...props}><path d="M6 6l12 12M18 6 6 18" /></svg>;
    case "delete": return <svg {...props}><path d="M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3" /></svg>;
    case "reset": return <svg {...props}><path d="M4.5 8a8 8 0 1 1-.8 6M4 4v4h4" /><path d="M12 8v4l2.5 1.5" /></svg>;
    case "drop": return <svg {...props}><path d="M12 3.5c3.1 3.7 5.5 6.7 5.5 9.7a5.5 5.5 0 1 1-11 0c0-3 2.4-6 5.5-9.7Z" /><path d="M9.5 13.5A2.5 2.5 0 0 0 12 16" /></svg>;
    case "grid": return <svg {...props}><rect x="4.5" y="4.5" width="6.2" height="6.2" rx="1.1" /><rect x="13.3" y="4.5" width="6.2" height="6.2" rx="1.1" /><rect x="4.5" y="13.3" width="6.2" height="6.2" rx="1.1" /><rect x="13.3" y="13.3" width="6.2" height="6.2" rx="1.1" /></svg>;
    case "spark": return <svg {...props}><path d="m12 3.8 1.6 3.5L17 9l-3.4 1.8L12 14.4l-1.6-3.6L7 9l3.4-1.7Z" /><path d="M18.5 14.5 19.4 16l1.6.9-1.6.9-.9 1.6-.9-1.6-1.6-.9 1.6-.9zM4.2 14.5l.8 1.5 1.6.9-1.6.9-.8 1.6-.9-1.6-1.6-.9 1.6-.9z" /></svg>;
    case "clock": return <svg {...props}><circle cx="12" cy="12" r="8.6" /><path d="M12 7.6v4.8l3 2" /></svg>;
    case "check": return <svg {...props} strokeWidth={2.6}><path d="m5 12.6 4.4 4.4L19 7.4" /></svg>;
  }
}
