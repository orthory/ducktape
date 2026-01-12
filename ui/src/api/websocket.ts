import { useEffect, useRef, useCallback } from 'react'

export type BroadcastEvent =
  | { type: 'document_changed'; path: string }
  | { type: 'document_created'; path: string }
  | { type: 'document_deleted'; path: string }

type EventHandler = (event: BroadcastEvent) => void

export function useWebSocket(onEvent: EventHandler) {
  const wsRef = useRef<WebSocket | null>(null)
  const reconnectTimeoutRef = useRef<number | null>(null)

  const connect = useCallback(() => {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
    const wsUrl = `${protocol}//${window.location.host}/ws`

    const ws = new WebSocket(wsUrl)
    wsRef.current = ws

    ws.onopen = () => {
      console.log('[ws] connected')
    }

    ws.onmessage = (e) => {
      try {
        const event = JSON.parse(e.data) as BroadcastEvent
        onEvent(event)
      } catch (err) {
        console.error('[ws] failed to parse message:', err)
      }
    }

    ws.onclose = () => {
      console.log('[ws] disconnected, reconnecting...')
      reconnectTimeoutRef.current = window.setTimeout(connect, 2000)
    }

    ws.onerror = (err) => {
      console.error('[ws] error:', err)
      ws.close()
    }
  }, [onEvent])

  useEffect(() => {
    connect()

    return () => {
      if (reconnectTimeoutRef.current) {
        clearTimeout(reconnectTimeoutRef.current)
      }
      wsRef.current?.close()
    }
  }, [connect])
}
