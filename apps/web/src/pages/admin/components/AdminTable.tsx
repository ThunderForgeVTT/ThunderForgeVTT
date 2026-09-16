/**
 * The admin pages' table, which is now the product's table.
 *
 * The implementation moved to `@/components/ui/data-table/DataTable` the day
 * the world archive needed the same thing — a wall of tiles that stops being
 * readable past six worlds is the same complaint the provider cards were, and
 * answering it with a second table would have given the product two tables
 * that drift. The name stays because "the admin table" is what the admin
 * pages call it and because a rename across a dozen call sites would be noise
 * in a diff about the world archive.
 */
export {
  DataTable as AdminTable,
  DataDetailRow as AdminDetailRow,
} from "@/components/ui/data-table/DataTable";
export type { DataTableProps as AdminTableProps } from "@/components/ui/data-table/DataTable";
