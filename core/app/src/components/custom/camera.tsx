import type { CameraDevice } from "@/core/backend";
import { Camera } from "lucide-react";
import { Badge } from "@/components/ui/badge";

export const CameraItem = ({ device }: { device: CameraDevice }) => {
  return (
    <li className="grid min-h-16 cursor-pointer grid-cols-[40px_minmax(0,1fr)_auto] items-center gap-3 px-4 py-2.5 transition-colors hover:bg-muted/50">
      <span className="grid size-10 place-items-center rounded-lg bg-muted text-muted-foreground">
        <Camera className="size-5" aria-hidden="true" />
      </span>
      <span className="min-w-0">
        <strong className="block truncate text-sm font-medium">{device.name}</strong>
        <small className="mt-1 block truncate font-mono text-[10px] text-muted-foreground">{device.id}</small>
      </span>
      <Badge variant="secondary" className="gap-1.5 text-[11px] font-normal text-muted-foreground">
        <span className="size-1.5 rounded-full bg-emerald-500" aria-hidden="true" />
        Available
      </Badge>
    </li>
  );
};
