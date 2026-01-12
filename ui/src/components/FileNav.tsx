import { useState, useEffect, useCallback } from 'react'
import { fetchDocumentTree, DocumentEntry } from '../api/client'

interface FileNavProps {
  onSelect: (path: string) => void
  selectedPath: string | null
  refreshKey: number
}

export function FileNav({ onSelect, selectedPath, refreshKey }: FileNavProps) {
  const [entries, setEntries] = useState<DocumentEntry[]>([])
  const [expanded, setExpanded] = useState<Set<string>>(new Set())
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  const loadEntries = useCallback(async () => {
    try {
      setLoading(true)
      setError(null)
      const tree = await fetchDocumentTree()
      setEntries(tree)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load')
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    loadEntries()
  }, [loadEntries, refreshKey])

  const toggleExpand = (path: string) => {
    setExpanded((prev) => {
      const next = new Set(prev)
      if (next.has(path)) {
        next.delete(path)
      } else {
        next.add(path)
      }
      return next
    })
  }

  const renderEntry = (entry: DocumentEntry, depth = 0) => {
    const isExpanded = expanded.has(entry.path)
    const isSelected = selectedPath === entry.path
    const paddingLeft = 8 + depth * 16

    if (entry.type === 'directory') {
      return (
        <div key={entry.path}>
          <div
            onClick={() => toggleExpand(entry.path)}
            style={{
              padding: '4px 8px',
              paddingLeft,
              cursor: 'pointer',
              fontWeight: 'bold',
            }}
          >
            {isExpanded ? '[-]' : '[+]'} {entry.name}/
          </div>
          {isExpanded && entry.children?.map((child) => renderEntry(child, depth + 1))}
        </div>
      )
    }

    return (
      <div
        key={entry.path}
        onClick={() => onSelect(entry.path)}
        style={{
          padding: '4px 8px',
          paddingLeft,
          cursor: 'pointer',
          background: isSelected ? '#e0e0e0' : 'transparent',
        }}
      >
        {entry.name}
      </div>
    )
  }

  if (loading) return <div style={{ padding: 8 }}>Loading...</div>
  if (error) return <div style={{ padding: 8, color: 'red' }}>{error}</div>

  return (
    <div style={{ fontFamily: 'monospace', fontSize: 14 }}>
      <div style={{ padding: 8, fontWeight: 'bold', borderBottom: '1px solid #ccc' }}>
        Files
      </div>
      {entries.length === 0 ? (
        <div style={{ padding: 8, color: '#666' }}>No documents</div>
      ) : (
        entries.map((entry) => renderEntry(entry))
      )}
    </div>
  )
}
