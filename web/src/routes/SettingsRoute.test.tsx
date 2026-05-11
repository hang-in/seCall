import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import SettingsRoute from "@/routes/SettingsRoute";
import type { AppConfig } from "@/lib/api";

const mockUseConfig = vi.fn();
const mockUseConfigPatch = vi.fn();

vi.mock("@/hooks/useConfig", () => ({
  useConfig: () => mockUseConfig(),
  useConfigPatch: () => mockUseConfigPatch(),
}));

function baseConfig(): AppConfig {
  return {
    wiki: {
      default_backend: "claude",
      review_model: "sonnet",
      backends: {
        claude: { model: "sonnet", max_tokens: 4096 },
        codex: { model: "gpt-5.4", max_tokens: 4096 },
        haiku: { model: "claude-haiku-4-5-20251001", max_tokens: 4096 },
        ollama: { model: "gemma4:e4b", api_url: "http://localhost:11434", max_tokens: 4096 },
        lmstudio: { model: "gemma-4-e4b-it", api_url: "http://localhost:1234", max_tokens: 4096 },
      },
    },
    graph: {
      semantic: true,
      semantic_backend: "ollama_cloud",
      ollama_url: "http://localhost:11434",
      ollama_model: "gemma4:e4b",
      anthropic_model: "claude-haiku-4-5-20251001",
      cloud_host: "https://ollama.com",
      cloud_model: "gemma4:31b-cloud",
      cloud_api_key: "<masked>",
    },
    log: {
      backend: "haiku",
      model: "claude-haiku-4-5-20251001",
      api_url: null,
      max_tokens: 1024,
    },
    embedding: {
      backend: "ollama",
      ollama_url: "http://localhost:11434",
      ollama_model: "bge-m3",
      openai_model: null,
      openvino_device: "CPU",
    },
  };
}

function renderRoute() {
  const client = new QueryClient();
  return render(
    <QueryClientProvider client={client}>
      <SettingsRoute />
    </QueryClientProvider>,
  );
}

beforeEach(() => {
  const config = baseConfig();
  mockUseConfig.mockReturnValue({
    data: config,
    isLoading: false,
    error: null,
  });
  mockUseConfigPatch.mockReturnValue({
    isPending: false,
    error: null,
    mutateAsync: vi.fn(async ({ section, body }: { section: string; body: unknown }) => ({
      section,
      data: {
        ...config,
        [section]: body,
      },
    })),
  });
});

afterEach(() => {
  vi.clearAllMocks();
});

test("shows dirty badge and resets after save", async () => {
  renderRoute();

  fireEvent.click(screen.getByRole("button", { name: /graph/i }));
  const input = screen.getByLabelText("Cloud model");
  fireEvent.change(input, { target: { value: "gemma4:12b-cloud" } });

  expect(screen.getAllByText("변경됨").length).toBeGreaterThan(0);

  fireEvent.click(screen.getByRole("button", { name: /저장/i }));

  await waitFor(() => {
    expect(screen.queryByText("변경됨")).toBeNull();
  });
});

test("shows inline validation and disables save for invalid model", async () => {
  renderRoute();

  fireEvent.click(screen.getByRole("button", { name: /graph/i }));
  const input = screen.getByLabelText("Cloud model");
  fireEvent.change(input, { target: { value: "잘못 모델!@#" } });

  expect(screen.getByText("잘못된 모델 이름")).not.toBeNull();
  const saveButton = screen.getByRole("button", { name: /저장/i });
  expect(saveButton.getAttribute("disabled")).not.toBeNull();
});

test("opens modal when masked key field is clicked", async () => {
  renderRoute();

  fireEvent.click(screen.getByRole("button", { name: /graph/i }));
  fireEvent.click(screen.getByRole("button", { name: /ollama cloud api key/i }));

  await waitFor(() => {
    expect(screen.getByText("OLLAMA_CLOUD_API_KEY 안내")).not.toBeNull();
  });
});

test("log section: invalid cloud_host disables save button", async () => {
  renderRoute();

  fireEvent.click(screen.getByRole("button", { name: /log/i }));
  const input = screen.getByLabelText("Log cloud host");
  fireEvent.change(input, { target: { value: "not-a-url" } });

  expect(screen.getByText("유효한 URL을 입력하세요.")).not.toBeNull();
  const saveButton = screen.getByRole("button", { name: /저장/i });
  expect(saveButton.getAttribute("disabled")).not.toBeNull();
});

test("log section: invalid cloud_model disables save button", async () => {
  renderRoute();

  fireEvent.click(screen.getByRole("button", { name: /log/i }));
  const input = screen.getByLabelText("Log cloud model");
  fireEvent.change(input, { target: { value: "잘못 모델!@#" } });

  expect(screen.getByText("잘못된 모델 이름")).not.toBeNull();
  const saveButton = screen.getByRole("button", { name: /저장/i });
  expect(saveButton.getAttribute("disabled")).not.toBeNull();
});

test("embedding section: ollama_cloud backend renders trigger correctly", async () => {
  // Verify that when embedding.backend is "ollama_cloud", the SelectTrigger shows it
  const config = baseConfig();
  config.embedding.backend = "ollama_cloud";
  mockUseConfig.mockReturnValue({ data: config, isLoading: false, error: null });

  renderRoute();

  fireEvent.click(screen.getByRole("button", { name: /embedding/i }));
  // SelectValue inside the trigger reflects the current backend value
  expect(screen.getByText("ollama_cloud")).not.toBeNull();
});

test("embedding section: invalid cloud_host disables save button", async () => {
  renderRoute();

  fireEvent.click(screen.getByRole("button", { name: /embedding/i }));
  const input = screen.getByLabelText("Embedding cloud host");
  fireEvent.change(input, { target: { value: "not-a-url" } });

  expect(screen.getByText("유효한 URL을 입력하세요.")).not.toBeNull();
  const saveButton = screen.getByRole("button", { name: /저장/i });
  expect(saveButton.getAttribute("disabled")).not.toBeNull();
});

test("embedding section: invalid cloud_model disables save button", async () => {
  renderRoute();

  fireEvent.click(screen.getByRole("button", { name: /embedding/i }));
  const input = screen.getByLabelText("Embedding cloud model");
  fireEvent.change(input, { target: { value: "잘못 모델!@#" } });

  expect(screen.getByText("잘못된 모델 이름")).not.toBeNull();
  const saveButton = screen.getByRole("button", { name: /저장/i });
  expect(saveButton.getAttribute("disabled")).not.toBeNull();
});

test("embedding section: pool_size input accepts number", async () => {
  renderRoute();

  fireEvent.click(screen.getByRole("button", { name: /embedding/i }));
  const input = screen.getByLabelText("Embedding pool size");
  fireEvent.change(input, { target: { value: "2" } });

  expect((input as HTMLInputElement).value).toBe("2");
});
