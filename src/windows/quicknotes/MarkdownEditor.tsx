import { useEffect, useImperativeHandle, useMemo, useRef, type Ref } from "react";
import { defaultKeymap, history, historyKeymap, indentLess, indentMore } from "@codemirror/commands";
import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { HighlightStyle, indentUnit, syntaxHighlighting, syntaxTree } from "@codemirror/language";
import { Annotation, EditorState, RangeSetBuilder, type Extension } from "@codemirror/state";
import { Decoration, EditorView, keymap, placeholder, ViewPlugin, type Command, type DecorationSet, type ViewUpdate } from "@codemirror/view";
import { tags } from "@lezer/highlight";
import { toggleTask, toggleWrap, type Edit } from "./markdown";

/** The syntax stays visible and dimmed; headings, emphasis, code, quotes and links are styled. */
const highlight = HighlightStyle.define([
  { tag: tags.heading, color: "var(--style-yellow)", fontWeight: "700" },
  { tag: tags.strong, color: "var(--style-purple)", fontWeight: "700" },
  { tag: tags.emphasis, color: "var(--a-teal)", fontStyle: "italic" },
  { tag: tags.strikethrough, textDecoration: "line-through" },
  { tag: tags.monospace, backgroundColor: "var(--fill)", borderRadius: "3px" },
  { tag: tags.quote, color: "var(--o2)" },
  { tag: [tags.link, tags.url], color: "var(--accent)" },
  { tag: tags.processingInstruction, opacity: "0.5" },
]);

/** Text the page puts in (the note changed elsewhere); it isn't reported back as an edit. */
const external = Annotation.define<boolean>();

const quoteLine = Decoration.line({ class: "cm-quote-line" });

/** A bar and a tint on every line a block quote spans. */
const blockQuotes = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;
    constructor(view: EditorView) {
      this.decorations = this.build(view);
    }
    update(update: ViewUpdate) {
      if (update.docChanged || update.viewportChanged || syntaxTree(update.startState) !== syntaxTree(update.state)) {
        this.decorations = this.build(update.view);
      }
    }
    build(view: EditorView): DecorationSet {
      const builder = new RangeSetBuilder<Decoration>();
      let last = -1;
      for (const { from, to } of view.visibleRanges) {
        syntaxTree(view.state).iterate({
          from,
          to,
          enter: (node) => {
            if (node.name !== "Blockquote") return;
            for (let pos = node.from; pos <= node.to; ) {
              const line = view.state.doc.lineAt(pos);
              if (line.from > last) builder.add(line.from, line.from, quoteLine);
              last = line.from;
              pos = line.to + 1;
            }
            return false;
          },
        });
      }
      return builder.finish();
    }
  },
  { decorations: (plugin) => plugin.decorations },
);

function editCommand(compute: (text: string, from: number, to: number) => Edit): Command {
  return (view) => {
    const { from, to } = view.state.selection.main;
    const edit = compute(view.state.doc.toString(), from, to);
    view.dispatch({
      changes: { from: edit.from, to: edit.to, insert: edit.text },
      selection: { anchor: edit.select[0], head: edit.select[1] },
      scrollIntoView: true,
      userEvent: "input.format",
    });
    return true;
  };
}

const formatKeymap = keymap.of([
  { key: "Mod-b", run: editCommand((text, from, to) => toggleWrap(text, from, to, "**")) },
  { key: "Mod-i", run: editCommand((text, from, to) => toggleWrap(text, from, to, "*")) },
  { key: "Mod-l", run: editCommand(toggleTask) },
  { key: "Tab", run: indentMore, shift: indentLess },
]);

const BASE_EXTENSIONS: Extension[] = [
  history(),
  formatKeymap,
  keymap.of([...defaultKeymap, ...historyKeymap]),
  markdown({ base: markdownLanguage }),
  syntaxHighlighting(highlight),
  blockQuotes,
  indentUnit.of("    "),
  EditorState.tabSize.of(4),
  EditorView.lineWrapping,
  EditorView.contentAttributes.of({ "aria-label": "Note", spellcheck: "true" }),
  placeholder("Write in Markdown…"),
];

export interface MarkdownEditorHandle {
  focus: () => void;
  /** Re-reads the text's size after a CSS change, such as the presentation font size. */
  requestMeasure: () => void;
}

/**
 * CodeMirror 6 with Markdown, as in Obsidian's source mode. Edits go to `onChange`; another `value` from
 * outside replaces the text; a new `documentKey` starts a fresh editor and undo history.
 */
export function MarkdownEditor({ value, documentKey, onChange, ref }: { value: string; documentKey: string; onChange: (value: string) => void; ref?: Ref<MarkdownEditorHandle> }) {
  const parent = useRef<HTMLDivElement>(null);
  const view = useRef<EditorView | null>(null);
  const latest = useRef({ value, onChange });
  useEffect(() => {
    latest.current = { value, onChange };
  });

  const extensions = useMemo<Extension[]>(
    () => [
      ...BASE_EXTENSIONS,
      EditorView.updateListener.of((update) => {
        if (update.docChanged && !update.transactions.some((tr) => tr.annotation(external))) {
          latest.current.onChange(update.state.doc.toString());
        }
      }),
    ],
    [],
  );

  useEffect(() => {
    if (!parent.current) return;
    const editor = new EditorView({ parent: parent.current, state: EditorState.create({ doc: latest.current.value, extensions }) });
    view.current = editor;
    editor.focus();
    return () => {
      editor.destroy();
      view.current = null;
    };
  }, [documentKey, extensions]);

  useEffect(() => {
    const editor = view.current;
    if (!editor || editor.state.doc.toString() === value) return;
    editor.dispatch({ changes: { from: 0, to: editor.state.doc.length, insert: value }, annotations: external.of(true) });
  }, [value]);

  useImperativeHandle(ref, () => ({ focus: () => view.current?.focus(), requestMeasure: () => view.current?.requestMeasure() }), []);

  return <div ref={parent} className="quick-note-editor" data-stage="quick-notes:editor" data-stage-label="Quick notes editor" />;
}
