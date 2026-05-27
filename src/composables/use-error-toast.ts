import { describeError } from '../api/svn'
import { useAppToast } from './use-app-toast'

export function useErrorToast() {
  const toast = useAppToast()
  return (err: unknown, fallback = '操作失败') => {
    const text = describeError(err) || fallback
    toast.error(fallback, text)
    console.error('[svn-client]', err)
  }
}
