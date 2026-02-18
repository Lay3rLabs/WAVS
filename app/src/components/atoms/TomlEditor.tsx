import { useEffect, useRef } from 'react';
import { EditorState } from '@codemirror/state';
import { EditorView, keymap } from '@codemirror/view';
import { defaultKeymap, history, historyKeymap } from '@codemirror/commands';
import { StreamLanguage } from '@codemirror/language';
import { toml } from '@codemirror/legacy-modes/mode/toml';
import { oneDark } from '@codemirror/theme-one-dark';
import { basicSetup } from 'codemirror';
import { searchKeymap } from '@codemirror/search';

const wavsTheme = EditorView.theme(
  {
    '&': {
      backgroundColor: '#1E1C1B',
      borderRadius: '0.5rem',
      border: '1px solid #37332E',
    },
    '.cm-content': {
      caretColor: '#E8DDD0',
      color: '#E8DDD0',
      fontFamily: 'ui-monospace, SFMono-Regular, "SF Mono", Menlo, monospace',
      fontSize: '13px',
    },
    '.cm-cursor': {
      borderLeftColor: '#E8DDD0',
    },
    '&.cm-focused .cm-selectionBackground, .cm-selectionBackground': {
      backgroundColor: '#37332E !important',
    },
    '.cm-gutters': {
      backgroundColor: '#151413',
      color: '#C5B5A3',
      border: 'none',
      borderRight: '1px solid #37332E',
    },
    '.cm-activeLineGutter': {
      backgroundColor: '#262423',
    },
    '.cm-activeLine': {
      backgroundColor: '#262423',
    },
  },
  { dark: true },
);

interface TomlEditorProps {
  value: string;
  onChange?: (value: string) => void;
  height?: string;
}

export function TomlEditor({ value, onChange, height = '400px' }: TomlEditorProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);
  const isExternalUpdate = useRef(false);

  // Create editor on mount
  useEffect(() => {
    if (!containerRef.current) return;

    const updateListener = EditorView.updateListener.of((update) => {
      if (update.docChanged && !isExternalUpdate.current) {
        onChange?.(update.state.doc.toString());
      }
    });

    const state = EditorState.create({
      doc: value,
      extensions: [
        basicSetup,
        keymap.of([...defaultKeymap, ...historyKeymap, ...searchKeymap]),
        history(),
        StreamLanguage.define(toml),
        oneDark,
        wavsTheme,
        EditorView.theme({
          '&': { height },
          '.cm-scroller': { overflow: 'auto' },
        }),
        updateListener,
      ],
    });

    const view = new EditorView({
      state,
      parent: containerRef.current,
    });

    viewRef.current = view;

    return () => {
      view.destroy();
      viewRef.current = null;
    };
    // Only run on mount/unmount
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Sync external value changes into editor
  useEffect(() => {
    const view = viewRef.current;
    if (!view) return;

    const currentContent = view.state.doc.toString();
    if (currentContent !== value) {
      isExternalUpdate.current = true;
      view.dispatch({
        changes: {
          from: 0,
          to: currentContent.length,
          insert: value,
        },
      });
      isExternalUpdate.current = false;
    }
  }, [value]);

  return <div ref={containerRef} />;
}
