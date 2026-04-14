import { requestUrl } from "obsidian";

export class SeCallApi {
  constructor(private baseUrl: string) {}

  async recall(query: string, limit = 10) {
    const resp = await requestUrl({
      url: `${this.baseUrl}/api/recall`,
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ query, limit }),
    });
    return resp.json;
  }

  async get(sessionId: string, full = false) {
    const resp = await requestUrl({
      url: `${this.baseUrl}/api/get`,
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ session_id: sessionId, full }),
    });
    return resp.json;
  }

  async status() {
    const resp = await requestUrl({
      url: `${this.baseUrl}/api/status`,
      method: "GET",
    });
    return resp.json;
  }
}
