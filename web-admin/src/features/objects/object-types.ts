/**
 * Object domain types used by the Web Admin.
 * Web Admin 使用的对象域类型。
 */

export type ObjectState = 'ready' | 'deleting'

export type ObjectSummary = {
  /** Logical object key / 逻辑对象键 */
  key: string
  /** Optional display name from the current backend adapter / 当前后端适配层提供的可选展示名 */
  name?: string
  /** Object size in bytes / 对象大小（字节） */
  size: number
  /** Object checksum when available / 对象校验和（如可用） */
  checksum?: string
  /** Reference count when available / 引用计数（如可用） */
  refCount?: number
  /** Whether the object is pinned / 对象是否被固定 */
  pinned?: boolean
  /** Object lifecycle state / 对象生命周期状态 */
  state: ObjectState
  /** Updated timestamp in epoch seconds / 更新时间戳（秒） */
  updatedAt?: number
  /** Free-form metadata / 扩展元数据 */
  metadata?: Record<string, unknown>
}

export type ObjectDetail = ObjectSummary & {
  /** Created timestamp in epoch seconds / 创建时间戳（秒） */
  createdAt?: number
  /** Raw backend payload for troubleshooting / 用于排障的原始后端载荷 */
  raw?: Record<string, unknown>
}

export type ListObjectsParams = {
  /** Optional prefix or fuzzy query / 可选前缀或模糊查询 */
  prefix?: string
  /** Maximum items per page / 每页最大条数 */
  limit?: number
  /** Opaque page token / 不透明分页令牌 */
  pageToken?: string
}

export type ListObjectsResult = {
  /** Returned object summaries / 返回的对象摘要 */
  items: ObjectSummary[]
  /** Continuation token when more results exist / 存在更多结果时的续页令牌 */
  nextPageToken?: string
  /** Total items when available / 可用时的总数 */
  totalCount?: number
}
