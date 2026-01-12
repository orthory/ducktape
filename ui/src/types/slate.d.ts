import { BaseEditor, Descendant } from 'slate'
import { ReactEditor } from 'slate-react'
import { HistoryEditor } from 'slate-history'

export type CustomText = {
  text: string
  bold?: boolean
  italic?: boolean
  code?: boolean
}

export type ParagraphElement = {
  type: 'paragraph'
  children: CustomText[]
}

export type HeadingElement = {
  type: 'heading'
  level: 1 | 2 | 3 | 4 | 5 | 6
  children: CustomText[]
}

export type CommentElement = {
  type: 'comment'
  author: string
  parentId: number
  timestamp: number
  children: CustomText[]
}

export type TaskElement = {
  type: 'task'
  title: string
  author: string
  assignees: string[]
  status: string
  startAt: number
  endAt: number
  children: CustomText[]
}

export type CustomElement =
  | ParagraphElement
  | HeadingElement
  | CommentElement
  | TaskElement

declare module 'slate' {
  interface CustomTypes {
    Editor: BaseEditor & ReactEditor & HistoryEditor
    Element: CustomElement
    Text: CustomText
  }
}
