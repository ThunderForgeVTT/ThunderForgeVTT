/**
 * Collections on the shelf: authored content beside the books read in (spec
 * 050 FR-007 to FR-009c).
 *
 * "Shelf collection" on the wire because spec 026 already owns "collection"
 * for a world's own gathering of its artifacts. On the page it is simply a
 * collection, beside books.
 *
 * Every rule is the server's: which systems a collection may be kept for,
 * which kinds go in it, that a book read in cannot be written to or
 * downloaded, and that a download names anything it left out.
 */
import { postGraphQL } from "@/api/graphqlClient";
import type { Compendium } from "@/api/compendium";
import type { ReadValue } from "@/engine/sdk/ReadValue";

/** Start a collection on your shelf, for one game system. */
export async function createShelfCollection(
  title: string,
  systemId: string,
): Promise<Compendium> {
  const data = await postGraphQL<{ createShelfCollection: Compendium }>(
    `mutation CreateShelfCollection($title: String!, $systemId: String!) {
       createShelfCollection(title: $title, systemId: $systemId) {
         id
         bookTitle
         sourceHash
         systemId
         origin
         pageCount
         silentPageCount
         entryTotal
         entryCounts { kind count }
         baseVersion
       }
     }`,
    { title, systemId },
  );
  return data.createShelfCollection;
}

/** Write an entry into one of your collections: fields or prose, not both. */
export async function writeShelfCollectionEntry(input: {
  collectionId: string;
  kind: string;
  name: string;
  fieldValues?: Record<string, ReadValue> | null;
  proseText?: string | null;
}): Promise<void> {
  await postGraphQL<{ writeShelfCollectionEntry: { id: string } }>(
    `mutation WriteShelfCollectionEntry(
       $collectionId: UUID!
       $kind: String!
       $name: String!
       $fieldValues: JSON
       $proseText: String
     ) {
       writeShelfCollectionEntry(
         collectionId: $collectionId
         kind: $kind
         name: $name
         fieldValues: $fieldValues
         proseText: $proseText
       ) {
         id
       }
     }`,
    {
      collectionId: input.collectionId,
      kind: input.kind,
      name: input.name,
      fieldValues: input.fieldValues ?? null,
      proseText: input.proseText ?? null,
    },
  );
}

/** Take an entry out of one of your collections. */
export async function removeShelfCollectionEntry(
  collectionId: string,
  entryId: string,
): Promise<void> {
  await postGraphQL<{ removeShelfCollectionEntry: boolean }>(
    `mutation RemoveShelfCollectionEntry($collectionId: UUID!, $entryId: UUID!) {
       removeShelfCollectionEntry(collectionId: $collectionId, entryId: $entryId)
     }`,
    { collectionId, entryId },
  );
}

/** A collection as a file, and anything the server left out of it. */
export interface ShelfCollectionDownload {
  fileName: string;
  contents: string;
  entryCount: number;
  excluded: { kind: string; name: string; reason: string }[];
}

/**
 * One of your collections as JSON (FR-009a). A book read in is refused by the
 * server (FR-009b); anything left out is named (FR-009c).
 */
export async function downloadShelfCollection(
  id: string,
): Promise<ShelfCollectionDownload> {
  const data = await postGraphQL<{
    downloadShelfCollection: ShelfCollectionDownload;
  }>(
    `query DownloadShelfCollection($id: UUID!) {
       downloadShelfCollection(id: $id) {
         fileName
         contents
         entryCount
         excluded { kind name reason }
       }
     }`,
    { id },
  );
  return data.downloadShelfCollection;
}

/** Hand the browser a file to save. */
export function saveFile(fileName: string, contents: string): void {
  const url = URL.createObjectURL(
    new Blob([contents], { type: "application/json" }),
  );
  const link = document.createElement("a");
  link.href = url;
  link.download = fileName;
  document.body.appendChild(link);
  link.click();
  link.remove();
  // After the click has been handled, not before: revoking at once can cancel
  // the save in some browsers.
  setTimeout(() => URL.revokeObjectURL(url), 0);
}
