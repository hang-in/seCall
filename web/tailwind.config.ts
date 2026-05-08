import type { Config } from "tailwindcss";

/**
 * secall-web Tailwind config
 *
 * 디자인 시스템 토큰은 web/src/index.css 에서 CSS variable 로 정의되고,
 * 본 config 는 그 위에 Tailwind 매핑만 둔다 (web/src/lib/design-tokens.md 참고).
 * shadcn/ui 호환을 위해 hsl(var(--foo)) 형태를 유지한다.
 */
export default {
  darkMode: "class",
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      fontFamily: {
        sans: [
          "Pretendard Variable",
          "Geist",
          "-apple-system",
          "BlinkMacSystemFont",
          "system-ui",
          "sans-serif",
        ],
        mono: [
          "Geist Mono",
          "ui-monospace",
          "SF Mono",
          "Menlo",
          "monospace",
        ],
      },
      // shadcn/ui 컴포넌트가 참조하는 색 (hsl token).
      // prototype 의 --bg/--surface 등 hex token 은 inline `var(...)` 로 직접 사용한다.
      colors: {
        background: "hsl(var(--background))",
        foreground: "hsl(var(--foreground))",
        card: {
          DEFAULT: "hsl(var(--card))",
          foreground: "hsl(var(--card-foreground))",
        },
        primary: {
          DEFAULT: "hsl(var(--primary))",
          foreground: "hsl(var(--primary-foreground))",
        },
        secondary: {
          DEFAULT: "hsl(var(--secondary))",
          foreground: "hsl(var(--secondary-foreground))",
        },
        muted: {
          DEFAULT: "hsl(var(--muted))",
          foreground: "hsl(var(--muted-foreground))",
        },
        accent: {
          // shadcn/ui 의 "accent" 는 hover 배경(neutral)이라 prototype 의 brand accent 와 다름.
          // brand accent(=indigo) 는 별도 `brand` 컬러로 노출.
          DEFAULT: "hsl(var(--accent-bg))",
          foreground: "hsl(var(--accent-foreground))",
        },
        destructive: {
          DEFAULT: "hsl(var(--destructive))",
          foreground: "hsl(var(--destructive-foreground))",
        },
        popover: {
          DEFAULT: "hsl(var(--popover))",
          foreground: "hsl(var(--popover-foreground))",
        },
        border: "hsl(var(--border))",
        input: "hsl(var(--input))",
        ring: "hsl(var(--ring))",

        // prototype 의 hex token 들 — 컴포넌트에서 `bg-surface`, `text-text-3` 등으로 사용 가능
        surface: {
          DEFAULT: "var(--surface)",
          2: "var(--surface-2)",
          3: "var(--surface-3)",
        },
        hairline: "var(--hairline)",
        "border-strong": "var(--border-strong)",
        text: {
          DEFAULT: "var(--text)",
          2: "var(--text-2)",
          3: "var(--text-3)",
          4: "var(--text-4)",
          "on-accent": "var(--text-on-accent)",
        },
        brand: {
          // 본 도구의 단일 brand accent (indigo) — 버튼 fill, link, focus
          DEFAULT: "var(--accent)",
          hover: "var(--accent-hover)",
          soft: "var(--accent-soft)",
          "border-soft": "var(--accent-border)",
        },
        status: {
          danger: "var(--danger)",
          warn: "var(--warn)",
          success: "var(--success)",
          info: "var(--info)",
        },
        agent: {
          "claude-code": "var(--agent-claude-code)",
          codex: "var(--agent-codex)",
          gemini: "var(--agent-gemini)",
          "claude-ai": "var(--agent-claude-ai)",
          chatgpt: "var(--agent-chatgpt)",
        },
      },
      // 8-grid (4 half-step). gap-1=4px, gap-2=8px, gap-3=12px, ...
      spacing: {
        "ds-1": "var(--s-1)",
        "ds-2": "var(--s-2)",
        "ds-3": "var(--s-3)",
        "ds-4": "var(--s-4)",
        "ds-5": "var(--s-5)",
        "ds-6": "var(--s-6)",
        "ds-7": "var(--s-7)",
        "ds-8": "var(--s-8)",
        "ds-9": "var(--s-9)",
        "ds-10": "var(--s-10)",
        "nav-h": "var(--nav-h)",
        "list-w": "var(--list-w)",
        "read-w": "var(--read-w)",
      },
      borderRadius: {
        none: "0",
        sm: "var(--r-1)",
        md: "var(--r-2)",
        lg: "var(--r-3)",
        xl: "var(--r-4)",
        "2xl": "var(--r-5)",
        full: "999px",
      },
      boxShadow: {
        "ds-1": "var(--shadow-1)",
        "ds-2": "var(--shadow-2)",
        "ds-pop": "var(--shadow-pop)",
      },
      transitionTimingFunction: {
        ds: "cubic-bezier(.2,0,0,1)",
      },
      transitionDuration: {
        fast: "120ms",
        base: "160ms",
        slow: "240ms",
      },
      // type scale — variable 직참
      fontSize: {
        "t-display-s": ["var(--t-display-s)", { lineHeight: "var(--t-display-s-lh)", letterSpacing: "var(--t-display-s-tr)" }],
        "t-h1":        ["var(--t-h1)",        { lineHeight: "var(--t-h1-lh)",        letterSpacing: "var(--t-h1-tr)" }],
        "t-h2":        ["var(--t-h2)",        { lineHeight: "var(--t-h2-lh)" }],
        "t-h3":        ["var(--t-h3)",        { lineHeight: "var(--t-h3-lh)" }],
        "t-body":      ["var(--t-body)",      { lineHeight: "var(--t-body-lh)" }],
        "t-prose":     ["var(--t-prose)",     { lineHeight: "var(--t-prose-lh)" }],
        "t-small":     ["var(--t-small)",     { lineHeight: "var(--t-small-lh)" }],
        "t-meta":      ["var(--t-meta)",      { lineHeight: "var(--t-meta-lh)",     letterSpacing: "var(--t-meta-tr)" }],
        "t-caption":   ["var(--t-caption)",   { lineHeight: "var(--t-caption-lh)",  letterSpacing: "var(--t-caption-tr)" }],
        "t-mono":      ["var(--t-mono)",      { lineHeight: "var(--t-mono-lh)" }],
      },
    },
  },
  plugins: [require("@tailwindcss/typography")],
} satisfies Config;
