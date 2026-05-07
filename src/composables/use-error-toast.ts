import { useMessage } from 'naive-ui'

import { describeError } from '../api/svn'

export function useErrorToast() {
  const message = useMessage()
  return (err: unknown, fallback = '操作失败') => {
    const text = describeError(err) || fallback
    message.error(text, { duration: 6000, closable: true })
    console.error('[svn-client]', err)
  }
}
