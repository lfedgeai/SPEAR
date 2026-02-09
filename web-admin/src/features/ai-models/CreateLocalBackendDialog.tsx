import { useMemo, useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { toast } from 'sonner'

import { listNodes } from '@/api/nodes'
import { createNodeModelDeployment } from '@/api/model-deployments'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogHeader } from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'

type Provider = 'vllm' | 'llamacpp'

type FormState = {
  node_uuid: string
  provider: Provider
  model: string
  model_url: string
}

function emptyForm(): FormState {
  return {
    node_uuid: '',
    provider: 'llamacpp',
    model: '',
    model_url: '',
  }
}

export default function CreateLocalBackendDialog(props: {
  open: boolean
  onOpenChange: (open: boolean) => void
  onCreated: () => void | Promise<void>
  onCreatedDeployment?: (info: { deployment_id: string; node_uuid: string }) => void
}) {
  const nodesQuery = useQuery({
    queryKey: ['nodes', 'for-local-backend-create'],
    queryFn: () => listNodes({ limit: 500 }),
    enabled: props.open,
    refetchInterval: false,
  })

  const nodes = useMemo(() => nodesQuery.data?.nodes ?? [], [nodesQuery.data?.nodes])

  const [form, setForm] = useState<FormState>(() => emptyForm())
  const defaultNodeUuid = useMemo(() => nodes[0]?.uuid || '', [nodes])
  const handleOpenChange = (open: boolean) => {
    props.onOpenChange(open)
    if (open) {
      setForm((f) => ({
        ...emptyForm(),
        node_uuid: f.node_uuid || defaultNodeUuid,
      }))
    }
  }

  const effectiveNodeUuid = useMemo(() => {
    if (nodes.length > 0) return (form.node_uuid || defaultNodeUuid).trim()
    return form.node_uuid.trim()
  }, [defaultNodeUuid, form.node_uuid, nodes.length])

  const canSubmit = useMemo(() => {
    if (!(effectiveNodeUuid && form.provider && form.model.trim())) return false
    if (form.provider === 'llamacpp') return !!form.model_url.trim()
    return true
  }, [effectiveNodeUuid, form.model, form.model_url, form.provider])

  const disableReason = useMemo(() => {
    if (!effectiveNodeUuid) return 'Node is required'
    if (!form.model.trim()) return 'Model is required'
    if (form.provider === 'llamacpp' && !form.model_url.trim()) return 'Model URL is required'
    return ''
  }, [effectiveNodeUuid, form.model, form.model_url, form.provider])

  return (
    <Dialog open={props.open} onOpenChange={handleOpenChange}>
      <DialogContent>
        <DialogHeader
          title="Create local backend"
          description="Create a local model deployment (vLLM placeholder, LLaMA CPP supported)"
        />

        <div className="grid grid-cols-2 gap-3">
          {nodesQuery.isError ? (
            <div className="col-span-2 rounded-[var(--radius)] border border-[hsl(var(--border))] bg-[hsl(var(--secondary))] px-3 py-2 text-sm text-[hsl(var(--muted-foreground))]">
              Failed to load nodes. Check token in Settings.
            </div>
          ) : nodesQuery.isLoading ? (
            <div className="col-span-2 rounded-[var(--radius)] border border-[hsl(var(--border))] bg-[hsl(var(--secondary))] px-3 py-2 text-sm text-[hsl(var(--muted-foreground))]">
              Loading nodes…
            </div>
          ) : null}

          {nodes.length > 0 ? (
            <select
              className="col-span-2 h-9 w-full rounded-[calc(var(--radius)-4px)] border border-[hsl(var(--input))] bg-[hsl(var(--background))] px-3 text-sm"
              value={effectiveNodeUuid}
              onChange={(e) => setForm((f) => ({ ...f, node_uuid: e.target.value }))}
              aria-label="Node"
            >
              {nodes.map((n) => (
                <option key={n.uuid} value={n.uuid}>
                  {n.name ? `${n.name} (${n.uuid})` : n.uuid}
                </option>
              ))}
            </select>
          ) : (
            <div className="col-span-2">
              <Input
                placeholder="Node UUID"
                value={form.node_uuid}
                onChange={(e) => setForm((f) => ({ ...f, node_uuid: e.target.value }))}
              />
            </div>
          )}

          <select
            className="col-span-2 h-9 w-full rounded-[calc(var(--radius)-4px)] border border-[hsl(var(--input))] bg-[hsl(var(--background))] px-3 text-sm"
            value={form.provider}
            onChange={(e) =>
              setForm((f) => ({ ...f, provider: e.target.value as Provider }))
            }
            aria-label="Provider"
          >
            <option value="llamacpp">LLaMA CPP</option>
            <option value="vllm">vLLM</option>
          </select>

          <div className="col-span-2">
            <Input
              placeholder="Model name (for display)"
              value={form.model}
              onChange={(e) => setForm((f) => ({ ...f, model: e.target.value }))}
            />
          </div>

          <div className="col-span-2">
            <Input
              placeholder={
                form.provider === 'llamacpp'
                  ? 'Model URL (.gguf, required)'
                  : 'Base URL (optional, placeholder)'
              }
              value={form.model_url}
              onChange={(e) => setForm((f) => ({ ...f, model_url: e.target.value }))}
            />
          </div>

          <div className="col-span-2 flex items-center justify-end gap-2 pt-2">
            <Button variant="secondary" onClick={() => props.onOpenChange(false)}>
              Cancel
            </Button>
            {!canSubmit ? (
              <div className="mr-2 text-xs text-[hsl(var(--muted-foreground))]">
                {disableReason}
              </div>
            ) : null}
            <Button
              disabled={!canSubmit}
              onClick={async () => {
                try {
                  const params: Record<string, string> = {}
                  if (form.model_url.trim()) {
                    if (form.provider === 'llamacpp') params.model_url = form.model_url.trim()
                    else params.base_url = form.model_url.trim()
                  }

                  const resp = await createNodeModelDeployment({
                    node_uuid: effectiveNodeUuid,
                    provider: form.provider,
                    model: form.model.trim(),
                    params: Object.keys(params).length ? params : undefined,
                  })
                  if (!resp.success) {
                    toast.error(resp.message || 'Create failed')
                    return
                  }
                  toast.success('Created, provisioning…')
                  if (resp.deployment_id) {
                    props.onCreatedDeployment?.({
                      deployment_id: resp.deployment_id,
                      node_uuid: effectiveNodeUuid,
                    })
                  }
                  await props.onCreated()
                  props.onOpenChange(false)
                } catch (e) {
                  toast.error((e as Error).message)
                }
              }}
            >
              Create
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  )
}
