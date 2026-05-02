import { useState } from "react";
import { format, startOfMonth, startOfWeek } from "date-fns";
import { Star, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useAgents, useProjects } from "@/hooks/useSessions";
import { useAllTags } from "@/lib/allTags";
import { tagColor } from "@/lib/tagColor";
import type { SessionFilterState } from "@/lib/types";

interface Props {
  value: SessionFilterState;
  onChange: (next: SessionFilterState) => void;
}

const ALL = "__all__";

/**
 * 세션 필터 바 — 프로젝트/에이전트/날짜 범위/태그(다중 칩)/즐겨찾기.
 * 모든 변경은 부모로 전체 새 객체를 올림 (불변 갱신).
 *
 * P34 Task 03:
 * - 단일 태그 select → 다중 태그 칩 입력으로 교체. `value.tags`(string[])에 저장.
 * - 날짜 quick range 버튼 4개 (오늘/이번 주/이번 달/전체) 추가.
 * - 단일 `value.tag`(P32 호환)는 schema에 남아있지만 본 UI는 작성하지 않음.
 */
export function SessionFilters({ value, onChange }: Props) {
  const projects = useProjects();
  const agents = useAgents();
  const allTags = useAllTags();

  const [tagDraft, setTagDraft] = useState("");

  const set = <K extends keyof SessionFilterState>(
    key: K,
    next: SessionFilterState[K] | undefined,
  ) => {
    const merged: SessionFilterState = { ...value, [key]: next };
    if (next === undefined || next === "" || next === false) {
      delete merged[key];
    }
    if (Array.isArray(next) && next.length === 0) {
      delete merged[key];
    }
    onChange(merged);
  };

  // ── 태그 칩 ──────────────────────────────────────────────────────────────
  const currentTags = value.tags ?? [];

  const addTag = (raw: string) => {
    // 정규화: 소문자 + 공백→`-` (서버 normalize_tag와 동일 방향).
    const normalized = raw.trim().toLowerCase().replace(/\s+/g, "-");
    if (!normalized) return;
    if (currentTags.includes(normalized)) {
      setTagDraft("");
      return;
    }
    set("tags", [...currentTags, normalized]);
    setTagDraft("");
  };

  const removeTag = (t: string) => {
    set(
      "tags",
      currentTags.filter((x) => x !== t),
    );
  };

  const draftLower = tagDraft.trim().toLowerCase();
  const tagSuggestions = draftLower
    ? allTags
        .filter((t) => t.startsWith(draftLower) && !currentTags.includes(t))
        .slice(0, 6)
    : [];

  // ── 날짜 quick range ────────────────────────────────────────────────────
  const today = format(new Date(), "yyyy-MM-dd");
  const startOfThisWeek = format(
    startOfWeek(new Date(), { weekStartsOn: 1 }),
    "yyyy-MM-dd",
  );
  const startOfThisMonth = format(startOfMonth(new Date()), "yyyy-MM-dd");

  const setRange = (from?: string, to?: string) => {
    const merged: SessionFilterState = { ...value };
    if (from) merged.date_from = from;
    else delete merged.date_from;
    if (to) merged.date_to = to;
    else delete merged.date_to;
    onChange(merged);
  };

  const isRangeToday =
    value.date_from === today && value.date_to === today;
  const isRangeWeek =
    value.date_from === startOfThisWeek && value.date_to === today;
  const isRangeMonth =
    value.date_from === startOfThisMonth && value.date_to === today;
  const isRangeAll = !value.date_from && !value.date_to;

  const reset = () => {
    setTagDraft("");
    onChange({});
  };
  const hasAny = Object.keys(value).length > 0;

  return (
    <div className="space-y-2">
      <div className="grid grid-cols-2 gap-2">
        <Select
          value={value.project ?? ALL}
          onValueChange={(v) => set("project", v === ALL ? undefined : v)}
        >
          <SelectTrigger className="h-8 text-xs">
            <SelectValue placeholder="프로젝트" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value={ALL}>모든 프로젝트</SelectItem>
            {projects.data?.projects.map((p) => (
              <SelectItem key={p} value={p}>
                {p}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>

        <Select
          value={value.agent ?? ALL}
          onValueChange={(v) => set("agent", v === ALL ? undefined : v)}
        >
          <SelectTrigger className="h-8 text-xs">
            <SelectValue placeholder="에이전트" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value={ALL}>모든 에이전트</SelectItem>
            {agents.data?.agents.map((a) => (
              <SelectItem key={a} value={a}>
                {a}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>

      {/* 날짜 quick range */}
      <div className="flex flex-wrap gap-1">
        <Button
          type="button"
          size="sm"
          variant={isRangeToday ? "secondary" : "ghost"}
          className="h-7 px-2 text-xs"
          onClick={() => setRange(today, today)}
        >
          오늘
        </Button>
        <Button
          type="button"
          size="sm"
          variant={isRangeWeek ? "secondary" : "ghost"}
          className="h-7 px-2 text-xs"
          onClick={() => setRange(startOfThisWeek, today)}
        >
          이번 주
        </Button>
        <Button
          type="button"
          size="sm"
          variant={isRangeMonth ? "secondary" : "ghost"}
          className="h-7 px-2 text-xs"
          onClick={() => setRange(startOfThisMonth, today)}
        >
          이번 달
        </Button>
        <Button
          type="button"
          size="sm"
          variant={isRangeAll ? "secondary" : "outline"}
          className="h-7 px-2 text-xs"
          onClick={() => setRange(undefined, undefined)}
        >
          전체
        </Button>
      </div>

      <div className="grid grid-cols-2 gap-2">
        <Input
          type="date"
          value={value.date_from ?? ""}
          onChange={(e) => set("date_from", e.target.value || undefined)}
          className="h-8 text-xs"
          aria-label="시작 날짜"
        />
        <Input
          type="date"
          value={value.date_to ?? ""}
          onChange={(e) => set("date_to", e.target.value || undefined)}
          className="h-8 text-xs"
          aria-label="종료 날짜"
        />
      </div>

      {/* 다중 태그 칩 입력 */}
      <div className="space-y-1">
        <div className="flex flex-wrap items-center gap-1">
          {currentTags.map((t) => (
            <span
              key={t}
              className={`inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-xs ring-1 ${tagColor(t)}`}
            >
              {t}
              <button
                type="button"
                onClick={() => removeTag(t)}
                className="opacity-70 hover:opacity-100"
                aria-label={`태그 ${t} 제거`}
              >
                <X className="size-3" />
              </button>
            </span>
          ))}
          <Input
            type="text"
            value={tagDraft}
            onChange={(e) => setTagDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" || e.key === ",") {
                e.preventDefault();
                addTag(tagDraft);
              } else if (
                e.key === "Backspace" &&
                tagDraft === "" &&
                currentTags.length > 0
              ) {
                removeTag(currentTags[currentTags.length - 1]);
              }
            }}
            placeholder={currentTags.length === 0 ? "태그 (Enter로 추가)" : ""}
            className="h-7 flex-1 min-w-[6rem] text-xs"
            aria-label="태그 추가"
          />
        </div>
        {tagSuggestions.length > 0 && (
          <div className="flex flex-wrap gap-1">
            {tagSuggestions.map((s) => (
              <button
                key={s}
                type="button"
                onClick={() => addTag(s)}
                className={`rounded-full px-2 py-0.5 text-xs ring-1 ${tagColor(s)} opacity-80 hover:opacity-100`}
              >
                + {s}
              </button>
            ))}
          </div>
        )}
      </div>

      <div className="flex items-center justify-between">
        <Button
          type="button"
          variant={value.favorite ? "secondary" : "ghost"}
          size="sm"
          className="h-7 px-2 text-xs gap-1"
          onClick={() => set("favorite", value.favorite ? undefined : true)}
        >
          <Star className={`size-3.5 ${value.favorite ? "fill-amber-400 text-amber-400" : ""}`} />
          즐겨찾기만
        </Button>
        {hasAny && (
          <Button
            type="button"
            variant="ghost"
            size="sm"
            className="h-7 px-2 text-xs gap-1 text-muted-foreground"
            onClick={reset}
          >
            <X className="size-3.5" />
            초기화
          </Button>
        )}
      </div>
    </div>
  );
}
