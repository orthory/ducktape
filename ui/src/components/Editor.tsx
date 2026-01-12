import { useMemo, useCallback, useState, useEffect, useRef } from 'react'
import { createEditor, Descendant } from 'slate'
import { Slate, Editable, withReact, RenderElementProps, RenderLeafProps } from 'slate-react'
import { withHistory } from 'slate-history'
import { fetchDocument, updateDocument } from '../api/client'
import { CustomElement, CustomText } from '../types/slate'

interface EditorProps {
  path: string
  refreshKey: number
}

const initialValue: Descendant[] = [
  { type: 'paragraph', children: [{ text: '' }] },
]

export function Editor({ path, refreshKey }: EditorProps) {
  const editor = useMemo(() => withHistory(withReact(createEditor())), [])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const saveTimeoutRef = useRef<number | null>(null)

  // Parse raw markdown into Slate nodes (simplified - just paragraphs for now)
  const parseContent = useCallback((content: string): Descendant[] => {
    if (!content) return initialValue

    const lines = content.split('\n')
    const nodes: Descendant[] = []
    let i = 0

    while (i < lines.length) {
      const line = lines[i]

      // Check for comment block
      if (line.startsWith('/comment{')) {
        const match = line.match(/^\/comment\{([^}]*)\}/)
        if (match) {
          const args = match[1].split(';')
          const bodyLines: string[] = []
          i++
          while (i < lines.length && lines[i] !== '/comment') {
            bodyLines.push(lines[i])
            i++
          }
          nodes.push({
            type: 'comment',
            author: args[0] || '',
            parentId: parseInt(args[1]) || 0,
            timestamp: parseInt(args[2]) || 0,
            children: [{ text: bodyLines.join('\n') }],
          })
          i++ // skip closing tag
          continue
        }
      }

      // Check for task block
      if (line.startsWith('/task{')) {
        const match = line.match(/^\/task\{([^}]*)\}/)
        if (match) {
          const args = match[1].split(';')
          const bodyLines: string[] = []
          i++
          while (i < lines.length && lines[i] !== '/task') {
            bodyLines.push(lines[i])
            i++
          }
          nodes.push({
            type: 'task',
            author: args[0] || '',
            title: args[1] || '',
            status: args[2] || 'Backlog',
            startAt: parseInt(args[3]) || 0,
            endAt: parseInt(args[4]) || 0,
            assignees: args.slice(5),
            children: [{ text: bodyLines.join('\n') }],
          })
          i++ // skip closing tag
          continue
        }
      }

      // Check for heading
      const headingMatch = line.match(/^(#{1,6})\s+(.*)$/)
      if (headingMatch) {
        const level = headingMatch[1].length as 1 | 2 | 3 | 4 | 5 | 6
        nodes.push({
          type: 'heading',
          level,
          children: [{ text: headingMatch[2] }],
        })
        i++
        continue
      }

      // Regular paragraph
      nodes.push({
        type: 'paragraph',
        children: [{ text: line }],
      })
      i++
    }

    return nodes.length > 0 ? nodes : initialValue
  }, [])

  // Serialize Slate nodes back to markdown
  const serializeContent = useCallback((nodes: Descendant[]): string => {
    return nodes
      .map((node) => {
        const el = node as CustomElement
        switch (el.type) {
          case 'heading':
            return '#'.repeat(el.level) + ' ' + el.children.map(c => c.text).join('')
          case 'comment':
            return [
              `/comment{${el.author};${el.parentId};${el.timestamp}}`,
              el.children.map(c => c.text).join(''),
              '/comment',
            ].join('\n')
          case 'task':
            return [
              `/task{${el.author};${el.title};${el.status};${el.startAt};${el.endAt};${el.assignees.join(';')}}`,
              el.children.map(c => c.text).join(''),
              '/task',
            ].join('\n')
          default:
            return el.children.map(c => c.text).join('')
        }
      })
      .join('\n')
  }, [])

  // Load document
  useEffect(() => {
    const load = async () => {
      try {
        setLoading(true)
        setError(null)
        const content = await fetchDocument(path)

        // Parse and set editor content
        const parsed = parseContent(typeof content === 'string' ? content : '')
        editor.children = parsed
        editor.onChange()
      } catch (err) {
        setError(err instanceof Error ? err.message : 'Failed to load')
      } finally {
        setLoading(false)
      }
    }
    load()
  }, [path, refreshKey, editor, parseContent])

  // Auto-save with debounce
  const handleChange = useCallback(
    (value: Descendant[]) => {
      if (saveTimeoutRef.current) {
        clearTimeout(saveTimeoutRef.current)
      }

      saveTimeoutRef.current = window.setTimeout(async () => {
        try {
          const content = serializeContent(value)
          await updateDocument(path, content)
        } catch (err) {
          console.error('Failed to save:', err)
        }
      }, 1000)
    },
    [path, serializeContent]
  )

  // Cleanup timeout on unmount
  useEffect(() => {
    return () => {
      if (saveTimeoutRef.current) {
        clearTimeout(saveTimeoutRef.current)
      }
    }
  }, [])

  const renderElement = useCallback((props: RenderElementProps) => {
    const { element, attributes, children } = props
    const el = element as CustomElement

    switch (el.type) {
      case 'heading': {
        const HeadingTag = ({ level, ...props }: { level: number; children: React.ReactNode }) => {
          switch (level) {
            case 1: return <h1 {...props} />
            case 2: return <h2 {...props} />
            case 3: return <h3 {...props} />
            case 4: return <h4 {...props} />
            case 5: return <h5 {...props} />
            default: return <h6 {...props} />
          }
        }
        return <HeadingTag level={el.level} {...attributes}>{children}</HeadingTag>
      }

      case 'comment':
        return (
          <div
            {...attributes}
            style={{
              border: '1px solid #ccc',
              borderLeft: '4px solid #4a9eff',
              padding: '8px 12px',
              margin: '8px 0',
              background: '#f5f9ff',
            }}
          >
            <div style={{ fontSize: 12, color: '#666', marginBottom: 4 }}>
              Comment by {el.author}
            </div>
            {children}
          </div>
        )

      case 'task':
        return (
          <div
            {...attributes}
            style={{
              border: '1px solid #ccc',
              borderLeft: `4px solid ${el.status === 'Done' ? '#4caf50' : '#ff9800'}`,
              padding: '8px 12px',
              margin: '8px 0',
              background: '#fffef5',
            }}
          >
            <div style={{ fontSize: 12, color: '#666', marginBottom: 4 }}>
              Task: {el.title} [{el.status}]
            </div>
            {children}
          </div>
        )

      default:
        return <p {...attributes}>{children}</p>
    }
  }, [])

  const renderLeaf = useCallback((props: RenderLeafProps) => {
    const { attributes, children, leaf } = props
    let el = children
    const l = leaf as CustomText

    if (l.bold) el = <strong>{el}</strong>
    if (l.italic) el = <em>{el}</em>
    if (l.code) el = <code>{el}</code>

    return <span {...attributes}>{el}</span>
  }, [])

  if (loading) return <div style={{ padding: 16 }}>Loading...</div>
  if (error) return <div style={{ padding: 16, color: 'red' }}>{error}</div>

  return (
    <div style={{ padding: 16 }}>
      <div style={{ fontSize: 12, color: '#666', marginBottom: 8 }}>{path}</div>
      <Slate editor={editor} initialValue={initialValue} onChange={handleChange}>
        <Editable
          renderElement={renderElement}
          renderLeaf={renderLeaf}
          placeholder="Start typing..."
          style={{
            fontFamily: 'monospace',
            fontSize: 14,
            lineHeight: 1.6,
            minHeight: 400,
            outline: 'none',
          }}
        />
      </Slate>
    </div>
  )
}
