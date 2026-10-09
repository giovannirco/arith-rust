{{/*
Chart name, allowing an override.
*/}}
{{- define "arith.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Resource name. Release name when it contains the chart name, both otherwise.
*/}}
{{- define "arith.fullname" -}}
{{- if .Values.fullnameOverride }}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- $name := default .Chart.Name .Values.nameOverride }}
{{- if contains $name .Release.Name }}
{{- .Release.Name | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}
{{- end }}

{{/*
The image tag. An empty value means the chart's appVersion. Tags are plain
integers here, and `--set image.tag=2` hands the template an int, so it is
turned into a string before anything prints it.
*/}}
{{- define "arith.tag" -}}
{{- .Values.image.tag | toString | default .Chart.AppVersion }}
{{- end }}

{{- define "arith.image" -}}
{{- printf "%s:%s" .Values.image.repository (include "arith.tag" .) }}
{{- end }}

{{- define "arith.labels" -}}
helm.sh/chart: {{ printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{ include "arith.selectorLabels" . }}
app.kubernetes.io/version: {{ include "arith.tag" . | quote }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{- define "arith.selectorLabels" -}}
app.kubernetes.io/name: {{ include "arith.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}
