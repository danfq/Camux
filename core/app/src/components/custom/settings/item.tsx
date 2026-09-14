import { Item, ItemActions, ItemContent, ItemDescription, ItemTitle } from "@/components/ui/item";
import { cn } from "@/lib/utils";
import type { ComponentProps, ReactNode } from "react";

type SettingsItemProps = Omit<ComponentProps<typeof Item>, "children" | "title"> & {
  title: ReactNode;
  description?: ReactNode;
  children?: ReactNode;
};

export const SettingsItem = ({ title, description, children, className, ...props }: SettingsItemProps) => {
  return (
    <Item className={cn("min-h-8 px-0 py-0", className)} {...props}>
      <ItemContent>
        <ItemTitle>{title}</ItemTitle>
        {description !== undefined && <ItemDescription>{description}</ItemDescription>}
      </ItemContent>
      {children !== undefined && <ItemActions>{children}</ItemActions>}
    </Item>
  );
};
