const MODEL_NAME_RE = /^[a-zA-Z0-9._:-]+$/;

export function validateModelName(value: string): string | null {
  if (!value.trim()) return null;
  return MODEL_NAME_RE.test(value.trim()) ? null : "잘못된 모델 이름";
}

export function validateHttpUrl(value: string): string | null {
  if (!value.trim()) return null;
  try {
    const parsed = new URL(value);
    if (parsed.protocol === "http:" || parsed.protocol === "https:") {
      return null;
    }
    return "URL은 http:// 또는 https:// 로 시작해야 합니다.";
  } catch {
    return "유효한 URL을 입력하세요.";
  }
}
