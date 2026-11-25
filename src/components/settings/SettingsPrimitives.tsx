import { emit } from "@tauri-apps/api/event";
import { Label } from "@/components/ui/label";
import { RadioGroupItem } from "@/components/ui/radio-group";
import { Separator } from "@/components/ui/separator";
import { type ThemeMode } from "@/lib/ipc";
import { cn } from "@/lib/utils";

export function notifyStatus(message: string) {
  void emit("triex://status-message", message);
}

export function ContentSection({
  title,
  description,
  action,
  children,
}: {
  title: string;
  description: string;
  action?: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <div className="flex flex-col">
      <div className="flex items-start justify-between gap-4">
        <div className="min-w-0 space-y-0.5">
          <h3 className="text-[15px] font-semibold tracking-tight text-foreground">
            {title}
          </h3>
          <p className="text-xs text-muted-foreground">{description}</p>
        </div>
        {action && <div className="shrink-0">{action}</div>}
      </div>
      <Separator className="my-3.5 bg-border/70" />
      {children}
    </div>
  );
}

/**
 * Visual theme preview card aligned with the launcher's rounded-xl surface system.
 */
export function ThemePreviewOption({
  value,
  label,
  selected,
  variant,
}: {
  value: ThemeMode;
  label: string;
  selected: boolean;
  variant: "light" | "dark" | "system";
}) {
  const id = `theme-option-${value}`;

  return (
    <Label htmlFor={id} className="flex cursor-pointer flex-col items-center gap-2 font-normal">
      <RadioGroupItem id={id} value={value} className="sr-only" />
      <div
        className={cn(
          "w-full rounded-xl border-2 p-1 transition-colors",
          selected
            ? "border-primary ring-2 ring-primary/15"
            : "border-border/70 bg-surface hover:border-foreground/30",
        )}
      >
        {variant === "light" ? (
          <div className="space-y-1.5 rounded-sm bg-[#ecedef] p-2">
            <div className="space-y-1.5 rounded-md bg-white p-2 shadow-xs">
              <div className="h-1.5 w-12 rounded-lg bg-[#ecedef]" />
              <div className="h-1.5 w-16 rounded-lg bg-[#ecedef]" />
            </div>
            <div className="flex items-center gap-1.5 rounded-md bg-white p-2 shadow-xs">
              <div className="size-3 rounded-full bg-[#ecedef]" />
              <div className="h-1.5 w-14 rounded-lg bg-[#ecedef]" />
            </div>
          </div>
        ) : variant === "dark" ? (
          <div className="space-y-1.5 rounded-sm bg-slate-950 p-2">
            <div className="space-y-1.5 rounded-md bg-slate-800 p-2 shadow-xs">
              <div className="h-1.5 w-12 rounded-lg bg-slate-400" />
              <div className="h-1.5 w-16 rounded-lg bg-slate-400" />
            </div>
            <div className="flex items-center gap-1.5 rounded-md bg-slate-800 p-2 shadow-xs">
              <div className="size-3 rounded-full bg-slate-400" />
              <div className="h-1.5 w-14 rounded-lg bg-slate-400" />
            </div>
          </div>
        ) : (
          <div className="grid grid-cols-2 overflow-hidden rounded-sm">
            <div className="space-y-1.5 bg-[#ecedef] p-2 pr-1">
              <div className="space-y-1.5 rounded-l-md bg-white p-2 shadow-xs">
                <div className="h-1.5 w-6 rounded-lg bg-[#ecedef]" />
                <div className="h-1.5 w-8 rounded-lg bg-[#ecedef]" />
              </div>
              <div className="flex items-center gap-1 rounded-l-md bg-white p-2 shadow-xs">
                <div className="size-3 rounded-full bg-[#ecedef]" />
              </div>
            </div>
            <div className="space-y-1.5 bg-slate-950 p-2 pl-1">
              <div className="space-y-1.5 rounded-r-md bg-slate-800 p-2 shadow-xs">
                <div className="h-1.5 w-6 rounded-lg bg-slate-400" />
                <div className="h-1.5 w-8 rounded-lg bg-slate-400" />
              </div>
              <div className="flex items-center gap-1 rounded-r-md bg-slate-800 p-2 shadow-xs">
                <div className="h-1.5 w-6 rounded-lg bg-slate-400" />
              </div>
            </div>
          </div>
        )}
      </div>
      <span className="text-xs font-medium">{label}</span>
    </Label>
  );
}
