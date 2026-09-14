import { cn } from "@/lib/utils";

export type CameraStatusKind = "available" | "inUse" | "disconnected";

export const StatusIndicator = ({ status, className }: { status: CameraStatusKind; className?: string }) => {
  return (
    <span
      className={cn(
        "block size-1.5 shrink-0 animate-pulse rounded-full",
        status === "available" && "bg-emerald-500",
        status === "inUse" && "bg-amber-500",
        status === "disconnected" && "bg-red-500",
        className,
      )}
      aria-hidden="true"
    />
  );
};
