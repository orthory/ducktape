const API_BASE = ''

export interface DocumentEntry {
  type: 'file' | 'directory'
  name: string
  path: string
  children?: DocumentEntry[]
}

export interface Document {
  frontmatter?: {
    title: string
    author: string
    created_at: number
    updated_at: number
  }
  body: string
  comments: Comment[]
  tasks: Task[]
}

export interface Comment {
  index: number
  author: string
  parent_id: number
  timestamp: number
  body: string[]
}

export interface Task {
  index: number
  title: string
  author: string
  body: string[]
  assignees: string[]
  status: string
  start_at: number
  end_at: number
}

export async function fetchDocumentTree(path = ''): Promise<DocumentEntry[]> {
  const res = await fetch(`${API_BASE}/documents/${path}`)
  if (!res.ok) throw new Error(`Failed to fetch: ${res.statusText}`)
  const data = await res.json()

  // Handle directory response which is a map of name -> entry
  if (typeof data === 'object' && !Array.isArray(data)) {
    return Object.entries(data).map(([name, entry]) => ({
      name,
      path: path ? `${path}/${name}` : name,
      type: typeof entry === 'object' && entry !== null && 'children' in entry
        ? 'directory'
        : 'file',
      ...(entry as object),
    }))
  }
  return data
}

export async function fetchDocument(path: string): Promise<string> {
  const res = await fetch(`${API_BASE}/documents/${path}?view=full`)
  if (!res.ok) throw new Error(`Failed to fetch: ${res.statusText}`)
  return res.json()
}

export async function updateDocument(
  path: string,
  content: string,
  title?: string
): Promise<void> {
  const res = await fetch(`${API_BASE}/documents/${path}`, {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ content, title }),
  })
  if (!res.ok) throw new Error(`Failed to update: ${res.statusText}`)
}

export async function createDocument(
  path: string,
  title: string,
  author: string,
  content = ''
): Promise<void> {
  const res = await fetch(`${API_BASE}/documents/${path}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ title, author, content }),
  })
  if (!res.ok) throw new Error(`Failed to create: ${res.statusText}`)
}

export async function deleteDocument(path: string): Promise<void> {
  const res = await fetch(`${API_BASE}/documents/${path}`, {
    method: 'DELETE',
  })
  if (!res.ok) throw new Error(`Failed to delete: ${res.statusText}`)
}
