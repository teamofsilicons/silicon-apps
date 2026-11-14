/**
 * Each category's icon (lucide, as the account site's sections): the sidebar's rows, and wherever a category is named
 * beside an icon. One for any category without its own.
 */
import { Blocks, Bot, Compass, ImageIcon, LayoutGrid, ListTodo, MessageCircle, PenLine, type LucideIcon } from "lucide-react";

const ICONS: Record<string, LucideIcon> = {
  essentials: Compass,
  communication: MessageCircle,
  productivity: PenLine,
  business: ListTodo,
  "ai-agents": Bot,
  "developer-tools": Blocks,
  media: ImageIcon,
};

export function CategoryIcon({ id, size = 16 }: { id: string; size?: number }) {
  const Icon = ICONS[id] ?? LayoutGrid;
  return <Icon size={size} strokeWidth={2} aria-hidden="true" />;
}
