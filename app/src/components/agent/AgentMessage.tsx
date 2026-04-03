import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import type { AgentMessage as AgentMessageType } from '../../stores/agentStore';
import { AgentToolCall } from './AgentToolCall';

interface AgentMessageProps {
  message: AgentMessageType;
}

export function AgentMessage({ message }: AgentMessageProps) {
  switch (message.role) {
    case 'user':
      return <UserBubble content={message.content} />;
    case 'assistant':
      return <AssistantBubble content={message.content} />;
    case 'system':
      return <SystemBubble content={message.content} level={message.level} />;
    case 'toolResult':
      return null;
    default:
      return null;
  }
}

function SystemBubble({ content, level }: { content: string; level: 'info' | 'warning' | 'error' }) {
  const colorClass = level === 'error' ? 'text-red-3 border-red-2/30'
    : level === 'warning' ? 'text-tan-muted border-tan-muted/30'
    : 'text-tan-muted border-charcoal-light';

  return (
    <div className="flex justify-center mb-2">
      <div className={`text-xs px-3 py-1.5 rounded-full border ${colorClass}`}>
        {content}
      </div>
    </div>
  );
}

function UserBubble({ content }: { content: string }) {
  return (
    <div className="flex justify-end mb-3">
      <div className="max-w-[85%] px-4 py-2.5 rounded-2xl rounded-br-md bg-charcoal-medium text-beige-warm text-sm leading-relaxed">
        {content}
      </div>
    </div>
  );
}

// ── Markdown renderer ───────────────────────────────────────────────────

function Markdown({ text, className }: { text: string; className?: string }) {
  return (
    <div className={className}>
    <ReactMarkdown
      remarkPlugins={[remarkGfm]}
      components={{
        // Headings
        h1: ({ children }) => <h1 className="text-base font-bold text-cream-light mt-3 mb-1">{children}</h1>,
        h2: ({ children }) => <h2 className="text-sm font-bold text-cream-light mt-2.5 mb-1">{children}</h2>,
        h3: ({ children }) => <h3 className="text-sm font-semibold text-beige-warm mt-2 mb-0.5">{children}</h3>,
        // Paragraphs
        p: ({ children }) => <p className="text-sm text-beige-warm leading-relaxed mb-2 last:mb-0">{children}</p>,
        // Lists
        ul: ({ children }) => <ul className="text-sm text-beige-warm list-disc pl-5 mb-2 space-y-0.5">{children}</ul>,
        ol: ({ children }) => <ol className="text-sm text-beige-warm list-decimal pl-5 mb-2 space-y-0.5">{children}</ol>,
        li: ({ children }) => <li className="leading-relaxed">{children}</li>,
        // Code
        code: ({ className: codeClass, children, ...props }) => {
          const isInline = !codeClass;
          if (isInline) {
            return (
              <code className="px-1 py-0.5 rounded bg-charcoal-darkest text-purple-1 font-mono text-xs" {...props}>
                {children}
              </code>
            );
          }
          return (
            <code className={`block overflow-x-auto rounded-md bg-charcoal-darkest p-3 font-mono text-xs text-beige-warm leading-relaxed ${codeClass ?? ''}`} {...props}>
              {children}
            </code>
          );
        },
        pre: ({ children }) => <pre className="mb-2 last:mb-0">{children}</pre>,
        // Links
        a: ({ href, children }) => (
          <a href={href} target="_blank" rel="noopener noreferrer" className="text-purple-1 hover:underline">
            {children}
          </a>
        ),
        // Blockquotes
        blockquote: ({ children }) => (
          <blockquote className="border-l-2 border-charcoal-light pl-3 text-tan-muted italic mb-2">
            {children}
          </blockquote>
        ),
        // Tables
        table: ({ children }) => (
          <div className="overflow-x-auto mb-2">
            <table className="text-xs border-collapse w-full">{children}</table>
          </div>
        ),
        th: ({ children }) => <th className="border border-charcoal-light px-2 py-1 text-left text-tan-muted bg-charcoal-darkest">{children}</th>,
        td: ({ children }) => <td className="border border-charcoal-light px-2 py-1 text-beige-warm">{children}</td>,
        // Horizontal rule
        hr: () => <hr className="border-charcoal-light my-3" />,
        // Strong/em
        strong: ({ children }) => <strong className="font-semibold text-cream-light">{children}</strong>,
        em: ({ children }) => <em className="italic">{children}</em>,
      }}
    >
      {text}
    </ReactMarkdown>
    </div>
  );
}

// ── Content blocks ──────────────────────────────────────────────────────

interface ContentBlock {
  type: string;
  text?: string;
  thinking?: string;
  id?: string;
  name?: string;
  arguments?: Record<string, unknown>;
  streaming?: boolean;
}

function AssistantBubble({ content }: { content: ContentBlock[] }) {
  if (content.length === 0) return null;

  return (
    <div className="flex justify-start mb-3">
      <div className="max-w-[95%] space-y-1">
        {content.map((block, i) => {
          switch (block.type) {
            case 'text':
              return (
                <div key={i}>
                  <Markdown text={block.text ?? ''} />
                  {block.streaming && (
                    <span className="inline-block w-1.5 h-4 ml-0.5 bg-beige-warm/60 animate-pulse align-text-bottom" />
                  )}
                </div>
              );
            case 'thinking':
              return (
                <details key={i} className="group" open={block.streaming}>
                  <summary className="text-xs text-tan-muted cursor-pointer select-none hover:text-beige-warm transition-colors">
                    {block.streaming ? '💭 Thinking…' : '💭 Thought'}
                  </summary>
                  <div className="mt-1 pl-4 border-l border-charcoal-light opacity-80">
                    <Markdown text={block.thinking ?? ''} className="text-xs text-tan-muted" />
                  </div>
                </details>
              );
            case 'toolCall':
              return (
                <AgentToolCall
                  key={block.id!}
                  id={block.id!}
                  name={block.name!}
                  arguments={block.arguments ?? {}}
                  streaming={block.streaming}
                  rawArgs={(block as unknown as { _rawArgs?: string })._rawArgs}
                />
              );
            default:
              return null;
          }
        })}
      </div>
    </div>
  );
}
