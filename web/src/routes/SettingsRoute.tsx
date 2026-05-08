import { useEffect, useState } from "react";
import { Loader2, Lock, Save } from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useConfig, useConfigPatch } from "@/hooks/useConfig";
import type { AppConfig } from "@/lib/api";

type SectionKey = "wiki" | "graph" | "log" | "embedding";

const SECTION_LIST: { key: SectionKey; label: string; note: string }[] = [
  { key: "wiki", label: "Wiki", note: "기본 backend와 backend별 모델 설정" },
  { key: "graph", label: "Graph", note: "시맨틱 추출 backend와 모델 설정" },
  { key: "log", label: "Log", note: "Daily diary backend와 model/api_url 설정" },
  { key: "embedding", label: "Embedding", note: "임베딩 backend 설정" },
];

export default function SettingsRoute() {
  const { data, isLoading, error } = useConfig();
  const patch = useConfigPatch();
  const [section, setSection] = useState<SectionKey>("wiki");
  const [readOnly, setReadOnly] = useState(false);
  const [wikiForm, setWikiForm] = useState<AppConfig["wiki"] | null>(null);
  const [graphForm, setGraphForm] = useState<AppConfig["graph"] | null>(null);
  const [logForm, setLogForm] = useState<AppConfig["log"] | null>(null);
  const [embeddingForm, setEmbeddingForm] = useState<AppConfig["embedding"] | null>(null);

  useEffect(() => {
    if (!data) return;
    setWikiForm(data.wiki);
    setGraphForm(data.graph);
    setLogForm(data.log);
    setEmbeddingForm(data.embedding);
  }, [data]);

  useEffect(() => {
    if (patch.error instanceof Error && patch.error.message.includes("403")) {
      setReadOnly(true);
    }
  }, [patch.error]);

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

  if (!data || !wikiForm || !graphForm || !logForm || !embeddingForm) {
    return null;
  }

  return (
    <div className="h-full overflow-auto bg-[var(--bg)]">
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
                <div className="text-t-h2 font-medium">{item.label}</div>
                <div className="mt-1 text-t-meta">{item.note}</div>
              </button>
            ))}
          </aside>

          <div className="min-w-0">
            {section === "wiki" && (
              <Card className="border-hairline">
                <CardHeader>
                  <CardTitle>Wiki Settings</CardTitle>
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
                  <Field label="Review model">
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
                      <div key={backend} className="rounded-xl border border-hairline bg-surface-2 p-ds-4">
                        <div className="mb-ds-3 text-t-h2 font-medium">{backend}</div>
                        <div className="grid grid-cols-1 gap-ds-3 md:grid-cols-3">
                          <Field label="Model">
                            <Input
                              value={backendCfg.model ?? ""}
                              onChange={(e) =>
                                setWikiForm((prev) =>
                                  prev
                                    ? {
                                        ...prev,
                                        backends: {
                                          ...prev.backends,
                                          [backend]: { ...backendCfg, model: e.target.value },
                                        },
                                      }
                                    : prev,
                                )
                              }
                              disabled={readOnly}
                            />
                          </Field>
                          <Field label="API URL">
                            <Input
                              value={backendCfg.api_url ?? ""}
                              onChange={(e) =>
                                setWikiForm((prev) =>
                                  prev
                                    ? {
                                        ...prev,
                                        backends: {
                                          ...prev.backends,
                                          [backend]: { ...backendCfg, api_url: e.target.value },
                                        },
                                      }
                                    : prev,
                                )
                              }
                              disabled={readOnly}
                            />
                          </Field>
                          <Field label="Max tokens">
                            <Input
                              type="number"
                              value={String(backendCfg.max_tokens ?? 4096)}
                              onChange={(e) =>
                                setWikiForm((prev) =>
                                  prev
                                    ? {
                                        ...prev,
                                        backends: {
                                          ...prev.backends,
                                          [backend]: {
                                            ...backendCfg,
                                            max_tokens: Number(e.target.value || 0),
                                          },
                                        },
                                      }
                                    : prev,
                                )
                              }
                              disabled={readOnly}
                            />
                          </Field>
                        </div>
                      </div>
                    );
                  })}
                  <SaveRow
                    disabled={readOnly || patch.isPending}
                    onSave={() => patch.mutate({ section: "wiki", body: wikiForm })}
                  />
                </CardContent>
              </Card>
            )}

            {section === "graph" && (
              <Card className="border-hairline">
                <CardHeader><CardTitle>Graph Settings</CardTitle></CardHeader>
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
                        {["ollama", "anthropic", "gemini", "lmstudio", "disabled"].map((item) => (
                          <SelectItem key={item} value={item}>{item}</SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </Field>
                  <SettingsGrid>
                    <Field label="Ollama / LM Studio URL">
                      <Input
                        value={graphForm.ollama_url ?? ""}
                        onChange={(e) =>
                          setGraphForm((prev) => (prev ? { ...prev, ollama_url: e.target.value } : prev))
                        }
                        disabled={readOnly}
                      />
                    </Field>
                    <Field label="Ollama model">
                      <Input
                        value={graphForm.ollama_model ?? ""}
                        onChange={(e) =>
                          setGraphForm((prev) => (prev ? { ...prev, ollama_model: e.target.value } : prev))
                        }
                        disabled={readOnly}
                      />
                    </Field>
                    <Field label="Anthropic model">
                      <Input
                        value={graphForm.anthropic_model ?? ""}
                        onChange={(e) =>
                          setGraphForm((prev) => (prev ? { ...prev, anthropic_model: e.target.value } : prev))
                        }
                        disabled={readOnly}
                      />
                    </Field>
                    <Field label="Gemini model">
                      <Input
                        value={graphForm.gemini_model ?? ""}
                        onChange={(e) =>
                          setGraphForm((prev) => (prev ? { ...prev, gemini_model: e.target.value } : prev))
                        }
                        disabled={readOnly}
                      />
                    </Field>
                  </SettingsGrid>
                  <Field label="Gemini API key">
                    <Input value="<masked>" disabled placeholder="<env>" />
                  </Field>
                  <SaveRow
                    disabled={readOnly || patch.isPending}
                    onSave={() => patch.mutate({ section: "graph", body: graphForm })}
                  />
                </CardContent>
              </Card>
            )}

            {section === "log" && (
              <Card className="border-hairline">
                <CardHeader><CardTitle>Log Settings</CardTitle></CardHeader>
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
                        {["claude", "codex", "haiku", "ollama", "lmstudio"].map((item) => (
                          <SelectItem key={item} value={item}>{item}</SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </Field>
                  <SettingsGrid>
                    <Field label="Model">
                      <Input
                        value={logForm.model ?? ""}
                        onChange={(e) =>
                          setLogForm((prev) => (prev ? { ...prev, model: e.target.value } : prev))
                        }
                        disabled={readOnly}
                      />
                    </Field>
                    <Field label="API URL">
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
                  </SettingsGrid>
                  <SaveRow
                    disabled={readOnly || patch.isPending}
                    onSave={() => patch.mutate({ section: "log", body: logForm })}
                  />
                </CardContent>
              </Card>
            )}

            {section === "embedding" && (
              <Card className="border-hairline">
                <CardHeader><CardTitle>Embedding Settings</CardTitle></CardHeader>
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
                        {["ollama", "ort", "openai", "openvino"].map((item) => (
                          <SelectItem key={item} value={item}>{item}</SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </Field>
                  <SettingsGrid>
                    <Field label="Ollama URL">
                      <Input
                        value={embeddingForm.ollama_url ?? ""}
                        onChange={(e) =>
                          setEmbeddingForm((prev) => (prev ? { ...prev, ollama_url: e.target.value } : prev))
                        }
                        disabled={readOnly}
                      />
                    </Field>
                    <Field label="Ollama model">
                      <Input
                        value={embeddingForm.ollama_model ?? ""}
                        onChange={(e) =>
                          setEmbeddingForm((prev) => (prev ? { ...prev, ollama_model: e.target.value } : prev))
                        }
                        disabled={readOnly}
                      />
                    </Field>
                    <Field label="OpenAI model">
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
                  </SettingsGrid>
                  <SaveRow
                    disabled={readOnly || patch.isPending}
                    onSave={() => patch.mutate({ section: "embedding", body: embeddingForm })}
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

function Field({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <label className="block space-y-ds-2">
      <div className="text-t-meta uppercase tracking-[0.12em] text-text-3">{label}</div>
      {children}
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
