import type { RefObject } from 'react'
import { type ChatMessage } from '../models/conversation'
import { Button } from '../../../web-admin/src/shared/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '../../../web-admin/src/shared/components/ui/card'
import { ChatBubble } from './ChatBubble'

type ChatViewProps = {
  hasActiveChat: boolean
  messages: ChatMessage[]
  endRef: RefObject<HTMLDivElement | null>
  onNewChat: () => void
  onNewChatAndConnect: () => void
}

export function ChatView({
  hasActiveChat,
  messages,
  endRef,
  onNewChat,
  onNewChatAndConnect,
}: ChatViewProps) {
  return (
    <div className="cw-chat">
      {messages.length ? (
        messages.map((message) => <ChatBubble key={message.id} message={message} />)
      ) : !hasActiveChat ? (
        <Card className="cw-empty">
          <CardHeader className="pb-2">
            <div className="cw-emptyEyebrow">Start Here</div>
            <CardTitle className="cw-emptyTitle">No chat selected</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="cw-emptyText">
              Create a chat first, then connect it to an execution or endpoint when needed.
            </div>
            <div className="cw-emptyActions">
              <Button variant="secondary" size="sm" onClick={onNewChat}>
                <span className="cw-btnGlyph">+</span>
                <span className="cw-btnText">New chat</span>
              </Button>
              <Button size="sm" onClick={onNewChatAndConnect}>
                <span className="cw-btnGlyph">↗</span>
                <span className="cw-btnText">New chat and connect</span>
              </Button>
            </div>
          </CardContent>
        </Card>
      ) : (
        <Card className="cw-empty">
          <CardHeader className="pb-2">
            <div className="cw-emptyEyebrow">Next Step</div>
            <CardTitle className="cw-emptyTitle">Connect and start chatting</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="cw-emptyText">
              Connect by execution (stream session) or by endpoint gateway. The chat window stays the same.
            </div>
          </CardContent>
        </Card>
      )}
      <div ref={endRef} />
    </div>
  )
}
