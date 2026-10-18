/**
 * Editor dialog for unified AI backends.
 * 统一 AI backend 的编辑对话框。
 */

import { useMemo, useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { toast } from 'sonner'

import type { AiBackendSummary, AiBackendDesiredState, AiBackendHosting, WriteAiBackendInput } from '@/api/ai-backends'
import { listNodes } from '@/api/nodes'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogHeader } from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import {
  backendKindOptionsFor,
  buildPayload,
  buildPlacementPolicy,
  defaultBackendKindFor,
  defaultPlacementScopeForHosting,
  defaultProviderForHosting,
  defaultsForKind,
  FEATURE_OPTIONS,
  formFromBackend,
  type AiBackendEditorFormState as FormState,
  OPERATION_OPTIONS,
  type PlacementPolicyInput,
  providerOptionsForHosting,
  splitCsv,
  toggleCsvValue,
  TRANSPORT_OPTIONS,
  emptyForm,
  validateForm,
} from './AiBackendEditorForm'
import LocalLlamaCppFields from './LocalLlamaCppFields'
import PlacementSection from './PlacementSection'
import RemoteBackendFields from './RemoteBackendFields'

function deriveInitialFormState(props: {
  backend?: AiBackendSummary | null
  initialHosting?: AiBackendHosting
}): FormState {
  return props.backend
    ? formFromBackend(props.backend)
    : emptyForm(props.initialHosting ?? 'remote')
}

function applySingleNodePlacementDefault(
  form: FormState,
  nodes: Array<{ uuid: string }>,
  isEditing: boolean,
): FormState {
  if (isEditing) return form
  if (form.placement_scope !== 'single_node') return form
  if (form.placement_node_uuid.trim()) return form
  if (nodes.length !== 1) return form
  return { ...form, placement_node_uuid: nodes[0].uuid }
}

export default function AiBackendEditorDialog(props: {
  open: boolean
  backend?: AiBackendSummary | null
  initialHosting?: AiBackendHosting
  title?: string
  description?: string
  submitLabel?: string
  onOpenChange: (open: boolean) => void
  onSubmit: (input: WriteAiBackendInput, placementPolicy?: PlacementPolicyInput) => Promise<void>
}) {
  // Remount the dialog body on open/input identity changes so form state resets
  // from props without a sync effect. / 当打开状态或输入对象变化时重建对话框主体，
  // 让表单直接从 props 重新初始化，而不是依赖同步 effect。
  const dialogKey = `${props.open ? 'open' : 'closed'}:${props.backend?.backend_id || 'create'}:${props.initialHosting || 'auto'}`

  return (
    <Dialog open={props.open} onOpenChange={props.onOpenChange}>
      {props.open ? <AiBackendEditorDialogBody key={dialogKey} {...props} /> : null}
    </Dialog>
  )
}

function AiBackendEditorDialogBody(props: {
  open: boolean
  backend?: AiBackendSummary | null
  initialHosting?: AiBackendHosting
  title?: string
  description?: string
  submitLabel?: string
  onOpenChange: (open: boolean) => void
  onSubmit: (input: WriteAiBackendInput, placementPolicy?: PlacementPolicyInput) => Promise<void>
}) {
  const [form, setForm] = useState<FormState>(() => deriveInitialFormState(props))
  const isEditing = !!props.backend?.backend_id
  const forcedHosting = !isEditing ? props.initialHosting : undefined
  const nodesQuery = useQuery({
    queryKey: ['nodes-for-placement'],
    queryFn: () => listNodes({ sort_by: 'last_heartbeat', order: 'desc', limit: 200 }),
    enabled: !isEditing,
    staleTime: 15_000,
  })
  const nodes = useMemo(
    () => nodesQuery.data?.nodes ?? [],
    [nodesQuery.data?.nodes],
  )
  const effectiveForm = useMemo(
    () => applySingleNodePlacementDefault(form, nodes, isEditing),
    [form, isEditing, nodes],
  )

  const validationErrors = useMemo(
    () => validateForm(effectiveForm, { isEditing, nodes }),
    [effectiveForm, isEditing, nodes],
  )
  const canSubmit = validationErrors.length === 0
  const disableReason = validationErrors[0] || ''
  const selectedOperations = useMemo(
    () => splitCsv(effectiveForm.operations),
    [effectiveForm.operations],
  )
  const selectedFeatures = useMemo(
    () => splitCsv(effectiveForm.features),
    [effectiveForm.features],
  )
  const selectedPlacementNodes = useMemo(
    () => splitCsv(effectiveForm.placement_node_uuids),
    [effectiveForm.placement_node_uuids],
  )
  const providerOptions = useMemo(
    () => providerOptionsForHosting(effectiveForm.hosting, effectiveForm.provider),
    [effectiveForm.hosting, effectiveForm.provider],
  )
  const backendKindOptions = useMemo(
    () =>
      backendKindOptionsFor(
        effectiveForm.hosting,
        effectiveForm.provider,
        effectiveForm.backend_kind,
      ),
    [effectiveForm.backend_kind, effectiveForm.hosting, effectiveForm.provider],
  )

  return (
      <DialogContent>
        <DialogHeader
          title={props.title ?? (isEditing ? 'Edit AI backend' : 'Create AI backend')}
          description={
            props.description ?? 'One row maps to one canonical backend_id in the new control plane.'
          }
        />

        <div className="min-h-0 flex-1 overflow-y-auto pr-1">
          <div className="space-y-3">
          <div className="grid grid-cols-2 gap-3">
            <div className="space-y-1">
              <div className="text-sm font-medium">Display name</div>
              <Input
                value={effectiveForm.display_name}
                onChange={(event) => setForm((current) => ({ ...current, display_name: event.target.value }))}
                placeholder="e.g. OpenAI Production"
              />
            </div>
            {forcedHosting ? (
              <div className="space-y-1">
                <div className="text-sm font-medium">Hosting</div>
                <div className="flex h-9 items-center rounded-[calc(var(--radius)-4px)] border border-[hsl(var(--input))] bg-[hsl(var(--muted))] px-3 text-sm">
                  {forcedHosting}
                </div>
              </div>
            ) : (
              <div className="space-y-1">
                <div className="text-sm font-medium">Hosting</div>
                <select
                  className="h-9 w-full rounded-[calc(var(--radius)-4px)] border border-[hsl(var(--input))] bg-[hsl(var(--background))] px-3 text-sm"
                  value={effectiveForm.hosting}
                  onChange={(event) =>
                    setForm((current) => {
                      const hosting = event.target.value as AiBackendHosting
                      const nextOptions = providerOptionsForHosting(hosting)
                      const nextProvider = nextOptions.includes(current.provider)
                        ? current.provider
                        : defaultProviderForHosting(hosting)
                      const nextKindOptions = backendKindOptionsFor(hosting, nextProvider)
                      const nextKind = nextKindOptions.includes(current.backend_kind)
                        ? current.backend_kind
                        : defaultBackendKindFor(hosting, nextProvider)
                      const nextDefaults = defaultsForKind(nextKind)
                      return {
                        ...current,
                        hosting,
                        provider: nextProvider,
                        model: current.model.trim() ? current.model : nextDefaults.model,
                        backend_kind: nextKind,
                        operations: nextDefaults.operations.join(', '),
                        transports: nextDefaults.transports,
                        features: nextDefaults.features,
                        base_url: current.base_url.trim() ? current.base_url : nextDefaults.base_url,
                        placement_scope: defaultPlacementScopeForHosting(hosting),
                        placement_node_uuid:
                          hosting === 'local' && nodes.length === 1 ? nodes[0].uuid : '',
                        placement_node_uuids: '',
                        model_url: '',
                      }
                    })
                  }
                >
                  <option value="remote">remote</option>
                  <option value="local">local</option>
                </select>
              </div>
            )}
          </div>

          <div className="grid grid-cols-2 gap-3">
            <div className="space-y-1">
              <div className="text-sm font-medium">Provider</div>
              <select
                className="h-9 w-full rounded-[calc(var(--radius)-4px)] border border-[hsl(var(--input))] bg-[hsl(var(--background))] px-3 text-sm"
                value={effectiveForm.provider}
                onChange={(event) =>
                  setForm((current) => {
                    const provider = event.target.value
                    const backendKind = defaultBackendKindFor(current.hosting, provider)
                    const nextDefaults = defaultsForKind(backendKind)
                    return {
                      ...current,
                      provider,
                      model: current.model.trim() ? current.model : nextDefaults.model,
                      backend_kind: backendKind,
                      operations: nextDefaults.operations.join(', '),
                      transports: nextDefaults.transports,
                      features: nextDefaults.features,
                      base_url: current.base_url.trim() ? current.base_url : nextDefaults.base_url,
                    }
                  })
                }
              >
                {providerOptions.map((provider) => (
                  <option key={provider} value={provider}>
                    {provider}
                  </option>
                ))}
              </select>
            </div>
            <div className="space-y-1">
              <div className="text-sm font-medium">Model</div>
              <Input
                value={effectiveForm.model}
                onChange={(event) => setForm((current) => ({ ...current, model: event.target.value }))}
                placeholder="e.g. gpt-4.1"
              />
            </div>
          </div>

          <div className="grid grid-cols-2 gap-3">
            <div className="space-y-1">
              <div className="text-sm font-medium">Backend kind</div>
              <select
                className="h-9 w-full rounded-[calc(var(--radius)-4px)] border border-[hsl(var(--input))] bg-[hsl(var(--background))] px-3 text-sm"
                value={effectiveForm.backend_kind}
                onChange={(event) =>
                  setForm((current) => {
                    const backendKind = event.target.value
                    const nextDefaults = defaultsForKind(backendKind)
                    return {
                      ...current,
                      model: current.model.trim() ? current.model : nextDefaults.model,
                      backend_kind: backendKind,
                      operations: nextDefaults.operations.join(', '),
                      transports: nextDefaults.transports,
                      features: nextDefaults.features,
                      base_url: current.base_url.trim() ? current.base_url : nextDefaults.base_url,
                    }
                  })
                }
              >
                {backendKindOptions.map((backendKind) => (
                  <option key={backendKind} value={backendKind}>
                    {backendKind}
                  </option>
                ))}
              </select>
            </div>
            <div className="space-y-1">
              <div className="text-sm font-medium">Desired state</div>
              <select
                className="h-9 w-full rounded-[calc(var(--radius)-4px)] border border-[hsl(var(--input))] bg-[hsl(var(--background))] px-3 text-sm"
                value={effectiveForm.desired_state}
                onChange={(event) =>
                  setForm((current) => ({
                    ...current,
                    desired_state: event.target.value as AiBackendDesiredState,
                  }))
                }
              >
                <option value="enabled">enabled</option>
                <option value="disabled">disabled</option>
              </select>
            </div>
          </div>

          {effectiveForm.hosting === 'remote' ? (
            <RemoteBackendFields form={effectiveForm} setForm={setForm} />
          ) : null}

          {effectiveForm.hosting === 'local' && effectiveForm.provider === 'llamacpp' ? (
            <LocalLlamaCppFields form={effectiveForm} setForm={setForm} />
          ) : null}

          <div className="space-y-1">
            <div className="text-sm font-medium">Operations</div>
            <div className="grid grid-cols-1 gap-2 rounded-[calc(var(--radius)-4px)] border border-[hsl(var(--input))] px-3 py-2 md:grid-cols-2">
              {OPERATION_OPTIONS.map((operation) => (
                <label key={operation} className="flex items-center gap-2 text-sm">
                  <input
                    type="checkbox"
                    checked={selectedOperations.includes(operation)}
                    onChange={(event) =>
                      setForm((current) => ({
                        ...current,
                        operations: toggleCsvValue(
                          current.operations,
                          operation,
                          event.target.checked,
                        ),
                      }))
                    }
                  />
                  <span>{operation}</span>
                </label>
              ))}
            </div>
          </div>

          <div className="grid grid-cols-2 gap-3">
            <div className="space-y-1">
              <div className="text-sm font-medium">Features</div>
              <div className="grid grid-cols-1 gap-2 rounded-[calc(var(--radius)-4px)] border border-[hsl(var(--input))] px-3 py-2">
                {FEATURE_OPTIONS.map((feature) => (
                  <label key={feature} className="flex items-center gap-2 text-sm">
                    <input
                      type="checkbox"
                      checked={selectedFeatures.includes(feature)}
                      onChange={(event) =>
                        setForm((current) => ({
                          ...current,
                          features: toggleCsvValue(
                            current.features,
                            feature,
                            event.target.checked,
                          ),
                        }))
                      }
                    />
                    <span>{feature}</span>
                  </label>
                ))}
              </div>
            </div>
            <div className="space-y-1">
              <div className="text-sm font-medium">Transports</div>
              <select
                className="h-9 w-full rounded-[calc(var(--radius)-4px)] border border-[hsl(var(--input))] bg-[hsl(var(--background))] px-3 text-sm"
                value={effectiveForm.transports}
                onChange={(event) =>
                  setForm((current) => ({ ...current, transports: event.target.value }))
                }
              >
                {TRANSPORT_OPTIONS.map((transport) => (
                  <option key={transport} value={transport}>
                    {transport}
                  </option>
                ))}
              </select>
            </div>
          </div>

          <div className="grid grid-cols-2 gap-3">
            <div className="space-y-1">
              <div className="text-sm font-medium">Weight</div>
              <Input
                value={effectiveForm.weight}
                onChange={(event) => setForm((current) => ({ ...current, weight: event.target.value }))}
                placeholder="100"
              />
            </div>
            <div className="space-y-1">
              <div className="text-sm font-medium">Priority</div>
              <Input
                value={effectiveForm.priority}
                onChange={(event) => setForm((current) => ({ ...current, priority: event.target.value }))}
                placeholder="0"
              />
            </div>
          </div>

          <div className="grid grid-cols-2 gap-3">
            <div className="space-y-1">
              <div className="text-sm font-medium">Labels</div>
              <textarea
                className="min-h-[88px] w-full rounded-[calc(var(--radius)-4px)] border border-[hsl(var(--input))] bg-[hsl(var(--background))] px-3 py-2 text-sm"
                value={effectiveForm.labels}
                onChange={(event) =>
                  setForm((current) => ({ ...current, labels: event.target.value }))
                }
                placeholder={'env=prod\nteam=ml-platform'}
              />
              <div className="text-xs text-[hsl(var(--muted-foreground))]">
                One `key=value` pair per line.
              </div>
            </div>
            <div className="space-y-1">
              <div className="text-sm font-medium">Metadata</div>
              <textarea
                className="min-h-[88px] w-full rounded-[calc(var(--radius)-4px)] border border-[hsl(var(--input))] bg-[hsl(var(--background))] px-3 py-2 font-mono text-xs"
                value={effectiveForm.metadata}
                onChange={(event) =>
                  setForm((current) => ({
                    ...current,
                    metadata: event.target.value,
                  }))
                }
                placeholder={'{\n  "region": "us-east-1"\n}'}
              />
              <div className="text-xs text-[hsl(var(--muted-foreground))]">
                JSON object stored on the backend record.
              </div>
            </div>
          </div>

          {effectiveForm.hosting === 'local' && effectiveForm.backend_kind.toLowerCase().includes('vllm') ? (
            <div className="rounded-[var(--radius)] border border-[hsl(var(--border))] bg-[hsl(var(--muted))] px-3 py-2 text-xs text-[hsl(var(--muted-foreground))]">
              vLLM remains a scaffolded path in the current control plane. Keep it only for planned
              placeholders or external endpoint wiring.
            </div>
          ) : null}

          {!isEditing ? (
            <PlacementSection
              form={effectiveForm}
              setForm={setForm}
              nodes={nodes}
              selectedPlacementNodes={selectedPlacementNodes}
              isLoadingNodes={nodesQuery.isLoading}
            />
          ) : null}

          {validationErrors.length > 0 ? (
            <div className="rounded-[var(--radius)] border border-[hsl(var(--destructive))] bg-[hsl(var(--destructive))/0.08] px-3 py-2 text-xs text-[hsl(var(--destructive))]">
              {validationErrors.join(' ')}
            </div>
          ) : null}

        </div>
        </div>
        <div className="mt-4 flex shrink-0 items-center justify-end gap-2 border-t border-[hsl(var(--border))] pt-3">
          <Button variant="secondary" onClick={() => props.onOpenChange(false)}>
            Cancel
          </Button>
          <Button
            disabled={!canSubmit}
            title={canSubmit ? '' : disableReason}
            onClick={async () => {
              try {
                await props.onSubmit(
                  buildPayload(effectiveForm),
                  !isEditing ? buildPlacementPolicy(effectiveForm, nodes) : undefined,
                )
                props.onOpenChange(false)
              } catch (error) {
                toast.error((error as Error).message)
              }
            }}
          >
            {props.submitLabel ?? 'Save'}
          </Button>
        </div>
      </DialogContent>
  )
}
