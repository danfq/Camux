import { ItemGroup } from "@/components/ui/item";
import { cn } from "@/lib/utils";
import type { ComponentProps, ReactNode } from "react";

type SettingsSectionProps = Omit<ComponentProps<"section">, "title"> & {
  title: ReactNode;
};

export const SettingsSection = ({ title, children, className, ...props }: SettingsSectionProps) => {
  return (
    <section className={cn("flex w-full flex-col gap-3", className)} {...props}>
      <h2 className="text-xs font-medium text-muted-foreground">{title}</h2>
      <ItemGroup className="gap-1.5">{children}</ItemGroup>
    </section>
  );
};
