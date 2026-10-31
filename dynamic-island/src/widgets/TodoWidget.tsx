import { useEffect, useMemo, useState } from "react";
import { ipc } from "../lib/ipc";
import { ActionIcon } from "../components/WidgetIcon";
import type { TodoItem } from "../types";

export function TodoWidget({ gridColumns = 2 }: { gridColumns?: 1 | 2 | 3 }) {
  const [draft, setDraft] = useState("");
  const [todos, setTodos] = useState<TodoItem[]>([]);
  const [error, setError] = useState<string | null>(null);

  const refresh = async () => {
    try {
      setTodos(await ipc.listTodos());
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  };

  useEffect(() => {
    void refresh();
  }, []);

  const doneCount = useMemo(() => todos.filter((t) => t.done).length, [todos]);

  const add = async () => {
    const label = draft.trim();
    if (!label) return;
    try {
      const item = await ipc.addTodo(label);
      setTodos((prev) => [item, ...prev]);
      setDraft("");
    } catch (e) {
      setError(String(e));
    }
  };

  const toggle = async (todo: TodoItem) => {
    const updated = await ipc.setTodoDone(todo.id, !todo.done);
    setTodos((prev) => prev.map((t) => (t.id === updated.id ? updated : t)));
  };

  const remove = async (id: string) => {
    await ipc.deleteTodo(id);
    setTodos((prev) => prev.filter((t) => t.id !== id));
  };

  return (
    <div className="widget-card flex h-full min-h-0 flex-col rounded-2xl p-4">
      <div className="mb-3 flex shrink-0 items-center justify-between text-[11px] leading-none text-[var(--text-muted)]">
        <span className="tabular-nums tracking-wide">
          <span className="font-semibold text-[var(--text-main)]">{doneCount}</span>
          <span>/{todos.length}</span> tasks done today
        </span>
        <div className="h-1 w-16 overflow-hidden rounded-full bg-white/10">
          <div
            className="h-full rounded-full bg-cyan-300/70 transition-all"
            style={{ width: todos.length ? `${(doneCount / todos.length) * 100}%` : "0%" }}
          />
        </div>
      </div>

      <div className="mb-3 flex gap-2">
        <input
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") void add();
          }}
          placeholder="Quick add task"
          className="min-w-0 flex-1 rounded-xl border border-white/10 bg-black/20 px-3 py-2 text-[12px] text-[var(--text-main)] outline-none transition placeholder:text-white/40 focus:border-cyan-300/50 focus:ring-2 focus:ring-cyan-300/10"
        />
        <button
          type="button"
          onClick={add}
          aria-label="Add task"
          title="Add task"
          className="flex shrink-0 items-center gap-1 rounded-xl bg-cyan-300/25 px-3 text-[11px] font-medium text-cyan-50 transition hover:bg-cyan-300/35 active:scale-[0.97]"
        >
          <ActionIcon name="add" className="h-3.5 w-3.5" /> Add
        </button>
      </div>

      {error && <p className="mb-2 text-xs text-rose-300">{error}</p>}

      <div className={`grid min-h-0 flex-1 content-start gap-1 overflow-y-auto ${gridColumns === 1 ? "grid-cols-1" : gridColumns === 2 ? "grid-cols-2" : "grid-cols-3"}`}>
        {todos.map((todo) => (
          <div
            key={todo.id}
            className="group flex min-w-0 items-start gap-0.5 rounded-xl border border-white/[0.06] bg-white/[0.04] py-2 pl-2.5 pr-1.5 transition hover:bg-white/[0.07]"
          >
            <label className="flex min-w-0 flex-1 cursor-pointer items-start gap-2.5">
              <input
                type="checkbox"
                checked={todo.done}
                onChange={() => void toggle(todo)}
                className="peer sr-only"
              />
              <span
                aria-hidden="true"
                className={`mt-px grid h-[15px] w-[15px] shrink-0 place-items-center rounded-[5px] border transition peer-focus-visible:ring-2 peer-focus-visible:ring-cyan-300/40 ${
                  todo.done
                    ? "border-cyan-300/80 bg-cyan-300 text-[#06222e]"
                    : "border-white/25 bg-white/[0.05] text-transparent"
                }`}
              >
                <ActionIcon name="check" className="h-[11px] w-[11px]" />
              </span>
              <span
                className={`min-w-0 flex-1 break-words text-[12.5px] leading-[1.32] ${
                  todo.done
                    ? "text-[var(--text-muted)] line-through decoration-white/25"
                    : "text-[var(--text-main)]"
                }`}
              >
                {todo.label}
              </span>
            </label>
            <button
              type="button"
              onClick={() => void remove(todo.id)}
              aria-label={`Delete ${todo.label}`}
              title="Delete task"
              className="-mt-[3px] shrink-0 rounded-md p-1 text-[var(--text-muted)] opacity-0 transition hover:bg-rose-400/10 hover:text-rose-300 group-hover:opacity-100 focus-visible:opacity-100"
            >
              <ActionIcon name="delete" className="h-3.5 w-3.5" />
            </button>
          </div>
        ))}

        {!todos.length && (
          <div className="col-span-full rounded-lg border border-white/10 p-3 text-xs text-[var(--text-muted)]">
            No tasks remaining. Add one above to start tracking progress.
          </div>
        )}
      </div>
    </div>
  );
}
