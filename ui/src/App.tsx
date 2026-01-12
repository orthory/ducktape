import { useState, useCallback } from 'react'
import { FileNav } from './components/FileNav'
import { Editor } from './components/Editor'
import { useWebSocket, BroadcastEvent } from './api/websocket'

export default function App() {
  const [selectedPath, setSelectedPath] = useState<string | null>(null)
  const [refreshKey, setRefreshKey] = useState(0)

  const handleWebSocketEvent = useCallback((event: BroadcastEvent) => {
    console.log('[ws] event:', event)
    // Refresh on any document change
    setRefreshKey((k) => k + 1)
  }, [])

  useWebSocket(handleWebSocketEvent)

  return (
    <div style={{ display: 'flex', height: '100vh', fontFamily: 'system-ui, sans-serif' }}>
      <div
        style={{
          width: 250,
          borderRight: '1px solid #ccc',
          overflow: 'auto',
          flexShrink: 0,
        }}
      >
        <FileNav
          onSelect={setSelectedPath}
          selectedPath={selectedPath}
          refreshKey={refreshKey}
        />
      </div>

      <div style={{ flex: 1, overflow: 'auto' }}>
        {selectedPath ? (
          <Editor path={selectedPath} refreshKey={refreshKey} />
        ) : (
          <div
            style={{
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              height: '100%',
              color: '#666',
            }}
          >
            Select a document from the sidebar
          </div>
        )}
      </div>
    </div>
  )
}
