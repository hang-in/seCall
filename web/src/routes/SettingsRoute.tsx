import { useEffect, useMemo, useState } from "react";
import { Loader2, Lock, Save } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { MaskedKeyInfoModal } from "@/components/settings/MaskedKeyInfoModal";
import { useConfig, useConfigPatch } from "@/hooks/useConfig";
import type { AppConfig, ConfigBackendConfig } from "@/lib/api";
import { validateHttpUrl, validateModelName } from "@/lib/validators";

type SectionKey = "wiki" | "graph" | "log" | "embedding";

type FormsState = {
  wiki: AppConfig["wiki"];
  graph: AppConfig["graph"];
  log: AppConfig["log"];
  embedding: AppConfig["embedding"];
};

type SectionErrors = {
  wiki: string[];
  graph: string[];
  log: string[];
  embedding: string[];
};

const SECTION_LIST: { key: SectionKey; label: string; note: string }[] = [
  { key: "wiki", label: "Wiki", note: "기본 backend와 backend별 모델 설정" },
  { key: "graph", label: "Graph", note: "시맨틱 추출 backend와 모델 설정" },
  { key: "log", label: "Log", note: "Daily diary backend와 model/api_url 설정" },
  { key: "embedding", label: "Embedding", note: "임베딩 backend 설정" },
];

function cloneForms(data: AppConfig): FormsState {
  return {
    wiki: structuredClone(data.wiki),
    graph: structuredClone(data.graph),
    log: structuredClone(data.log),
    embedding: structuredClone(data.embedding),
  };
}

function sameValue(a: unknown, b: unknown) {
  return JSON.stringify(a) === JSON.stringify(b);
}

function fieldError(...errors: Array<string | null | undefined>) {
  return errors.find(Boolean) ?? null;
}

export default function SettingsRoute() {
  const { data, isLoading, error } = useConfig();
  const patch = useConfigPatch();
  const [section, setSection] = useState<SectionKey>("wiki");
  const [readOnly, setReadOnly] = useState(false);
  const [wikiForm, setWikiForm] = useState<AppConfig["wiki"] | null>(null);
  const [graphForm, setGraphForm] = useState<AppConfig["graph"] | null>(null);
  const [logForm, setLogForm] = useState<AppConfig["log"] | null>(null);
  const [embeddingForm, setEmbeddingForm] = useState<AppConfig["embedding"] | null>(null);
  const [initialForms, setInitialForms] = useState<FormsState | null>(null);
  const [maskedModalOpen, setMaskedModalOpen] = useState(false);

  useEffect(() => {
    if (!data) return;
    const next = cloneForms(data);
    setWikiForm(next.wiki);
    setGraphForm(next.graph);
    setLogForm(next.log);
    setEmbeddingForm(next.embedding);
    setInitialForms(next);
  }, [data]);

  useEffect(() => {
    if (patch.error instanceof Error && patch.error.message.includes("403")) {
      setReadOnly(true);
    }
  }, [patch.error]);

  const sectionErrors = useMemo<SectionErrors>(() => {
    const wiki: string[] = [];
    const graph: string[] = [];
    const log: string[] = [];
    const embedding: string[] = [];

    if (wikiForm) {
      wiki.push(
        ...Object.entries(wikiForm.backends).flatMap(([backend, cfg]) =>
          [
            validateModelName(cfg.model ?? ""),
            validateHttpUrl(cfg.api_url ?? ""),
          ]
            .filter(Boolean)
            .map((msg) => `${backend}: ${msg}`),
        ),
      );
      const reviewModelError = validateModelName(wikiForm.review_model ?? "");
      if (reviewModelError) wiki.push(`review_model: ${reviewModelError}`);
    }

    if (graphForm) {
      const urlError = validateHttpUrl(graphForm.ollama_url ?? "");
      const ollamaModelError = validateModelName(graphForm.ollama_model ?? "");
      const anthropicModelError = validateModelName(graphForm.anthropic_model ?? "");
      const cloudModelError = validateModelName(graphForm.cloud_model ?? "");
      const cloudHostError = validateHttpUrl(graphForm.cloud_host ?? "");
      if (urlError) graph.push(`ollama_url: ${urlError}`);
      if (ollamaModelError) graph.push(`ollama_model: ${ollamaModelError}`);
      if (anthropicModelError) graph.push(`anthropic_model: ${anthropicModelError}`);
      if (cloudModelError) graph.push(`cloud_model: ${cloudModelError}`);
      if (cloudHostError) graph.push(`cloud_host: ${cloudHostError}`);
    }

    if (logForm) {
      const modelError = validateModelName(logForm.model ?? "");
      const urlError = validateHttpUrl(logForm.api_url ?? "");
      const cloudModelError = validateModelName(logForm.cloud_model ?? "");
      const cloudHostError = validateHttpUrl(logForm.cloud_host ?? "");
      if (modelError) log.push(`model: ${modelError}`);
      if (urlError) log.push(`api_url: ${urlError}`);
      if (cloudModelError) log.push(`cloud_model: ${cloudModelError}`);
      if (cloudHostError) log.push(`cloud_host: ${cloudHostError}`);
    }

    if (embeddingForm) {
      const ollamaModelError = validateModelName(embeddingForm.ollama_model ?? "");
      const openAiModelError = validateModelName(embeddingForm.openai_model ?? "");
      const urlError = validateHttpUrl(embeddingForm.ollama_url ?? "");
      const cloudHostError = validateHttpUrl(embeddingForm.cloud_host ?? "");
      const cloudModelError = validateModelName(embeddingForm.cloud_model ?? "");
      if (ollamaModelError) embedding.push(`ollama_model: ${ollamaModelError}`);
      if (openAiModelError) embedding.push(`openai_model: ${openAiModelError}`);
      if (urlError) embedding.push(`ollama_url: ${urlError}`);
      if (cloudHostError) embedding.push(`cloud_host: ${cloudHostError}`);
      if (cloudModelError) embedding.push(`cloud_model: ${cloudModelError}`);
    }

    return { wiki, graph, log, embedding };
  }, [embeddingForm, graphForm, logForm, wikiForm]);

  if (isLoading) {
    return (
      <div className="h-full flex items-center justify-center text-t-small text-text-3">
        <Loader2 className="size-4 animate-spin mr-ds-2" /> 설정 로드 중…
      </div>
    );
  }

  if (error) {
    const msg = error instanceof Error ? error.message : String(error);
    return (
      <div className="h-full flex items-center justify-center px-ds-6">
        <div className="text-t-small text-status-danger whitespace-pre-wrap">
          설정 로드 실패: {msg}
        </div>
      </div>
    );
  }

  if (!data || !wikiForm || !graphForm || !logForm || !embeddingForm || !initialForms) {
    return null;
  }

  const isDirty = (key: SectionKey) => {
    const current = { wiki: wikiForm, graph: graphForm, log: logForm, embedding: embeddingForm }[key];
    return !sameValue(current, initialForms[key]);
  };

  const currentErrors = sectionErrors[section];
  const saveDisabled =
    readOnly ||
    patch.isPending ||
    currentErrors.length > 0 ||
    !isDirty(section);

  const handleSave = async (key: SectionKey, body: unknown) => {
    const result = await patch.mutateAsync({ section: key, body });
    const next = cloneForms(result.data);
    setWikiForm(next.wiki);
    setGraphForm(next.graph);
    setLogForm(next.log);
    setEmbeddingForm(next.embedding);
    setInitialForms(next);
  };

  return (
    <div className="h-full overflow-auto bg-[var(--bg)]">
      <MaskedKeyInfoModal
        open={maskedModalOpen}
        onOpenChange={setMaskedModalOpen}
        envVar="OLLAMA_CLOUD_API_KEY"
      />

      <div className="mx-auto max-w-6xl px-ds-6 py-ds-6">
        <header className="mb-ds-6 flex items-start justify-between gap-ds-4">
          <div className="space-y-ds-1">
            <div className="eyebrow">Settings</div>
            <h1 className="text-t-display-s font-medium tracking-tight">LLM Configuration</h1>
            <p className="text-t-small text-text-3">
              Wiki / Graph / Log / Embedding 설정을 확인하고 저장합니다.
            </p>
          </div>
          {readOnly && (
            <div className="inline-flex items-center gap-ds-2 rounded-md border border-hairline bg-surface-2 px-ds-3 py-ds-2 text-t-small text-text-3">
              <Lock className="size-4" /> 읽기 전용 모드 — `secall serve --allow-config-edit`
            </div>
          )}
        </header>

        <div className="grid grid-cols-1 gap-ds-4 lg:grid-cols-[240px_minmax(0,1fr)]">
          <aside className="space-y-ds-2">
            {SECTION_LIST.map((item) => (
              <button
                key={item.key}
                type="button"
                onClick={() => setSection(item.key)}
                className={[
                  "w-full rounded-xl border px-ds-3 py-ds-3 text-left transition-colors duration-fast",
                  section === item.key
                    ? "border-[var(--accent)] bg-surface-2 text-text"
                    : "border-hairline bg-[var(--surface)] text-text-3 hover:bg-surface-2 hover:text-text",
                ].join(" ")}
              >
                <div className="flex items-center justify-between gap-ds-2">
                  <div className="text-t-h2 font-medium">{item.label}</div>
                  {isDirty(item.key) && (
                    <Badge
                      variant="outline"
                      className="border-status-warn/40 bg-status-warn/10 text-status-warn"
                    >
                      변경됨
                    </Badge>
                  )}
                </div>
                <div className="mt-1 text-t-meta">{item.note}</div>
              </button>
            ))}
          </aside>

          <div className="min-w-0">
            {section === "wiki" && (
              <Card className="border-hairline">
                <CardHeader>
                  <SectionTitle title="Wiki Settings" dirty={isDirty("wiki")} />
                </CardHeader>
                <CardContent className="space-y-ds-4">
                  <Field label="Default backend">
                    <Select
                      value={wikiForm.default_backend}
                      onValueChange={(value) =>
                        setWikiForm((prev) => (prev ? { ...prev, default_backend: value } : prev))
                      }
                      disabled={readOnly}
                    >
                      <SelectTrigger><SelectValue /></SelectTrigger>
                      <SelectContent>
                        {["claude", "codex", "haiku", "ollama", "lmstudio"].map((item) => (
                          <SelectItem key={item} value={item}>{item}</SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </Field>
                  <Field
                    label="Review model"
                    error={fieldError(validateModelName(wikiForm.review_model ?? ""))}
                  >
                    <Input
                      value={wikiForm.review_model ?? ""}
                      onChange={(e) =>
                        setWikiForm((prev) => (prev ? { ...prev, review_model: e.target.value } : prev))
                      }
                      disabled={readOnly}
                      placeholder="sonnet"
                    />
                  </Field>
                  {["claude", "codex", "haiku", "ollama", "lmstudio"].map((backend) => {
                    const backendCfg = wikiForm.backends[backend] ?? {};
                    return (
                      <BackendCard
                        key={backend}
                        backend={backend}
                        config={backendCfg}
                        readOnly={readOnly}
                        onChange={(next) =>
                          setWikiForm((prev) =>
                            prev
                              ? {
                                  ...prev,
                                  backends: {
                                    ...prev.backends,
                                    [backend]: next,
                                  },
                                }
                              : prev,
                          )
                        }
                      />
                    );
                  })}
                  <SaveRow
                    disabled={saveDisabled}
                    onSave={() => handleSave("wiki", wikiForm)}
                  />
                </CardContent>
              </Card>
            )}

            {section === "graph" && (
              <Card className="border-hairline">
                <CardHeader>
                  <SectionTitle title="Graph Settings" dirty={isDirty("graph")} />
                </CardHeader>
                <CardContent className="space-y-ds-4">
                  <label className="flex items-center gap-ds-2 text-t-small text-text-2">
                    <input
                      type="checkbox"
                      checked={graphForm.semantic}
                      onChange={(e) =>
                        setGraphForm((prev) => (prev ? { ...prev, semantic: e.target.checked } : prev))
                      }
                      disabled={readOnly}
                    />
                    Semantic extraction enabled
                  </label>
                  <Field label="Semantic backend">
                    <Select
                      value={graphForm.semantic_backend}
                      onValueChange={(value) =>
                        setGraphForm((prev) => (prev ? { ...prev, semantic_backend: value } : prev))
                      }
                      disabled={readOnly}
                    >
                      <SelectTrigger><SelectValue /></SelectTrigger>
                      <SelectContent>
                        {["ollama", "anthropic", "ollama_cloud", "lmstudio", "disabled"].map((item) => (
                          <SelectItem key={item} value={item}>{item}</SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </Field>
                  <SettingsGrid>
                    <Field
                      label="Ollama / LM Studio URL"
                      error={fieldError(validateHttpUrl(graphForm.ollama_url ?? ""))}
                    >
                      <Input
                        value={graphForm.ollama_url ?? ""}
                        onChange={(e) =>
                          setGraphForm((prev) => (prev ? { ...prev, ollama_url: e.target.value } : prev))
                        }
                        disabled={readOnly}
                      />
                    </Field>
                    <Field
                      label="Ollama model"
                      error={fieldError(validateModelName(graphForm.ollama_model ?? ""))}
                    >
                      <Input
                        value={graphForm.ollama_model ?? ""}
                        onChange={(e) =>
                          setGraphForm((prev) => (prev ? { ...prev, ollama_model: e.target.value } : prev))
                        }
                        disabled={readOnly}
                      />
                    </Field>
                    <Field
                      label="Anthropic model"
                      error={fieldError(validateModelName(graphForm.anthropic_model ?? ""))}
                    >
                      <Input
                        value={graphForm.anthropic_model ?? ""}
                        onChange={(e) =>
                          setGraphForm((prev) => (prev ? { ...prev, anthropic_model: e.target.value } : prev))
                        }
                        disabled={readOnly}
                      />
                    </Field>
                    <Field
                      label="Cloud host"
                      error={fieldError(validateHttpUrl(graphForm.cloud_host ?? ""))}
                    >
                      <Input
                        aria-label="Cloud host"
                        value={graphForm.cloud_host ?? ""}
                        onChange={(e) =>
                          setGraphForm((prev) => (prev ? { ...prev, cloud_host: e.target.value } : prev))
                        }
                        disabled={readOnly}
                        placeholder="https://ollama.com"
                      />
                    </Field>
                    <Field
                      label="Cloud model"
                      error={fieldError(validateModelName(graphForm.cloud_model ?? ""))}
                    >
                      <Input
                        aria-label="Cloud model"
                        value={graphForm.cloud_model ?? ""}
                        onChange={(e) =>
                          setGraphForm((prev) => (prev ? { ...prev, cloud_model: e.target.value } : prev))
                        }
                        disabled={readOnly}
                        placeholder="gemma4:31b-cloud"
                      />
                    </Field>
                  </SettingsGrid>
                  <Field
                    label="Ollama Cloud API key"
                    hint="환경변수 또는 .env 에서만 관리합니다."
                  >
                    <button
                      type="button"
                      className="w-full text-left"
                      onClick={() => setMaskedModalOpen(true)}
                    >
                      <Input
                        aria-label="Ollama Cloud API key"
                        value="<masked>"
                        readOnly
                        className="cursor-pointer"
                        placeholder="<env>"
                      />
                    </button>
                  </Field>
                  <SaveRow
                    disabled={saveDisabled}
                    onSave={() => handleSave("graph", graphForm)}
                  />
                </CardContent>
              </Card>
            )}

            {section === "log" && (
              <Card className="border-hairline">
                <CardHeader>
                  <SectionTitle title="Log Settings" dirty={isDirty("log")} />
                </CardHeader>
                <CardContent className="space-y-ds-4">
                  <Field label="Backend">
                    <Select
                      value={logForm.backend ?? ""}
                      onValueChange={(value) =>
                        setLogForm((prev) => (prev ? { ...prev, backend: value } : prev))
                      }
                      disabled={readOnly}
                    >
                      <SelectTrigger><SelectValue placeholder="(graph fallback)" /></SelectTrigger>
                      <SelectContent>
                        {["claude", "codex", "haiku", "ollama", "ollama_cloud", "lmstudio"].map((item) => (
                          <SelectItem key={item} value={item}>{item}</SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </Field>
                  <SettingsGrid>
                    <Field label="Model" error={fieldError(validateModelName(logForm.model ?? ""))}>
                      <Input
                        value={logForm.model ?? ""}
                        onChange={(e) =>
                          setLogForm((prev) => (prev ? { ...prev, model: e.target.value } : prev))
                        }
                        disabled={readOnly}
                      />
                    </Field>
                    <Field label="API URL" error={fieldError(validateHttpUrl(logForm.api_url ?? ""))}>
                      <Input
                        value={logForm.api_url ?? ""}
                        onChange={(e) =>
                          setLogForm((prev) => (prev ? { ...prev, api_url: e.target.value } : prev))
                        }
                        disabled={readOnly}
                      />
                    </Field>
                    <Field label="Max tokens">
                      <Input
                        type="number"
                        value={String(logForm.max_tokens ?? "")}
                        onChange={(e) =>
                          setLogForm((prev) => ({
                            ...prev,
                            max_tokens: e.target.value ? Number(e.target.value) : null,
                          }))
                        }
                        disabled={readOnly}
                      />
                    </Field>
                    <Field
                      label="Cloud host"
                      error={fieldError(validateHttpUrl(logForm.cloud_host ?? ""))}
                    >
                      <Input
                        aria-label="Log cloud host"
                        value={logForm.cloud_host ?? ""}
                        onChange={(e) =>
                          setLogForm((prev) => (prev ? { ...prev, cloud_host: e.target.value } : prev))
                        }
                        disabled={readOnly}
                        placeholder="https://ollama.com"
                      />
                    </Field>
                    <Field
                      label="Cloud model"
                      error={fieldError(validateModelName(logForm.cloud_model ?? ""))}
                    >
                      <Input
                        aria-label="Log cloud model"
                        value={logForm.cloud_model ?? ""}
                        onChange={(e) =>
                          setLogForm((prev) => (prev ? { ...prev, cloud_model: e.target.value } : prev))
                        }
                        disabled={readOnly}
                        placeholder="kimi-k2.6:cloud"
                      />
                    </Field>
                  </SettingsGrid>
                  <SaveRow
                    disabled={saveDisabled}
                    onSave={() => handleSave("log", logForm)}
                  />
                </CardContent>
              </Card>
            )}

            {section === "embedding" && (
              <Card className="border-hairline">
                <CardHeader>
                  <SectionTitle title="Embedding Settings" dirty={isDirty("embedding")} />
                </CardHeader>
                <CardContent className="space-y-ds-4">
                  <Field label="Backend">
                    <Select
                      value={embeddingForm.backend}
                      onValueChange={(value) =>
                        setEmbeddingForm((prev) => (prev ? { ...prev, backend: value } : prev))
                      }
                      disabled={readOnly}
                    >
                      <SelectTrigger><SelectValue /></SelectTrigger>
                      <SelectContent>
                        {["ollama", "ort", "openai", "openvino", "ollama_cloud"].map((item) => (
                          <SelectItem key={item} value={item}>{item}</SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </Field>
                  <SettingsGrid>
                    <Field
                      label="Ollama URL"
                      error={fieldError(validateHttpUrl(embeddingForm.ollama_url ?? ""))}
                    >
                      <Input
                        value={embeddingForm.ollama_url ?? ""}
                        onChange={(e) =>
                          setEmbeddingForm((prev) => (prev ? { ...prev, ollama_url: e.target.value } : prev))
                        }
                        disabled={readOnly}
                      />
                    </Field>
                    <Field
                      label="Ollama model"
                      error={fieldError(validateModelName(embeddingForm.ollama_model ?? ""))}
                    >
                      <Input
                        value={embeddingForm.ollama_model ?? ""}
                        onChange={(e) =>
                          setEmbeddingForm((prev) => (prev ? { ...prev, ollama_model: e.target.value } : prev))
                        }
                        disabled={readOnly}
                      />
                    </Field>
                    <Field
                      label="OpenAI model"
                      error={fieldError(validateModelName(embeddingForm.openai_model ?? ""))}
                    >
                      <Input
                        value={embeddingForm.openai_model ?? ""}
                        onChange={(e) =>
                          setEmbeddingForm((prev) => (prev ? { ...prev, openai_model: e.target.value } : prev))
                        }
                        disabled={readOnly}
                      />
                    </Field>
                    <Field label="OpenVINO device">
                      <Input
                        value={embeddingForm.openvino_device ?? ""}
                        onChange={(e) =>
                          setEmbeddingForm((prev) =>
                            prev ? { ...prev, openvino_device: e.target.value } : prev,
                          )
                        }
                        disabled={readOnly}
                      />
                    </Field>
                    <Field label="Pool size">
                      <Input
                        aria-label="Embedding pool size"
                        type="number"
                        min={1}
                        value={embeddingForm.pool_size ?? ""}
                        onChange={(e) =>
                          setEmbeddingForm((prev) =>
                            prev
                              ? {
                                  ...prev,
                                  pool_size: e.target.value === "" ? null : parseInt(e.target.value, 10),
                                }
                              : prev,
                          )
                        }
                        disabled={readOnly}
                      />
                    </Field>
                    <Field
                      label="Embedding cloud host"
                      error={fieldError(validateHttpUrl(embeddingForm.cloud_host ?? ""))}
                    >
                      <Input
                        aria-label="Embedding cloud host"
                        value={embeddingForm.cloud_host ?? ""}
                        onChange={(e) =>
                          setEmbeddingForm((prev) => (prev ? { ...prev, cloud_host: e.target.value } : prev))
                        }
                        disabled={readOnly}
                      />
                    </Field>
                    <Field
                      label="Embedding cloud model"
                      error={fieldError(validateModelName(embeddingForm.cloud_model ?? ""))}
                    >
                      <Input
                        aria-label="Embedding cloud model"
                        value={embeddingForm.cloud_model ?? ""}
                        onChange={(e) =>
                          setEmbeddingForm((prev) => (prev ? { ...prev, cloud_model: e.target.value } : prev))
                        }
                        disabled={readOnly}
                      />
                    </Field>
                    <Field label="Ollama Cloud API key">
                      <button
                        type="button"
                        className="text-t-small text-accent underline"
                        onClick={() => setMaskedModalOpen(true)}
                      >
                        환경변수로 관리 (OLLAMA_CLOUD_API_KEY)
                      </button>
                    </Field>
                  </SettingsGrid>
                  <SaveRow
                    disabled={saveDisabled}
                    onSave={() => handleSave("embedding", embeddingForm)}
                  />
                </CardContent>
              </Card>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

function BackendCard({
  backend,
  config,
  readOnly,
  onChange,
}: {
  backend: string;
  config: ConfigBackendConfig;
  readOnly: boolean;
  onChange: (next: ConfigBackendConfig) => void;
}) {
  return (
    <div className="rounded-xl border border-hairline bg-surface-2 p-ds-4">
      <div className="mb-ds-3 text-t-h2 font-medium">{backend}</div>
      <div className="grid grid-cols-1 gap-ds-3 md:grid-cols-3">
        <Field label="Model" error={fieldError(validateModelName(config.model ?? ""))}>
          <Input
            value={config.model ?? ""}
            onChange={(e) => onChange({ ...config, model: e.target.value })}
            disabled={readOnly}
          />
        </Field>
        <Field label="API URL" error={fieldError(validateHttpUrl(config.api_url ?? ""))}>
          <Input
            value={config.api_url ?? ""}
            onChange={(e) => onChange({ ...config, api_url: e.target.value })}
            disabled={readOnly}
          />
        </Field>
        <Field label="Max tokens">
          <Input
            type="number"
            value={String(config.max_tokens ?? 4096)}
            onChange={(e) =>
              onChange({
                ...config,
                max_tokens: Number(e.target.value || 0),
              })
            }
            disabled={readOnly}
          />
        </Field>
      </div>
    </div>
  );
}

function SectionTitle({ title, dirty }: { title: string; dirty: boolean }) {
  return (
    <div className="flex items-center gap-ds-2">
      <CardTitle>{title}</CardTitle>
      {dirty && (
        <Badge
          variant="outline"
          className="border-status-warn/40 bg-status-warn/10 text-status-warn"
        >
          변경됨
        </Badge>
      )}
    </div>
  );
}

function Field({
  label,
  children,
  error,
  hint,
}: {
  label: string;
  children: React.ReactNode;
  error?: string | null;
  hint?: string;
}) {
  return (
    <label className="block space-y-ds-2">
      <div className="text-t-meta uppercase tracking-[0.12em] text-text-3">{label}</div>
      {children}
      {error ? <p className="text-t-meta text-status-danger">{error}</p> : null}
      {!error && hint ? <p className="text-t-meta text-text-3">{hint}</p> : null}
    </label>
  );
}

function SettingsGrid({ children }: { children: React.ReactNode }) {
  return <div className="grid grid-cols-1 gap-ds-3 md:grid-cols-2">{children}</div>;
}

function SaveRow({
  disabled,
  onSave,
}: {
  disabled: boolean;
  onSave: () => void;
}) {
  return (
    <div className="flex justify-end pt-ds-2">
      <Button onClick={onSave} disabled={disabled}>
        <Save className="size-4" /> 저장
      </Button>
    </div>
  );
}
