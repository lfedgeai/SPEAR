{{/*
Helm helpers / Helm 辅助模板
*/}}

{{- define "spear.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{- define "spear.fullname" -}}
{{- if .Values.fullnameOverride -}}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" -}}
{{- else -}}
{{- $name := default .Chart.Name .Values.nameOverride -}}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" -}}
{{- end -}}
{{- end -}}

{{- define "spear.labels" -}}
app.kubernetes.io/name: {{ include "spear.name" . }}
helm.sh/chart: {{ printf "%s-%s" .Chart.Name .Chart.Version | quote }}
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end -}}

{{- define "spear.selectorLabels" -}}
app.kubernetes.io/name: {{ include "spear.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end -}}

{{- define "spear.sms.fullname" -}}
{{- printf "%s-sms" (include "spear.fullname" .) | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{- define "spear.spearlet.fullname" -}}
{{- printf "%s-spearlet" (include "spear.fullname" .) | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{- define "spear.serviceAccountName" -}}
{{- if .Values.spearlet.serviceAccount.create -}}
{{- if .Values.spearlet.serviceAccount.name -}}
{{- .Values.spearlet.serviceAccount.name -}}
{{- else -}}
{{- include "spear.spearlet.fullname" . -}}
{{- end -}}
{{- else -}}
{{- default "default" .Values.spearlet.serviceAccount.name -}}
{{- end -}}
{{- end -}}

{{- define "spear.image.repository" -}}
{{- $root := index . 0 -}}
{{- $component := index . 1 -}}
{{- default $root.Values.global.image.repository $component.image.repository -}}
{{- end -}}

{{- define "spear.image.tag" -}}
{{- $root := index . 0 -}}
{{- $component := index . 1 -}}
{{- default $root.Values.global.image.tag $component.image.tag -}}
{{- end -}}

{{- define "spear.image.pullPolicy" -}}
{{- $root := index . 0 -}}
{{- $component := index . 1 -}}
{{- default $root.Values.global.image.pullPolicy $component.image.pullPolicy -}}
{{- end -}}
