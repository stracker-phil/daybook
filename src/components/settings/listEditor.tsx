import { ItemActions, moveItem, replaceItem } from "./fields";

/** The edits of a settings list, each saving the whole list. */
export function listEditor<T>(list: T[], save: (next: T[]) => void) {
  const move = (from: number, to: number) => save(moveItem(list, from, to));
  const remove = (index: number) => save(list.filter((_, i) => i !== index));
  return {
    change: (index: number, patch: Partial<T>) => save(replaceItem(list, index, patch)),
    move,
    remove,
    add: (item: T) => save([...list, item]),
    /** The item's ↑ ↓ and, given `removeLabel`, its trash button. */
    actions: (index: number, removeLabel?: string) => (
      <ItemActions index={index} count={list.length} onMove={move} onRemove={removeLabel ? () => remove(index) : undefined} removeLabel={removeLabel} />
    ),
  };
}
