export type WebSocketCloseLike = {
  code: number
  reason: string
  wasClean: boolean
}

export function formatDisconnectError(event: WebSocketCloseLike): string {
  // Surface abnormal websocket termination in a user-readable way while keeping
  // clean/manual closes silent. / 将异常 WebSocket 断开转换为用户可读错误，同时保持正常/手动关闭静默。
  const codeText = `websocket disconnected (code ${event.code})`
  const reason = event.reason.trim()
  if (reason) return `${codeText}: ${reason}`
  if (!event.wasClean) return `${codeText}: connection closed unexpectedly`
  if (event.code !== 1000) return `${codeText}: connection closed`
  return ''
}
